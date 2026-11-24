use std::sync::Arc;

use async_trait::async_trait;
use axum::Json;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::{NavItem, PluginManifest, SlotDecl, SwitchConsent};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct InstallmentsPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> std::sync::Arc<dyn Plugin> {
    std::sync::Arc::new(InstallmentsPlugin)
}

impl Plugin for InstallmentsPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("installments", "Installments", "0.1.0");
        manifest.description = "Agreements, schedules, and payments.".into();
        manifest.consents.enable = Some(SwitchConsent {
            title: "Turn on installment agreements".into(),
            body: "This shop will record payment schedules. Turn this on only if customers buy on installments.".into(),
            acknowledge: true,
        });
        manifest.consents.disable = Some(SwitchConsent {
            title: "Turn off installment agreements".into(),
            body: "New agreements stop. Schedules already saved stay on record and can be turned back on later.".into(),
            acknowledge: false,
        });
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.permissions = vec![
            Permission::new("installments.read"),
            Permission::new("installments.write"),
        ];
        manifest.nav = vec![NavItem {
            id: "installments".into(),
            label: "Installments".into(),
            href: "/installments".into(),
            permission: Some("installments.read".into()),
            order: 47,
            children: Vec::new(),
        }];
        manifest.slot_contributions = vec![SlotDecl {
            name: "pos.tender".into(),
            contribution: "installments.schedule".into(),
            order: 30,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[Migration::new(
            1,
            "agreements",
            include_str!("../migrations/001_agreements.sql"),
        )];
        MIGRATIONS
    }

    fn register(&self, _registrar: &mut platform_kernel::contract::Registrar) {}

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(RecordPostedSale))?;
        Ok(())
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes.get("agreements", list, "installments.read")?;
        routes.post("agreements", create, "installments.write")?;
        Ok(())
    }
}

struct RecordPostedSale;

#[async_trait]
impl EventHandler for RecordPostedSale {
    fn event_type(&self) -> &str {
        "sales.bill_posted"
    }

    fn subscriber_id(&self) -> &str {
        "agreements"
    }

    async fn handle(&self, _tx: &mut TenantTx<'_>, _envelope: &EventEnvelope) -> Result<(), AppError> {
        Ok(())
    }
}

async fn list(db: TenantDb) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows: Vec<(i32, String, i64, i64, String)> = sqlx::query_as(
        "SELECT id, customer_name, total_amount, paid_amount, status FROM installment_agreements ORDER BY id DESC",
    )
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, customer_name, total_amount, paid_amount, status)| {
            json!({ "id": id, "customer_name": customer_name, "total_amount": total_amount, "paid_amount": paid_amount, "status": status })
        })
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct NewAgreement {
    customer_name: String,
    total_amount: i64,
}

async fn create(db: TenantDb, Json(body): Json<NewAgreement>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO installment_agreements (customer_name, total_amount) VALUES ($1, $2) RETURNING id",
    )
    .bind(body.customer_name.trim())
    .bind(body.total_amount)
    .fetch_one(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}
