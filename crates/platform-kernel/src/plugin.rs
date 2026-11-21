use crate::contract::Registrar;
use crate::manifest::PluginManifest;
use crate::migrate::Migration;

/// Statically linked business capability.
pub trait Plugin: Send + Sync + 'static {
    fn manifest(&self) -> PluginManifest;
    fn migrations(&self) -> &'static [Migration];
    fn register(&self, registrar: &mut Registrar);
}
