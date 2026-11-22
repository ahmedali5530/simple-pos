use std::sync::Arc;

use async_trait::async_trait;
use catalog_contract::CatalogApi;
use inventory_contract::{StockBalance, StockIssue, StockLedger};
use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityHandler, CapabilityKind};
use platform_kernel::context::TenantTx;
use platform_kernel::contract::Registrar;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::{Dependency, NavItem, PluginManifest};
use platform_kernel::routes::RegistrationError;
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use serde_json::json;

pub mod ledger;
mod routes;
pub mod sale;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("inventory", "Inventory", "0.1.0");
        manifest.description = "Stock balances, average cost and movements.".into();
        manifest.requires = vec![Dependency::new("catalog", "^0.1")];
        manifest.permissions = vec![
            Permission::new("inventory.read"),
            Permission::new("inventory.write"),
        ];
        manifest.capabilities = vec![
            Capability {
                id: "inventory.summary".into(),
                kind: CapabilityKind::Query,
                permission: "inventory.read".into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object","properties":{}}),
                output_schema: json!({"type":"array"}),
                description: "Stock summary for all items.".into(),
            },
            Capability {
                id: "inventory.adjust".into(),
                kind: CapabilityKind::Command,
                permission: "inventory.write".into(),
                effect: CapabilityEffect::Write,
                requires_confirmation: true,
                input_schema: json!({
                    "type":"object",
                    "required":["item_id","qty","direction"],
                    "properties":{
                        "item_id":{"type":"integer"},
                        "qty":{"type":"integer","minimum":1},
                        "direction":{"type":"string","enum":["in","out"]},
                        "confirm":{"type":"boolean"}
                    }
                }),
                output_schema: json!({"type":"object"}),
                description: "Adjust stock (requires confirm: true).".into(),
            },
        ];
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.nav = vec![NavItem {
            id: "stock".into(),
            label: "Stock In / Out".into(),
            href: "/stock".into(),
            permission: Some("inventory.write".into()),
            order: 20,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn routes(&self, routes: &mut platform_kernel::routes::PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(OnBillPosted))?;
        Ok(())
    }

    fn register(&self, registrar: &mut Registrar) {
        let catalog = registrar
            .get::<CatalogApi>()
            .expect("catalog is registered before inventory")
            .as_ref()
            .clone();
        registrar.provide(Arc::new(PgStockLedger {
            catalog: catalog.clone(),
        }));
        registrar.capability(
            Capability {
                id: "inventory.summary".into(),
                kind: CapabilityKind::Query,
                permission: "inventory.read".into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"array"}),
                description: "Stock summary".into(),
            },
            Arc::new(SummaryCapability),
        );
        registrar.capability(
            Capability {
                id: "inventory.adjust".into(),
                kind: CapabilityKind::Command,
                permission: "inventory.write".into(),
                effect: CapabilityEffect::Write,
                requires_confirmation: true,
                input_schema: json!({
                    "type":"object",
                    "required":["item_id","qty","direction","confirm"],
                    "properties":{
                        "item_id":{"type":"integer"},
                        "qty":{"type":"integer"},
                        "direction":{"type":"string"},
                        "confirm":{"type":"boolean"}
                    }
                }),
                output_schema: json!({"type":"object"}),
                description: "Adjust stock".into(),
            },
            Arc::new(AdjustCapability { catalog }),
        );
    }
}

struct OnBillPosted;

#[async_trait]
impl EventHandler for OnBillPosted {
    fn event_type(&self) -> &str {
        "sales.bill_posted"
    }

    fn subscriber_id(&self) -> &str {
        "sale"
    }

    async fn handle(&self, tx: &mut TenantTx<'_>, envelope: &EventEnvelope) -> Result<(), AppError> {
        let Some(bill_no) = envelope.payload.get("bill_no").and_then(|value| value.as_str()) else {
            return Ok(());
        };
        sale::apply_bill(tx.transaction(), bill_no).await
    }
}

struct PgStockLedger {
    catalog: CatalogApi,
}

