use std::sync::Arc;

use platform_kernel::contract::Registrar;
use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::{Dependency, PluginManifest, SlotDecl};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};

mod routes;
mod sale;
mod service;

pub struct MatrixPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> Arc<dyn Plugin> {
    Arc::new(MatrixPlugin)
}

impl Plugin for MatrixPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("matrix", "Variant Matrix", "0.1.0");
        manifest.description =
            "Dynamic variant groups with per-SKU price, cost, and stock for catalog and POS."
                .into();
        manifest.optional = vec![
            Dependency::new("catalog", "^0.1"),
            Dependency::new("pos", "^0.1"),
            Dependency::new("inventory", "^0.1"),
        ];
        manifest.permissions = vec![
            Permission::new("matrix.read"),
            Permission::new("matrix.write"),
        ];
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.slot_contributions = vec![
            SlotDecl {
                name: "catalog.item-form".into(),
                contribution: "matrix.recipe".into(),
                order: 20,
            },
            SlotDecl {
                name: "pos.item-options".into(),
                contribution: "matrix.picker".into(),
                order: 15,
            },
            SlotDecl {
                name: "inventory.stock-form".into(),
                contribution: "matrix.stock-form".into(),
                order: 10,
            },
            SlotDecl {
                name: "inventory.stock-summary".into(),
                contribution: "matrix.stock-summary".into(),
                order: 10,
            },
        ];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[
            Migration::new(1, "matrix", include_str!("../migrations/001_matrix.sql")),
            Migration::new(
                2,
                "matrix_sale_gate",
                include_str!("../migrations/002_sale_gate.sql"),
            ),
        ];
        MIGRATIONS
    }

    fn register(&self, _registrar: &mut Registrar) {}

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(sale::MatrixSaleHook))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_wires_slots() {
        let manifest = MatrixPlugin.manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.slot_contributions.len(), 4);
        assert!(manifest.events_consumed.iter().any(|e| e == "sales.bill_posted"));
    }
}
