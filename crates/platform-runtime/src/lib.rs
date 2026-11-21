//! Platform runtime: boot, pools, migrations, RLS, gateway, and HTTP middleware.

pub mod admin;
pub mod boot;
pub mod db;
pub mod events;
pub mod gateway;
pub mod http;
pub mod migrations;
pub mod plugins;

pub use boot::{BootConfig, Platform};
pub use db::{Pools, TenantTxExt};
pub use gateway::CapabilityGateway;
pub use plugins::{EnabledPlugins, PluginLifecycle, PluginStateStore};