impl PgStockLedger {
    async fn ensure_stocked(&self, tx: &mut TenantTx<'_>, item_id: i32) -> Result<(), AppError> {
        if !self.catalog.0.is_stock_tracked(tx, item_id).await? {
            return Err(AppError::BadRequest(
                "Services are not tracked in stock.".into(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl StockLedger for PgStockLedger {
    async fn issue(&self, tx: &mut TenantTx<'_>, issues: &[StockIssue]) -> Result<(), AppError> {
        let mut ids: Vec<i32> = issues.iter().map(|i| i.item_id).collect();
        ids.sort_unstable();
        ids.dedup();
        for item_id in ids {
            self.ensure_stocked(tx, item_id).await?;
            sqlx::query("SELECT id FROM items WHERE id = $1 FOR UPDATE")
                .bind(item_id)
                .fetch_optional(tx.conn())
                .await?;
        }
        for issue in issues {
            let updated = sqlx::query(
                r#"
                UPDATE items SET stock = stock - $1
                WHERE id = $2 AND stock >= $1
                "#,
            )
            .bind(issue.qty)
            .bind(issue.item_id)
            .execute(tx.conn())
            .await?;
            if updated.rows_affected() == 0 {
                return Err(AppError::BadRequest(format!(
                    "Not enough stock for item {}.",
                    issue.item_id
                )));
            }
            sqlx::query(
                r#"
                UPDATE inventory_balances SET stock = stock - $1
                WHERE item_id = $2 AND stock >= $1
                "#,
            )
            .bind(issue.qty)
            .bind(issue.item_id)
            .execute(tx.conn())
            .await
            .ok();
            sqlx::query(
                r#"
                INSERT INTO stock_movements (item_id, qty, reason, source_type, source_id)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(issue.item_id)
            .bind(-issue.qty)
            .bind(&issue.reason)
            .bind(&issue.source_type)
            .bind(issue.source_id)
            .execute(tx.conn())
            .await?;
        }
        Ok(())
    }

    async fn receive(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
        qty: i32,
        total_major: Option<i64>,
        reason: &str,
    ) -> Result<StockBalance, AppError> {
        self.ensure_stocked(tx, item_id).await?;
        let held: Option<(i32, i64)> =
            sqlx::query_as("SELECT stock, avg_cost FROM items WHERE id = $1 AND active FOR UPDATE")
                .bind(item_id)
                .fetch_optional(tx.conn())
                .await?;
        let Some((on_hand, average)) = held else {
            return Err(AppError::NotFound(
                "That item is no longer in the catalog.".into(),
            ));
        };
        let next_cost = total_major.map(|total| next_average(on_hand, average, qty, total));
        sqlx::query(
            "UPDATE items SET stock = stock + $1, avg_cost = COALESCE($2, avg_cost) WHERE id = $3",
        )
        .bind(qty)
        .bind(next_cost)
        .bind(item_id)
        .execute(tx.conn())
        .await?;
        sqlx::query(
            r#"
            INSERT INTO inventory_balances (tenant_id, item_id, stock, avg_cost)
            VALUES (current_setting('app.tenant_id')::uuid, $1, $2, COALESCE($3, 0))
            ON CONFLICT (tenant_id, item_id) DO UPDATE
            SET stock = inventory_balances.stock + EXCLUDED.stock,
                avg_cost = COALESCE($3, inventory_balances.avg_cost)
            "#,
        )
        .bind(item_id)
        .bind(qty)
        .bind(next_cost)
        .execute(tx.conn())
        .await
        .ok();
        sqlx::query(
            "INSERT INTO stock_movements (item_id, qty, reason, source_type) VALUES ($1, $2, $3, 'manual')",
        )
        .bind(item_id)
        .bind(qty)
        .bind(reason)
        .execute(tx.conn())
        .await?;
        let avg = next_cost.unwrap_or(average);
        Ok(StockBalance {
            item_id,
            stock: on_hand + qty,
            avg_cost: avg,
        })
    }

    async fn balance(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
    ) -> Result<Option<StockBalance>, AppError> {
        let row: Option<(i32, i64)> =
            sqlx::query_as("SELECT stock, avg_cost FROM items WHERE id = $1 AND active")
                .bind(item_id)
                .fetch_optional(tx.conn())
                .await?;
        Ok(row.map(|(stock, avg_cost)| StockBalance {
            item_id,
            stock,
            avg_cost,
        }))
    }

    async fn lock_for_sale(
        &self,
        tx: &mut TenantTx<'_>,
        item_ids: &[i32],
    ) -> Result<Vec<StockBalance>, AppError> {
        let mut ids = item_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        let mut out = Vec::new();
        for id in ids {
            let row: Option<(i32, i64)> = sqlx::query_as(
                "SELECT stock, avg_cost FROM items WHERE id = $1 AND active FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(tx.conn())
            .await?;
            if let Some((stock, avg_cost)) = row {
                out.push(StockBalance {
                    item_id: id,
                    stock,
                    avg_cost,
                });
            }
        }
        Ok(out)
    }
}

fn next_average(on_hand: i32, average: i64, qty: i32, total_major: i64) -> i64 {
    let qty = i64::from(qty);
    let unit = (total_major * 100 + qty / 2) / qty;
    if on_hand <= 0 || average <= 0 {
        return unit;
    }
    let on_hand = i64::from(on_hand);
    let next = on_hand + qty;
    (on_hand * average + qty * unit + next / 2) / next
}

struct SummaryCapability;

#[async_trait]
impl CapabilityHandler for SummaryCapability {
    async fn invoke(
        &self,
        tx: &mut TenantTx<'_>,
        _input: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let rows: Vec<(i32, String, i32, i64)> = sqlx::query_as(
            "SELECT id, name, stock, avg_cost FROM items WHERE active ORDER BY sort_order, id",
        )
        .fetch_all(tx.conn())
        .await?;
        Ok(json!(rows
            .into_iter()
            .map(|(id, name, stock, avg_cost)| {
                json!({"id": id, "name": name, "stock": stock, "avg_cost": avg_cost})
            })
            .collect::<Vec<_>>()))
    }
}

struct AdjustCapability {
    catalog: CatalogApi,
}

#[async_trait]
impl CapabilityHandler for AdjustCapability {
    async fn invoke(
        &self,
        tx: &mut TenantTx<'_>,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let item_id = input
            .get("item_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| AppError::BadRequest("item_id is required.".into()))?
            as i32;
        let qty = input
            .get("qty")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| AppError::BadRequest("qty is required.".into()))? as i32;
        let direction = input
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ledger = PgStockLedger {
            catalog: self.catalog.clone(),
        };
        match direction {
            "in" => {
                let bal = ledger.receive(tx, item_id, qty, None, "AI adjust").await?;
                Ok(json!({"item_id": bal.item_id, "stock": bal.stock}))
            }
            "out" => {
                ledger
                    .issue(
                        tx,
                        &[StockIssue {
                            item_id,
                            qty,
                            reason: "AI adjust".into(),
                            source_type: "manual".into(),
                            source_id: None,
                        }],
                    )
                    .await?;
                let bal = ledger.balance(tx, item_id).await?;
                Ok(json!({"item_id": item_id, "stock": bal.map(|b| b.stock)}))
            }
            _ => Err(AppError::BadRequest("direction must be in or out.".into())),
        }
    }
}
