use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use semver::{Version, VersionReq};

use crate::error::AppError;
use crate::manifest::{ManifestError, PluginId, PluginManifest};
use crate::plugin::Plugin;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("{0}")]
    InvalidManifest(#[from] ManifestError),
    #[error("plugin `{0}` is compiled in more than once")]
    DuplicateId(PluginId),
    #[error("{kind} `{value}` is claimed by both `{first}` and `{second}`")]
    DuplicateClaim {
        kind: &'static str,
        value: String,
        first: String,
        second: String,
    },
    #[error("plugin `{plugin}` requires `{dependency}`, which is not compiled in")]
    MissingDependency {
        plugin: PluginId,
        dependency: PluginId,
    },
    #[error("plugin `{plugin}` requires `{dependency}` {required}, but {actual} is compiled in")]
    IncompatibleDependency {
        plugin: PluginId,
        dependency: PluginId,
        required: String,
        actual: String,
    },
    #[error("plugin dependency cycle: {0}")]
    Cycle(String),
    #[error("plugin `{0}` is not compiled in")]
    NotCompiledIn(PluginId),
    #[error("cannot enable `{plugin}` because required plugins are disabled: {missing}")]
    DependencyDisabled { plugin: PluginId, missing: String },
    #[error("cannot disable `{plugin}` while {dependents} still require it")]
    HasEnabledDependents {
        plugin: PluginId,
        dependents: String,
    },
}

