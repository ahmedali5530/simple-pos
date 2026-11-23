use std::sync::Arc;

use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::{Dependency, NavItem, PluginManifest, SlotDecl};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};

mod routes;
mod sale;

pub struct MobilePlugin;

impl Plugin for MobilePlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("mobile", "Mobile & Warranty", "0.1.0");
        manifest.description = "Phone units, IMEI warranties and repair claims.".into();
        manifest.requires = vec![
            Dependency::new("catalog", "^0.1"),
            Dependency::new("pos", "^0.1"),
        ];
        manifest.permissions = vec![
            Permission::new("mobile.read"),
            Permission::new("mobile.write"),
        ];
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.nav = vec![
            NavItem {
                id: "mobile_devices".into(),
                label: "Devices".into(),
                href: "/mobile/devices".into(),
                permission: Some("mobile.read".into()),
                order: 48,
            },
            NavItem {
                id: "mobile_warranty".into(),
                label: "Warranty".into(),
                href: "/mobile/warranty".into(),
                permission: Some("mobile.read".into()),
                order: 49,
            },
        ];
        manifest.slot_contributions = vec![
            SlotDecl {
                name: "catalog.item-form".into(),
                contribution: "mobile.model".into(),
                order: 10,
            },
            SlotDecl {
                name: "pos.item-options".into(),
                contribution: "mobile.device".into(),
                order: 25,
            },
        ];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[
            Migration::new(1, "mobile", include_str!("../migrations/001_mobile.sql")),
            Migration::new(
                2,
                "mobile_sale_gate",
                include_str!("../migrations/002_sale_gate.sql"),
            ),
        ];
        MIGRATIONS
    }

    fn register(&self, _registrar: &mut platform_kernel::contract::Registrar) {}

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(sale::StartWarranties))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid_and_routes_stay_under_mobile() {
        let plugin = MobilePlugin;
        let manifest = plugin.manifest();
        manifest.validate().unwrap();
        assert!(manifest.events_consumed.iter().any(|event| event == "sales.bill_posted"));
        assert_eq!(manifest.slot_contributions.len(), 2);
        let declared = manifest
            .permissions
            .iter()
            .map(|permission| permission.as_str().to_string())
            .collect();
        let mut routes = PluginRoutes::new(manifest.id.clone(), declared);
        plugin.routes(&mut routes).unwrap();
        let (_router, specs) = routes.finish();
        let paths: Vec<_> = specs.iter().map(|spec| spec.path.as_str()).collect();
        assert!(paths.iter().all(|path| path.starts_with("/api/v1/mobile/")));
        assert!(paths.contains(&"/api/v1/mobile/devices"));
        assert!(paths.contains(&"/api/v1/mobile/claims"));
        assert!(paths.contains(&"/api/v1/mobile/models/{item_id}"));
    }
}
