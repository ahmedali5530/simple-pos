use std::sync::Arc;

use async_trait::async_trait;
use catalog_contract::{CatalogApi, CatalogQuery};
use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityHandler, CapabilityKind};
use platform_kernel::context::TenantTx;
use platform_kernel::contract::{DeletionVeto, Registrar};
use platform_kernel::error::AppError;
use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::{NavItem, PluginManifest, SettingsPage};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use serde_json::json;

mod routes;
pub mod service;

pub struct CatalogPlugin;

impl Plugin for CatalogPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.description = "Products, services, categories, units, variants and attributes.".into();
        manifest.permissions = vec![
            Permission::new(builtins::CATALOG_READ),
            Permission::new(builtins::CATALOG_WRITE),
        ];
        manifest.events_emitted = vec![
            catalog_contract::events::ITEM_SAVED.into(),
            catalog_contract::events::ITEM_REMOVED.into(),
        ];
        manifest.capabilities = vec![
            Capability {
                id: "catalog.items.list".into(),
                kind: CapabilityKind::Query,
                permission: builtins::CATALOG_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"object"}),
                description: "List active catalog items.".into(),
            },
            Capability {
                id: "catalog.items.get".into(),
                kind: CapabilityKind::Query,
                permission: builtins::CATALOG_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object","properties":{"id":{"type":"integer"}}}),
                output_schema: json!({"type":"object"}),
                description: "Read one catalog item.".into(),
            },
            Capability {
                id: "catalog.units.list".into(),
                kind: CapabilityKind::Query,
                permission: builtins::CATALOG_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"object"}),
                description: "List units of measurement.".into(),
            },
        ];
        manifest.nav = vec![NavItem {
            id: "settings".into(),
            label: "Settings".into(),
            href: "/settings/items".into(),
            permission: Some(builtins::CATALOG_WRITE.into()),
            order: 50,
        }];
        manifest.settings_pages = vec![
            SettingsPage {
                id: "items".into(),
                label: "Items".into(),
                href: "/settings/items".into(),
                permission: Some(builtins::CATALOG_WRITE.into()),
                order: 10,
            },
            SettingsPage {
                id: "sections".into(),
                label: "Sections".into(),
                href: "/settings/sections".into(),
                permission: Some(builtins::CATALOG_WRITE.into()),
                order: 20,
            },
            SettingsPage {
                id: "categories".into(),
                label: "Categories".into(),
                href: "/settings/categories".into(),
                permission: Some(builtins::CATALOG_WRITE.into()),
                order: 30,
            },
            SettingsPage {
                id: "units".into(),
                label: "Units".into(),
                href: "/settings/units".into(),
                permission: Some(builtins::CATALOG_WRITE.into()),
                order: 35,
            },
        ];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[
            Migration::new(1, "catalog_adopt", include_str!("../migrations/001_adopt.sql")),
            Migration::new(2, "catalog_units", include_str!("../migrations/002_units.sql")),
            Migration::new(3, "catalog_item_type", include_str!("../migrations/003_item_type.sql")),
            Migration::new(4, "catalog_attributes", include_str!("../migrations/004_attributes.sql")),
        ];
        MIGRATIONS
    }

    fn register(&self, registrar: &mut Registrar) {
        let query: Arc<dyn CatalogQuery> = Arc::new(service::Query);
        registrar.provide(Arc::new(CatalogApi(query)));
        registrar.deletion_veto(Arc::new(CatalogDeletionVeto));
        registrar.capability(
            Capability {
                id: "catalog.items.list".into(),
                kind: CapabilityKind::Query,
                permission: builtins::CATALOG_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"object"}),
                description: "List items".into(),
            },
            Arc::new(ListItems),
        );
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn event_handlers(&self, _events: &mut PluginEvents) -> Result<(), RegistrationError> {
        Ok(())
    }
}

struct CatalogDeletionVeto;

#[async_trait]
impl DeletionVeto for CatalogDeletionVeto {
    async fn check(
        &self,
        tx: &mut TenantTx<'_>,
        entity: &str,
        id: i32,
    ) -> Result<Option<String>, AppError> {
        if entity == "section" {
            let items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE section_id = $1")
                .bind(id)
                .fetch_one(tx.conn())
                .await?;
            if items > 0 {
                return Ok(Some("Move this section's items before removing it.".into()));
            }
        }
        if entity == "category" {
            let items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE category_id = $1")
                .bind(id)
                .fetch_one(tx.conn())
                .await?;
            if items > 0 {
                return Ok(Some("Move this category's items before removing it.".into()));
            }
        }
        Ok(None)
    }
}

struct ListItems;

#[async_trait]
impl CapabilityHandler for ListItems {
    async fn invoke(&self, tx: &mut TenantTx<'_>, _: serde_json::Value) -> Result<serde_json::Value, AppError> {
        let items = service::Query.items_by_ids(tx, &[]).await?;
        let _ = items;
        let rows: Vec<(i32, String, i32)> =
            sqlx::query_as("SELECT id, name, rate FROM items WHERE active ORDER BY sort_order, id")
                .fetch_all(tx.conn())
                .await?;
        Ok(json!({
            "items": rows.into_iter().map(|(id, name, rate)| json!({"id": id, "name": name, "rate": rate})).collect::<Vec<_>>()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid_and_routes_stay_under_catalog() {
        let plugin = CatalogPlugin;
        plugin.manifest().validate().unwrap();
        let manifest = plugin.manifest();
        let declared = manifest.permissions.iter().map(|p| p.as_str().to_string()).collect();
        let mut routes = PluginRoutes::new(manifest.id.clone(), declared);
        plugin.routes(&mut routes).unwrap();
        let (_router, specs) = routes.finish();
        assert!(specs.iter().all(|spec| spec.path.starts_with("/api/v1/catalog/")));
        assert!(specs.iter().any(|spec| spec.path == "/api/v1/catalog/items" && spec.permission == "catalog.read"));
        assert!(specs.iter().any(|spec| spec.permission == "catalog.write"));
    }

    #[test]
    fn services_are_not_stock_tracked() {
        assert!(service::tracks_stock("product"));
        assert!(!service::tracks_stock("service"));
    }
}
