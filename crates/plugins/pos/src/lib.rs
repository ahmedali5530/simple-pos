use std::sync::Arc;

use async_trait::async_trait;
use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityKind};
use platform_kernel::context::TenantTx;
use platform_kernel::contract::{DeletionVeto, Registrar};
use platform_kernel::error::AppError;
use platform_kernel::manifest::{NavItem, PluginManifest};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use sales_contract::{SalesQuery, SalesTotal};
use serde_json::json;

pub struct PosPlugin;

impl Plugin for PosPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "pos".into(),
            name: "Point of Sale".into(),
            version: "0.1.0".into(),
            description: "Billing, bills, till focus and barcode rules.".into(),
            requires: vec!["catalog".into()],
            optional: vec!["inventory".into(), "printing".into()],
            permissions: vec![
                Permission::new(builtins::POS_READ),
                Permission::new(builtins::POS_BILL),
            ],
            capabilities: vec![Capability {
                id: "pos.snapshot".into(),
                kind: CapabilityKind::Query,
                permission: builtins::POS_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"object"}),
                description: "Load the till snapshot.".into(),
            }],
            events_emitted: vec!["sales.bill_posted".into()],
            events_consumed: vec![],
            settings_schema: None,
            nav: vec![NavItem {
                id: "billing".into(),
                label: "POS Billing".into(),
                href: "/billing".into(),
                permission: Some(builtins::POS_BILL.into()),
                order: 10,
            }],
            settings_pages: vec![],
            report_pages: vec![],
        }
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide(Arc::new(PgSalesQuery));
        registrar.deletion_veto(Arc::new(PosDeletionVeto));
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

struct PosDeletionVeto;

#[async_trait]
impl DeletionVeto for PosDeletionVeto {
    async fn check(
        &self,
        tx: &mut TenantTx<'_>,
        entity: &str,
        id: i32,
    ) -> Result<Option<String>, AppError> {
        if entity == "section" {
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
