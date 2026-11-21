use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;

use crate::capability::{Capability, CapabilityHandler};
use crate::event::EventHandler;
use crate::plugin::Plugin;

/// Collects routes, contract providers, event handlers and capabilities
/// while plugins register themselves.
pub struct Registrar {
    pub routes: Router,
    providers: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    pub event_handlers: Vec<Arc<dyn EventHandler>>,
    pub capabilities: Vec<(Capability, Arc<dyn CapabilityHandler>)>,
    pub deletion_vetoes: Vec<Arc<dyn DeletionVeto>>,
    pub printables: Vec<Arc<dyn Printable>>,
}

impl Registrar {
    pub fn new() -> Self {
        Self {
            routes: Router::new(),
            providers: HashMap::new(),
            event_handlers: Vec::new(),
            capabilities: Vec::new(),
            deletion_vetoes: Vec::new(),
            printables: Vec::new(),
        }
    }

    pub fn merge_routes(&mut self, router: Router) {
        self.routes = std::mem::take(&mut self.routes).merge(router);
    }

    pub fn provide<T: Any + Send + Sync>(&mut self, value: Arc<T>) {
        self.providers.insert(TypeId::of::<T>(), value);
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
        self.capabilities.push((spec, handler));
    }

    pub fn deletion_veto(&mut self, veto: Arc<dyn DeletionVeto>) {
        self.deletion_vetoes.push(veto);
    }

    pub fn printable(&mut self, printable: Arc<dyn Printable>) {
        self.printables.push(printable);
    }
}

impl Default for Registrar {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension point: refuse a delete when another plugin still references the row.
#[async_trait::async_trait]
pub trait DeletionVeto: Send + Sync {
    async fn check(
        &self,
        tx: &mut crate::context::TenantTx<'_>,
        entity: &str,
        id: i32,
    ) -> Result<Option<String>, crate::error::AppError>;
}

/// Extension point: a document that printing can render.
#[async_trait::async_trait]
pub trait Printable: Send + Sync {
    fn kind(&self) -> &str;
    async fn render(
        &self,
        tx: &mut crate::context::TenantTx<'_>,
        args: serde_json::Value,
    ) -> Result<Vec<u8>, crate::error::AppError>;
}

/// Helper used when a plugin is optional and missing.
pub fn noop_provider<T: Default + Send + Sync + 'static>() -> Arc<T> {
    Arc::new(T::default())
}

/// Marker so plugins can declare they registered.
pub trait RegisteredPlugin: Plugin {}
