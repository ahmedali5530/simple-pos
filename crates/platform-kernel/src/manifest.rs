use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::Capability;
use crate::permission::Permission;

/// Schema version of [`PluginManifest`]. Bump only on a breaking manifest change.
pub const MANIFEST_VERSION: u32 = 1;

/// Kernel API this binary implements. Plugins declare a compatible range.
pub fn kernel_api_version() -> Version {
    Version::new(1, 0, 0)
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ManifestError {
    #[error("plugin id `{0}` is invalid; use 2–32 characters of [a-z0-9_], starting with a letter")]
    InvalidId(String),
    #[error("unsupported manifest version {0}")]
    UnsupportedVersion(u32),
    #[error("plugin `{plugin}` requires kernel API `{required}`, but this kernel is {actual}")]
    IncompatibleKernel {
        plugin: String,
        required: String,
        actual: String,
    },
    #[error("plugin `{plugin}` cannot declare permission `{permission}`")]
    ForeignPermission { plugin: String, permission: String },
}

/// Plugin identifier. Serialized as a plain string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(String);

impl PluginId {
    pub fn parse(value: &str) -> Result<Self, ManifestError> {
        let ok = (2..=32).contains(&value.len())
            && value.starts_with(|c: char| c.is_ascii_lowercase())
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if ok {
            Ok(Self(value.to_string()))
        } else {
            Err(ManifestError::InvalidId(value.to_string()))
        }
    }

    /// For compiled-in manifests whose ids are known to be valid.
    pub fn must(value: &str) -> Self {
        Self::parse(value).unwrap_or_else(|err| panic!("{err}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A required or optional dependency with a semver range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub id: PluginId,
    pub version: VersionReq,
}

impl Dependency {
    pub fn new(id: &str, version: &str) -> Self {
        Self {
            id: PluginId::must(id),
            version: VersionReq::parse(version)
                .unwrap_or_else(|err| panic!("version requirement `{version}`: {err}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItem {
    pub id: String,
    pub label: String,
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPage {
    pub id: String,
    pub label: String,
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPage {
    pub id: String,
    pub label: String,
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
}

/// Permissions a plugin may declare that are not under its own id prefix.
const SHARED_PERMISSIONS: &[&str] = &["users.write", "settings.write"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub manifest_version: u32,
    pub id: PluginId,
    pub name: String,
    pub version: Version,
    /// Semver range of kernel API versions this plugin supports.
    pub kernel_api: VersionReq,
    pub description: String,
    #[serde(default)]
    pub requires: Vec<Dependency>,
    #[serde(default)]
    pub optional: Vec<Dependency>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub events_emitted: Vec<String>,
    #[serde(default)]
    pub events_consumed: Vec<String>,
    #[serde(default)]
    pub settings_schema: Option<Value>,
    #[serde(default)]
    pub nav: Vec<NavItem>,
    #[serde(default)]
    pub settings_pages: Vec<SettingsPage>,
    #[serde(default)]
    pub report_pages: Vec<ReportPage>,
}

impl PluginManifest {
    /// A valid v1 manifest. Panics if `id` or `version` is not valid — compiled-in data only.
    pub fn v1(id: &str, name: impl Into<String>, version: &str) -> Self {
        Self {
            manifest_version: MANIFEST_VERSION,
            id: PluginId::must(id),
            name: name.into(),
            version: Version::parse(version).unwrap_or_else(|err| panic!("plugin version `{version}`: {err}")),
            kernel_api: VersionReq::parse("^1.0").expect("kernel api range"),
            description: String::new(),
            requires: Vec::new(),
            optional: Vec::new(),
            permissions: Vec::new(),
            capabilities: Vec::new(),
            events_emitted: Vec::new(),
            events_consumed: Vec::new(),
            settings_schema: None,
            nav: Vec::new(),
            settings_pages: Vec::new(),
            report_pages: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        PluginId::parse(self.id.as_str())?;
        if self.manifest_version != MANIFEST_VERSION {
            return Err(ManifestError::UnsupportedVersion(self.manifest_version));
        }
        if !self.kernel_api.matches(&kernel_api_version()) {
            return Err(ManifestError::IncompatibleKernel {
                plugin: self.id.to_string(),
                required: self.kernel_api.to_string(),
                actual: kernel_api_version().to_string(),
            });
        }
        let prefix = format!("{}.", self.id.as_str());
        for permission in &self.permissions {
            let name = permission.as_str();
            if name.starts_with(&prefix) || SHARED_PERMISSIONS.contains(&name) {
                continue;
            }
            return Err(ManifestError::ForeignPermission {
                plugin: self.id.to_string(),
                permission: name.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::Permission;

    #[test]
    fn rejects_bad_plugin_id() {
        assert!(PluginId::parse("Catalog").is_err());
        assert!(PluginId::parse("a").is_err());
        assert!(PluginId::parse("1catalog").is_err());
        assert!(PluginId::parse("catalog").is_ok());
    }

    #[test]
    fn rejects_unsupported_manifest_version() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.manifest_version = 99;
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn rejects_incompatible_kernel_api() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.kernel_api = VersionReq::parse("^2.0").unwrap();
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::IncompatibleKernel { .. })
        ));
    }

    #[test]
    fn rejects_foreign_permission_namespace() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.permissions = vec![Permission::new("inventory.write")];
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::ForeignPermission { .. })
        ));
    }

    #[test]
    fn allows_own_namespace_and_shared_permissions() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.permissions = vec![
            Permission::new("catalog.read"),
            Permission::new("users.write"),
            Permission::new("settings.write"),
        ];
        assert!(manifest.validate().is_ok());
    }
}
