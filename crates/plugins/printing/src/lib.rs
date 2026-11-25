pub mod print;
mod routes;

use platform_kernel::contract::Registrar;
use platform_kernel::manifest::{Dependency, PluginManifest, SettingsPage};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};

pub struct PrintingPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> std::sync::Arc<dyn Plugin> {
    std::sync::Arc::new(PrintingPlugin)
}

impl Plugin for PrintingPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("printing", "Printing", "0.1.0");
        manifest.default_enabled = true;
        manifest.description = "Receipt printer settings and ESC/POS dispatch.".into();
        manifest.optional = vec![
            Dependency::new("pos", "^0.1"),
            Dependency::new("inventory", "^0.1"),
            Dependency::new("cash", "^0.1"),
        ];
        manifest.permissions = vec![
            Permission::new("printing.use"),
            Permission::new("settings.write"),
        ];
        manifest.settings_pages = vec![SettingsPage {
            id: "printing".into(),
            label: "Printing".into(),
            href: "/settings/printing".into(),
            permission: Some("settings.write".into()),
            order: 55,
            children: Vec::new(),
        }];
        manifest
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _registrar: &mut Registrar) {}
}
