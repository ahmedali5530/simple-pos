mod cache;
mod lifecycle;
mod permissions;
mod store;

pub use cache::EnabledPlugins;
pub use lifecycle::PluginLifecycle;
pub use permissions::sync as sync_permissions;
pub use store::PluginStateStore;

/// Advisory lock that serialises boot-time, tenant-independent plugin bookkeeping writes.
/// Parallel boots (tests, several till processes against one database) would otherwise race
/// on `platform_permissions` upserts and `tenant_plugins` updates.
pub(crate) const BOOT_STATE_LOCK: i64 = 87201402;
