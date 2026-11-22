use std::sync::Arc;

use async_trait::async_trait;
use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityKind};
use platform_kernel::context::TenantTx;
use platform_kernel::contract::{DeletionVeto, Registrar};
use platform_kernel::error::AppError;
use platform_kernel::manifest::{Dependency, NavItem, PluginManifest};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use catalog_contract::CatalogApi;
use sales_contract::{SalesQuery, SalesTotal};
use serde_json::json;

mod routes;

pub struct PosPlugin;

impl Plugin for PosPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("pos", "Point of Sale", "0.1.0");
        manifest.description = "Billing, bills, till focus and barcode rules.".into();
        manifest.requires = vec![Dependency::new("catalog", "^0.1")];
        manifest.optional = vec![
            Dependency::new("inventory", "^0.1"),
            Dependency::new("printing", "^0.1"),
        ];
        manifest.permissions = vec![
            Permission::new(builtins::POS_READ),
            Permission::new(builtins::POS_BILL),
        ];
        manifest.capabilities = vec![Capability {
            id: "pos.snapshot".into(),
            kind: CapabilityKind::Query,
            permission: builtins::POS_READ.into(),
            effect: CapabilityEffect::Read,
            requires_confirmation: false,
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"object"}),
            description: "Load the till snapshot.".into(),
        }];
        manifest.events_emitted = vec!["sales.bill_posted".into()];
        manifest.nav = vec![NavItem {
            id: "billing".into(),
            label: "POS Billing".into(),
            href: "/billing".into(),
            permission: Some(builtins::POS_BILL.into()),
            order: 10,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn register(&self, registrar: &mut Registrar) {
        let catalog = registrar
            .get::<CatalogApi>()
            .expect("catalog is registered before pos")
            .as_ref()
            .clone();
        registrar.provide(Arc::new(PgSalesQuery));
        registrar.deletion_veto(Arc::new(PosDeletionVeto { catalog }));
    }
}

struct PgSalesQuery;

#[async_trait]
impl SalesQuery for PgSalesQuery {
    async fn total_since(
        &self,
        tx: &mut TenantTx<'_>,
        since_iso: &str,
    ) -> Result<SalesTotal, AppError> {
        let (total, bill_count): (i64, i64) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(total), 0)::bigint, COUNT(*)::bigint
            FROM bills
            WHERE created_at >= $1::timestamptz
            "#,
        )
        .bind(since_iso)
        .fetch_one(tx.conn())
        .await?;
        Ok(SalesTotal {
            from: Some(since_iso.into()),
            total,
            bill_count,
        })
    }
}

struct PosDeletionVeto {
    catalog: CatalogApi,
}

#[async_trait]
impl DeletionVeto for PosDeletionVeto {
    async fn check(
        &self,
        tx: &mut TenantTx<'_>,
        entity: &str,
        id: i32,
    ) -> Result<Option<String>, AppError> {
        if entity == "section" {
            let sections = self.catalog.0.list_sections(tx).await?;
            if !sections.iter().any(|section| section.id == id) {
                return Ok(None);
            }
            let bills: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bills WHERE section_id = $1")
                .bind(id)
                .fetch_one(tx.conn())
                .await?;
            if bills > 0 {
                return Ok(Some(
                    "This section has bills, so it stays on record.".into(),
                ));
            }
        }
        if entity == "user" {
            let sales: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bills WHERE user_id = $1")
                .bind(id)
                .fetch_one(tx.conn())
                .await?;
            if sales > 0 {
                return Ok(Some(
                    "This user has sales, so the account stays on record.".into(),
                ));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_kernel::plugin::Plugin;

    #[test]
    fn bill_routes_are_registered_on_the_pos_plugin() {
        let plugin = PosPlugin;
        let manifest = plugin.manifest();
        manifest.validate().unwrap();
        let declared = manifest.permissions.iter().map(|p| p.as_str().to_string()).collect();
        let mut routes = PluginRoutes::new(manifest.id.clone(), declared);
        plugin.routes(&mut routes).unwrap();
        let (_router, specs) = routes.finish();
        let paths: Vec<_> = specs.iter().map(|spec| spec.path.as_str()).collect();
        assert!(paths.contains(&"/api/v1/pos/bills"));
        assert!(paths.contains(&"/api/v1/pos/snapshot"));
        assert!(paths.iter().all(|path| path.starts_with("/api/v1/pos/")));
    }
}