fn join_ids(ids: &[PluginId]) -> String {
    ids.iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

impl From<RegistryError> for AppError {
    fn from(value: RegistryError) -> Self {
        match &value {
            RegistryError::DependencyDisabled { .. }
            | RegistryError::HasEnabledDependents { .. } => AppError::Conflict(value.to_string()),
            _ => AppError::BadRequest(value.to_string()),
        }
    }
}

/// A row loaded from `tenant_plugins`.
#[derive(Debug, Clone)]
pub struct PersistedPlugin {
    pub id: PluginId,
    pub enabled: bool,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedSkip {
    pub plugin: PluginId,
    pub reason: String,
}

/// Immutable set of compiled-in plugins, validated once at boot.
pub struct PluginCatalog {
    plugins: HashMap<PluginId, Arc<dyn Plugin>>,
    manifests: HashMap<PluginId, PluginManifest>,
    order: Vec<PluginId>,
    dependents: HashMap<PluginId, Vec<PluginId>>,
}

impl PluginCatalog {
    pub fn build(plugins: Vec<Arc<dyn Plugin>>) -> Result<Self, RegistryError> {
        let mut by_id: HashMap<PluginId, Arc<dyn Plugin>> = HashMap::new();
        let mut manifests: HashMap<PluginId, PluginManifest> = HashMap::new();
        for plugin in plugins {
            let manifest = plugin.manifest();
            manifest.validate()?;
            let id = manifest.id.clone();
            if by_id.contains_key(&id) {
                return Err(RegistryError::DuplicateId(id));
            }
            by_id.insert(id.clone(), plugin);
            manifests.insert(id, manifest);
        }

        validate_claims(&manifests.values().cloned().collect::<Vec<_>>())?;

        let mut edges: HashMap<PluginId, Vec<PluginId>> = HashMap::new();
        for manifest in manifests.values() {
            for dependency in manifest.requires.iter().chain(manifest.optional.iter()) {
                let Some(target) = manifests.get(&dependency.id) else {
                    if manifest
                        .requires
                        .iter()
                        .any(|required| required.id == dependency.id)
                    {
                        return Err(RegistryError::MissingDependency {
                            plugin: manifest.id.clone(),
                            dependency: dependency.id.clone(),
                        });
                    }
                    continue;
                };
                if !dependency.version.matches(&target.version) {
                    return Err(RegistryError::IncompatibleDependency {
                        plugin: manifest.id.clone(),
                        dependency: dependency.id.clone(),
                        required: dependency.version.to_string(),
                        actual: target.version.to_string(),
                    });
                }
            }
            edges.insert(
                manifest.id.clone(),
                manifest
                    .requires
                    .iter()
                    .map(|dependency| dependency.id.clone())
                    .collect(),
            );
        }

        let ids: Vec<_> = manifests.keys().cloned().collect();
        let order = topo_order(&ids, &edges)?;
        let mut dependents: HashMap<PluginId, Vec<PluginId>> = HashMap::new();
        for (plugin, deps) in &edges {
            for dep in deps {
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(plugin.clone());
            }
        }
        for list in dependents.values_mut() {
            list.sort();
        }

        Ok(Self {
            plugins: by_id,
            manifests,
            order,
            dependents,
        })
    }

    pub fn topo_order(&self) -> &[PluginId] {
        &self.order
    }

    pub fn manifest(&self, id: &PluginId) -> Option<&PluginManifest> {
        self.manifests.get(id)
    }

    pub fn manifests(&self) -> Vec<PluginManifest> {
        self.order
            .iter()
            .filter_map(|id| self.manifests.get(id).cloned())
            .collect()
    }

    /// Plugins that directly require `id`.
    pub fn dependents(&self, id: &PluginId) -> Vec<PluginId> {
        self.dependents.get(id).cloned().unwrap_or_default()
    }

    pub fn plugin(&self, id: &PluginId) -> Option<Arc<dyn Plugin>> {
        self.plugins.get(id).cloned()
    }

    pub fn plugins_in_order(&self) -> Vec<Arc<dyn Plugin>> {
        self.order
            .iter()
            .filter_map(|id| self.plugins.get(id).cloned())
            .collect()
    }
}

fn validate_claims(manifests: &[PluginManifest]) -> Result<(), RegistryError> {
    let mut claims: HashMap<(&'static str, String), String> = HashMap::new();
    let mut claim =
        |kind: &'static str, value: &str, owner: &PluginId| -> Result<(), RegistryError> {
            if value.is_empty() {
                return Ok(());
            }
            let key = (kind, value.to_string());
            if let Some(first) = claims.insert(key, owner.to_string()) {
                return Err(RegistryError::DuplicateClaim {
                    kind,
                    value: value.to_string(),
                    first,
                    second: owner.to_string(),
                });
            }
            Ok(())
        };
    fn nav_claims(
        entries: &[crate::manifest::NavItem],
        owner: &PluginId,
        claim: &mut impl FnMut(&'static str, &str, &PluginId) -> Result<(), RegistryError>,
    ) -> Result<(), RegistryError> {
        for entry in entries {
            claim("navigation id", &entry.id, owner)?;
            claim("navigation path", &entry.href, owner)?;
            nav_claims(&entry.children, owner, claim)?;
        }
        Ok(())
    }
    for manifest in manifests {
        nav_claims(&manifest.nav, &manifest.id, &mut claim)?;
        for page in &manifest.settings_pages {
            claim("settings page id", &page.id, &manifest.id)?;
            claim("settings path", &page.href, &manifest.id)?;
        }
        for page in &manifest.report_pages {
            claim("report page id", &page.id, &manifest.id)?;
            claim("report path", &page.href, &manifest.id)?;
        }
        for page in &manifest.pages {
            claim("page path", &page.path, &manifest.id)?;
        }
        for slot in &manifest.slots_provided {
            claim("slot provider", slot, &manifest.id)?;
        }
        for capability in &manifest.capabilities {
            claim("capability id", &capability.id, &manifest.id)?;
        }
    }
    Ok(())
}

/// Which compiled-in plugins are enabled. Pure: no I/O.
#[derive(Debug, Clone, Default)]
pub struct EnabledSet {
    enabled: HashSet<PluginId>,
}

impl EnabledSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn contains(&self, id: &PluginId) -> bool {
        self.enabled.contains(id)
    }

    pub fn ids(&self) -> HashSet<PluginId> {
        self.enabled.clone()
    }

    pub fn enable(&mut self, catalog: &PluginCatalog, id: &PluginId) -> Result<(), RegistryError> {
        if catalog.manifest(id).is_none() {
            return Err(RegistryError::NotCompiledIn(id.clone()));
        }
        if self.enabled.contains(id) {
            return Ok(());
        }
        let manifest = catalog.manifest(id).expect("checked");
        let missing: Vec<_> = manifest
            .requires
            .iter()
            .filter(|dependency| !self.enabled.contains(&dependency.id))
            .map(|dependency| dependency.id.clone())
            .collect();
        if !missing.is_empty() {
            return Err(RegistryError::DependencyDisabled {
                plugin: id.clone(),
                missing: join_ids(&missing),
            });
        }
        self.enabled.insert(id.clone());
        Ok(())
    }

    pub fn disable(&mut self, catalog: &PluginCatalog, id: &PluginId) -> Result<(), RegistryError> {
        if catalog.manifest(id).is_none() {
            return Err(RegistryError::NotCompiledIn(id.clone()));
        }
        if !self.enabled.contains(id) {
            return Ok(());
        }
        let dependents: Vec<_> = catalog
            .dependents(id)
            .into_iter()
            .filter(|dependent| self.enabled.contains(dependent))
            .collect();
        if !dependents.is_empty() {
            return Err(RegistryError::HasEnabledDependents {
                plugin: id.clone(),
                dependents: join_ids(&dependents),
            });
        }
        self.enabled.remove(id);
        Ok(())
    }

    /// Load enabled rows in dependency order.
    ///
    /// Rows whose required dependencies are disabled, or whose stored version is
    /// incompatible with the compiled-in version, are skipped and reported.
    /// Nothing is deleted.
    pub fn from_persisted(
        catalog: &PluginCatalog,
        rows: &[PersistedPlugin],
    ) -> (Self, Vec<PersistedSkip>) {
        let mut set = EnabledSet::new();
        let mut skips = Vec::new();
        let known: HashMap<&PluginId, &PersistedPlugin> = rows
            .iter()
            .filter_map(|row| match catalog.manifest(&row.id) {
                Some(_) => Some((&row.id, row)),
                None => {
                    skips.push(PersistedSkip {
                        plugin: row.id.clone(),
                        reason: "not compiled in".into(),
                    });
                    None
                }
            })
            .collect();

        for id in catalog.topo_order() {
            let Some(row) = known.get(id) else { continue };
            if !row.enabled {
                continue;
            }
            let compiled = &catalog.manifest(id).expect("in catalog").version;
            if !stored_version_compatible(&row.version, compiled) {
                skips.push(PersistedSkip {
                    plugin: id.clone(),
                    reason: format!(
                        "stored version {} is incompatible with {compiled}",
                        row.version
                    ),
                });
                continue;
            }
            if let Err(err) = set.enable(catalog, id) {
                skips.push(PersistedSkip {
                    plugin: id.clone(),
                    reason: err.to_string(),
                });
            }
        }
        (set, skips)
    }
}

fn stored_version_compatible(stored: &str, compiled: &Version) -> bool {
    let Ok(stored_version) = Version::parse(stored) else {
        return false;
    };
    VersionReq::parse(&format!("^{stored_version}"))
        .map(|req| req.matches(compiled))
        .unwrap_or(false)
}

fn topo_order(
    ids: &[PluginId],
    edges: &HashMap<PluginId, Vec<PluginId>>,
) -> Result<Vec<PluginId>, RegistryError> {
    let mut indegree: HashMap<PluginId, usize> = ids.iter().cloned().map(|id| (id, 0)).collect();
    let mut dependents: HashMap<PluginId, Vec<PluginId>> = HashMap::new();
    for id in ids {
        for dep in edges.get(id).into_iter().flatten() {
            *indegree.get_mut(id).expect("id") += 1;
            dependents.entry(dep.clone()).or_default().push(id.clone());
        }
    }
    let mut ready: Vec<PluginId> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect();
    ready.sort();
    let mut queue: VecDeque<PluginId> = ready.into();
    let mut order = Vec::new();
    while let Some(id) = queue.pop_front() {
        order.push(id.clone());
        let mut next = dependents.get(&id).cloned().unwrap_or_default();
        next.sort();
        let mut newly_ready = Vec::new();
        for dependent in next {
            let degree = indegree.get_mut(&dependent).expect("dependent");
            *degree -= 1;
            if *degree == 0 {
                newly_ready.push(dependent);
            }
        }
        newly_ready.sort();
        queue.extend(newly_ready);
    }
    if order.len() != ids.len() {
        return Err(RegistryError::Cycle(
            join_ids(&find_cycle(ids, edges)).replace(", ", " -> "),
        ));
    }
    Ok(order)
}

fn find_cycle(ids: &[PluginId], edges: &HashMap<PluginId, Vec<PluginId>>) -> Vec<PluginId> {
    let mut visiting = HashSet::new();
    let mut stack = Vec::new();
    let mut cycle = Vec::new();
    fn walk(
        id: &PluginId,
        edges: &HashMap<PluginId, Vec<PluginId>>,
        visiting: &mut HashSet<PluginId>,
        stack: &mut Vec<PluginId>,
        cycle: &mut Vec<PluginId>,
    ) {
        if !cycle.is_empty() {
            return;
        }
        if !visiting.insert(id.clone()) {
            if let Some(start) = stack.iter().position(|seen| seen == id) {
                *cycle = stack[start..].to_vec();
                cycle.push(id.clone());
            }
            return;
        }
        stack.push(id.clone());
        for dep in edges.get(id).into_iter().flatten() {
            walk(dep, edges, visiting, stack, cycle);
        }
        stack.pop();
        visiting.remove(id);
    }
    for id in ids {
        walk(id, edges, &mut visiting, &mut stack, &mut cycle);
        if !cycle.is_empty() {
            break;
        }
    }
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Registrar;
    use crate::migrate::Migration;
    use crate::permission::Permission;

    struct Fake {
        manifest: PluginManifest,
    }

    impl Plugin for Fake {
        fn manifest(&self) -> PluginManifest {
            self.manifest.clone()
        }
        fn migrations(&self) -> &'static [Migration] {
            &[]
        }
        fn register(&self, _: &mut Registrar) {}
    }

    fn plugin(manifest: PluginManifest) -> Arc<dyn Plugin> {
        Arc::new(Fake { manifest })
    }

    fn manifest(id: &str, version: &str) -> PluginManifest {
        let mut manifest = PluginManifest::v1(id, id, version);
        manifest.permissions = vec![Permission::new(format!("{id}.read"))];
        manifest
    }

    #[test]
    fn topo_order_is_deterministic() {
        let mut inventory = manifest("inventory", "0.1.0");
        inventory.requires = vec![crate::manifest::Dependency::new("catalog", "^0.1")];
        let mut pos = manifest("pos", "0.1.0");
        pos.requires = vec![crate::manifest::Dependency::new("catalog", "^0.1")];
        let catalog = PluginCatalog::build(vec![
            plugin(pos),
            plugin(inventory),
            plugin(manifest("catalog", "0.1.0")),
        ])
        .unwrap();
        let ids: Vec<_> = catalog
            .topo_order()
            .iter()
            .map(|id| id.as_str().to_string())
            .collect();
        assert_eq!(ids, vec!["catalog", "inventory", "pos"]);
    }

    #[test]
    fn rejects_duplicate_missing_mismatch_and_cycle() {
        let duplicate = PluginCatalog::build(vec![
            plugin(manifest("catalog", "0.1.0")),
            plugin(manifest("catalog", "0.1.1")),
        ]);
        assert!(matches!(duplicate, Err(RegistryError::DuplicateId(_))));

        let mut inventory = manifest("inventory", "0.1.0");
        inventory.requires = vec![crate::manifest::Dependency::new("catalog", "^0.1")];
        let missing = PluginCatalog::build(vec![plugin(inventory)]);
        assert!(matches!(
            missing,
            Err(RegistryError::MissingDependency { .. })
        ));

        let mut inventory = manifest("inventory", "0.1.0");
        inventory.requires = vec![crate::manifest::Dependency::new("catalog", "^0.2")];
        let mismatch = PluginCatalog::build(vec![
            plugin(manifest("catalog", "0.1.0")),
            plugin(inventory),
        ]);
        assert!(matches!(
            mismatch,
            Err(RegistryError::IncompatibleDependency { .. })
        ));

        let mut left = manifest("left", "0.1.0");
        left.requires = vec![crate::manifest::Dependency::new("right", "^0.1")];
        let mut right = manifest("right", "0.1.0");
        right.requires = vec![crate::manifest::Dependency::new("left", "^0.1")];
        let cycle = PluginCatalog::build(vec![plugin(left), plugin(right)]);
        assert!(matches!(cycle, Err(RegistryError::Cycle(_))));
    }

    #[test]
    fn enable_and_disable_respect_dependencies() {
        let mut inventory = manifest("inventory", "0.1.0");
        inventory.requires = vec![crate::manifest::Dependency::new("catalog", "^0.1")];
        let catalog = PluginCatalog::build(vec![
            plugin(manifest("catalog", "0.1.0")),
            plugin(inventory),
        ])
        .unwrap();
        let mut set = EnabledSet::new();
        let inventory_id = PluginId::must("inventory");
        let catalog_id = PluginId::must("catalog");
        let blocked = set.enable(&catalog, &inventory_id).unwrap_err();
        assert!(matches!(blocked, RegistryError::DependencyDisabled { .. }));
        set.enable(&catalog, &catalog_id).unwrap();
        set.enable(&catalog, &inventory_id).unwrap();
        set.enable(&catalog, &inventory_id).unwrap();
        let blocked = set.disable(&catalog, &catalog_id).unwrap_err();
        match blocked {
            RegistryError::HasEnabledDependents { dependents, .. } => {
                assert_eq!(dependents, inventory_id.as_str());
            }
            other => panic!("unexpected {other}"),
        }
        set.disable(&catalog, &inventory_id).unwrap();
        set.disable(&catalog, &catalog_id).unwrap();
    }

    #[test]
    fn from_persisted_skips_orphans_and_incompatible_versions() {
        let mut inventory = manifest("inventory", "0.1.0");
        inventory.requires = vec![crate::manifest::Dependency::new("catalog", "^0.1")];
        let catalog = PluginCatalog::build(vec![
            plugin(manifest("catalog", "0.1.0")),
            plugin(inventory),
        ])
        .unwrap();
        let (set, skips) = EnabledSet::from_persisted(
            &catalog,
            &[
                PersistedPlugin {
                    id: PluginId::must("catalog"),
                    enabled: false,
                    version: "0.1.0".into(),
                },
                PersistedPlugin {
                    id: PluginId::must("inventory"),
                    enabled: true,
                    version: "0.1.0".into(),
                },
                PersistedPlugin {
                    id: PluginId::must("gone"),
                    enabled: true,
                    version: "9.0.0".into(),
                },
            ],
        );
        assert!(!set.contains(&PluginId::must("inventory")));
        assert!(skips.iter().any(|skip| skip.plugin.as_str() == "inventory"));
        assert!(skips.iter().any(|skip| skip.plugin.as_str() == "gone"));

        let mut inventory = manifest("inventory", "0.2.0");
        inventory.requires = vec![crate::manifest::Dependency::new("catalog", "^0.1")];
        let catalog = PluginCatalog::build(vec![
            plugin(manifest("catalog", "0.1.0")),
            plugin(inventory),
        ])
        .unwrap();
        let (set, skips) = EnabledSet::from_persisted(
            &catalog,
            &[PersistedPlugin {
                id: PluginId::must("inventory"),
                enabled: true,
                version: "0.1.0".into(),
            }],
        );
        assert!(!set.contains(&PluginId::must("inventory")));
        assert!(skips
            .iter()
            .any(|skip| skip.reason.contains("incompatible")));
        let _ = set;
    }
}
