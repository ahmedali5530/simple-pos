use platform_kernel::contract::Registrar;
use platform_kernel::manifest::PluginManifest;
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};

mod models;
mod routes;
mod shop;

pub struct CorePlugin;

pub fn plugin() -> std::sync::Arc<dyn Plugin> {
    std::sync::Arc::new(CorePlugin)
}

impl Plugin for CorePlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("core", "Core", "0.1.0");
        manifest.default_enabled = true;
        manifest.description = "Core platform settings. Cannot be disabled.".into();
        manifest.permissions = vec![
            Permission::new("core.read"),
            Permission::new("settings.write"),
        ];
        manifest.settings_pages = vec![platform_kernel::manifest::SettingsPage {
            id: "system".into(),
            label: "System".into(),
            href: "/settings/system".into(),
            permission: Some("settings.write".into()),
            order: 50,
            children: Vec::new(),
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn register(&self, _registrar: &mut Registrar) {
        // No capabilities to register yet.
    }
}
