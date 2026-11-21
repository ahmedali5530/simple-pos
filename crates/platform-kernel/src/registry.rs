use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::capability::CapabilityRegistry;
use crate::contract::Registrar;
use crate::error::AppError;
use crate::manifest::PluginManifest;
use crate::plugin::Plugin;

/// Static registry of compiled-in plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, Arc<dyn Plugin>>,
    enabled: HashSet<String>,
    pub registrar: Registrar,
    pub capabilities: CapabilityRegistry,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            enabled: HashSet::new(),
            registrar: Registrar::new(),
            capabilities: CapabilityRegistry::new(),
        }
    }

    pub fn add(&mut self, plugin: Arc<dyn Plugin>) {
        let id = plugin.manifest().id.clone();
        self.plugins.insert(id, plugin);
    }

    pub fn enable(&mut self, id: &str) -> Result<(), AppError> {
        let plugin = self
            .plugins
            .get(id)
            .ok_or_else(|| AppError::NotFound(format!("Plugin `{id}` is not compiled in.")))?
            .clone();
        let manifest = plugin.manifest();
        for required in &manifest.requires {
            if !self.enabled.contains(required) && required != id {
                if !self.plugins.contains_key(required) {
                    return Err(AppError::BadRequest(format!(
                        "Plugin `{id}` requires `{required}`, which is not compiled in."
                    )));
                }
            }
        }
        self.enabled.insert(id.to_string());
        Ok(())
    }

    pub fn disable(&mut self, id: &str) -> Result<(), AppError> {
        for (other_id, plugin) in &self.plugins {
            if other_id == id || !self.enabled.contains(other_id) {
                continue;
            }
            if plugin.manifest().requires.iter().any(|r| r == id) {
                return Err(AppError::BadRequest(format!(
                    "Cannot disable `{id}` while `{other_id}` still requires it."
                )));
            }
        }
        self.enabled.remove(id);
        Ok(())
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.contains(id)
    }

    pub fn enabled_ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = self.enabled.iter().cloned().collect();
        ids.sort();
        ids
    }

    pub fn manifests(&self) -> Vec<PluginManifest> {
        let mut manifests: Vec<_> = self
            .enabled
            .iter()
            .filter_map(|id| self.plugins.get(id).map(|p| p.manifest()))
            .collect();
        manifests.sort_by(|a, b| a.id.cmp(&b.id));
        manifests
    }

    pub fn register_all(&mut self) {
        let enabled: Vec<_> = self
            .enabled
            .iter()
            .filter_map(|id| self.plugins.get(id).cloned())
            .collect();
        for plugin in enabled {
            plugin.register(&mut self.registrar);
        }
        for (spec, handler) in self.registrar.capabilities.drain(..) {
            self.capabilities.register(spec, handler);
        }
    }

    pub fn plugin(&self, id: &str) -> Option<Arc<dyn Plugin>> {
        self.plugins.get(id).cloned()
    }

    pub fn all_plugins(&self) -> Vec<Arc<dyn Plugin>> {
        self.plugins.values().cloned().collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
