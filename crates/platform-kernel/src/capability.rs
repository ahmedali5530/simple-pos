use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::TenantTx;
use crate::error::AppError;
use crate::manifest::PluginId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Query,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEffect {
    Read,
    Write,
    Destructive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub kind: CapabilityKind,
    pub permission: String,
    pub effect: CapabilityEffect,
    #[serde(default)]
    pub requires_confirmation: bool,
    pub input_schema: Value,
    pub output_schema: Value,
    #[serde(default)]
    pub description: String,
}

#[async_trait]
pub trait CapabilityHandler: Send + Sync {
    async fn invoke(&self, tx: &mut TenantTx<'_>, input: Value) -> Result<Value, AppError>;
}

#[derive(Default)]
pub struct CapabilityRegistry {
    handlers: HashMap<String, Arc<dyn CapabilityHandler>>,
    specs: HashMap<String, Capability>,
    owners: HashMap<String, PluginId>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, spec: Capability, handler: Arc<dyn CapabilityHandler>) {
        self.handlers.insert(spec.id.clone(), handler);
        self.specs.insert(spec.id.clone(), spec);
    }

    pub fn register_owned(&mut self, owner: PluginId, spec: Capability, handler: Arc<dyn CapabilityHandler>) {
        self.owners.insert(spec.id.clone(), owner);
        self.register(spec, handler);
    }

    pub fn owner(&self, id: &str) -> Option<&PluginId> {
        self.owners.get(id)
    }

    pub fn get(&self, id: &str) -> Option<(&Capability, Arc<dyn CapabilityHandler>)> {
        let spec = self.specs.get(id)?;
        let handler = self.handlers.get(id)?.clone();
        Some((spec, handler))
    }

    pub fn list(&self) -> Vec<&Capability> {
        let mut specs: Vec<_> = self.specs.values().collect();
        specs.sort_by(|a, b| a.id.cmp(&b.id));
        specs
    }
}
