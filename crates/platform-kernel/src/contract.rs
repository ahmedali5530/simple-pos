use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::capability::{Capability, CapabilityHandler};
use crate::event::EventHandler;
use crate::plugin::Plugin;

/// Collects routes, contract providers, event handlers and capabilities
/// while plugins register themselves.
pub struct Registrar {
    providers: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    provider_owners: HashMap<TypeId, crate::manifest::PluginId>,
    current_owner: Option<crate::manifest::PluginId>,
    pub event_handlers: Vec<Arc<dyn EventHandler>>,
    pub capabilities: Vec<(Capability, Arc<dyn CapabilityHandler>)>,
    pub capability_owners: Vec<Option<crate::manifest::PluginId>>,
}

#[derive(Clone, Default)]
pub struct ContractRegistry {
    providers: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    owners: HashMap<TypeId, crate::manifest::PluginId>,
}

impl ContractRegistry {
    pub fn get<T: Any + Send + Sync>(&self, enabled: &std::collections::HashSet<crate::manifest::PluginId>) -> Option<Arc<T>> {
        let key = TypeId::of::<T>();
        if self.owners.get(&key).is_some_and(|owner| !enabled.contains(owner)) {
            return None;
        }
        self.providers.get(&key)?.clone().downcast::<T>().ok()
    }
}

impl Registrar {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            provider_owners: HashMap::new(),
            current_owner: None,
            event_handlers: Vec::new(),
            capabilities: Vec::new(),
            capability_owners: Vec::new(),
        }
    }

    pub fn provide<T: Any + Send + Sync>(&mut self, value: Arc<T>) {
        let key = TypeId::of::<T>();
        self.providers.insert(key, value);
        if let Some(owner) = self.current_owner.clone() {
            self.provider_owners.insert(key, owner);
        }
    }

    /// Register a provider that is not tied to one plugin owner.
    /// Used for composable bags (e.g. multi-handler tender apply) so disabling
    /// one contributing plugin does not hide the whole registry.
    pub fn provide_shared<T: Any + Send + Sync>(&mut self, value: Arc<T>) {
        let key = TypeId::of::<T>();
        self.providers.insert(key, value);
        self.provider_owners.remove(&key);
    }

    pub fn begin_plugin(&mut self, owner: crate::manifest::PluginId) {
        self.current_owner = Some(owner);
    }

    pub fn contracts(&self) -> ContractRegistry {
        ContractRegistry {
            providers: self.providers.clone(),
            owners: self.provider_owners.clone(),
        }
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.providers
            .get(&TypeId::of::<T>())
            .and_then(|v| v.clone().downcast::<T>().ok())
    }

    pub fn on_event(&mut self, handler: Arc<dyn EventHandler>) {
        self.event_handlers.push(handler);
    }

    pub fn capability(&mut self, spec: Capability, handler: Arc<dyn CapabilityHandler>) {
        self.capability_owners.push(self.current_owner.clone());
        self.capabilities.push((spec, handler));
    }

}

impl Default for Registrar {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper used when a plugin is optional and missing.
pub fn noop_provider<T: Default + Send + Sync + 'static>() -> Arc<T> {
    Arc::new(T::default())
}

/// Marker so plugins can declare they registered.
pub trait RegisteredPlugin: Plugin {}
