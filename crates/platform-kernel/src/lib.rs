//! Business-neutral platform kernel: plugin traits, manifests, permissions,
//! contracts, events, and the tenant transaction handle.

pub mod capability;
pub mod context;
pub mod contract;
pub mod error;
pub mod event;
pub mod manifest;
pub mod migrate;
pub mod permission;
pub mod plugin;
pub mod registry;

pub use capability::{Capability, CapabilityEffect, CapabilityKind, CapabilityRegistry};
pub use context::{Actor, TenantId, TenantTx};
pub use contract::Registrar;
pub use error::{AppError, ErrorBody};
pub use event::{DomainEvent, EventEnvelope, EventHandler};
pub use manifest::{NavItem, PluginManifest, ReportPage, SettingsPage};
pub use migrate::Migration;
pub use permission::Permission;
pub use plugin::Plugin;
pub use registry::PluginRegistry;
