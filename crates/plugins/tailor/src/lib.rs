use platform_kernel::manifest::{Dependency, NavItem, PluginManifest, SlotDecl};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};

mod routes;

pub struct TailorPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> std::sync::Arc<dyn Plugin> {
    std::sync::Arc::new(TailorPlugin)
}

impl Plugin for TailorPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("tailor", "Tailor", "0.1.0");
        manifest.description = "Orders with measurements, cloth, sewing, delivery, and payment splits.".into();
        manifest.optional = vec![Dependency::new("customers", "^0.1")];
        manifest.permissions = vec![Permission::new("tailor.read"), Permission::new("tailor.write")];
        manifest.nav = vec![NavItem {
            id: "tailor".into(),
            label: "Jobs".into(),
            href: "/tailor".into(),
            permission: Some("tailor.read".into()),
            order: 46,
            children: Vec::new(),
        }];
        manifest.slot_contributions = vec![SlotDecl {
            name: "pos.line-source".into(),
            contribution: "tailor.job".into(),
            order: 20,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[
            Migration::new(1, "jobs", include_str!("../migrations/001_jobs.sql")),
            Migration::new(2, "order", include_str!("../migrations/002_order.sql")),
        ];
        MIGRATIONS
    }

    fn register(&self, _registrar: &mut platform_kernel::contract::Registrar) {}

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }
}
