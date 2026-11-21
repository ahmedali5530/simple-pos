use crate::contract::Registrar;
use crate::events::PluginEvents;
use crate::manifest::PluginManifest;
use crate::migrate::Migration;
use crate::routes::{PluginRoutes, RegistrationError};

/// Statically linked business capability.
///
/// Plugins register routes and event handlers only through [`PluginRoutes`] and
/// [`PluginEvents`]. They never receive a database pool.
pub trait Plugin: Send + Sync + 'static {
    fn manifest(&self) -> PluginManifest;
    fn migrations(&self) -> &'static [Migration];
    fn register(&self, registrar: &mut Registrar);
    fn routes(&self, _routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        Ok(())
    }
    fn event_handlers(&self, _events: &mut PluginEvents) -> Result<(), RegistrationError> {
        Ok(())
    }
}
