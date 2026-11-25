pub mod admin;
pub mod barcode;
pub mod billing;
pub mod models;
pub mod tenant;

use std::sync::Arc;

use async_trait::async_trait;
use catalog_contract::CatalogApi;
use platform_kernel::context::TenantTx;
use platform_kernel::contract::Registrar;
use platform_kernel::error::AppError;
use platform_kernel::manifest::{Dependency, NavItem, PluginManifest};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use sales_contract::{
    ReceiptData, ReceiptLine, SalesBill, SalesBillLine, SalesQuery, SalesQueryProvider,
    SalesReceiptProvider, SalesReceiptQuery, SalesReport, SalesReporting, SalesReportingProvider,
    SalesSectionTotal, SalesTotal,
};

mod routes;

pub struct PosPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> std::sync::Arc<dyn Plugin> {
    std::sync::Arc::new(PosPlugin)
}

impl Plugin for PosPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("pos", "Point of Sale", "0.1.1");
        manifest.default_enabled = true;
        manifest.description = "Billing, bills, till focus and barcode rules.".into();
        // Catalog and inventory are optional providers. Another plugin can supply
        // CatalogApi / StockLedger; POS stays enableable on its own.
        manifest.optional = vec![
            Dependency::new("catalog", "^0.1"),
            Dependency::new("inventory", "^0.1"),
        ];
        manifest.permissions = vec![
            Permission::new("pos.read"),
            Permission::new("pos.bills.post"),
            Permission::new("settings.write"),
        ];
        manifest.events_emitted = vec!["sales.bill_posted".into()];
        manifest.nav = vec![NavItem {
            id: "billing".into(),
            label: "POS Billing".into(),
            href: "/billing".into(),
            permission: Some("pos.bills.post".into()),
            order: 10,
            children: Vec::new(),
        }];
        manifest.slots_provided = vec![
            "pos.line-source".into(),
            "pos.line-detail".into(),
            "pos.item-options".into(),
        ];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[
            Migration::new(
                1,
                "line_source",
                include_str!("../migrations/001_line_source.sql"),
            ),
            Migration::new(
                2,
                "line_cost",
                include_str!("../migrations/002_line_cost.sql"),
            ),
        ];
        MIGRATIONS
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide(Arc::new(SalesQueryProvider(Arc::new(PgSalesQuery))));
        registrar.provide(Arc::new(SalesReceiptProvider(Arc::new(
            PgSalesReceiptQuery,
        ))));
        let catalog = registrar.get::<CatalogApi>().map(|api| api.as_ref().clone());
        registrar.provide(Arc::new(SalesReportingProvider(Arc::new(
            PgSalesReporting { catalog },
        ))));
    }
}

struct PgSalesReceiptQuery;

#[async_trait]
impl SalesReceiptQuery for PgSalesReceiptQuery {
    async fn receipt(&self, tx: &mut TenantTx<'_>, bill_no: &str) -> Result<ReceiptData, AppError> {
        #[derive(sqlx::FromRow)]
        struct Head {
            id: i32,
            bill_no: String,
            total: i32,
            sold_at: String,
            cashier: String,
            customer: Option<String>,
        }
        #[derive(sqlx::FromRow)]
        struct Line {
            name: String,
            qty: i32,
            rate: i32,
            amount: i32,
        }
        let head = sqlx::query_as::<_, Head>(r#"
            SELECT b.id, b.bill_no, b.total,
                   to_char(b.created_at AT TIME ZONE 'Asia/Karachi', 'YYYY-MM-DD HH12:MI AM') AS sold_at,
                   COALESCE(u.name, '') AS cashier, b.customer
            FROM bills b LEFT JOIN users u ON u.id = b.user_id WHERE b.bill_no = $1
        "#).bind(bill_no).fetch_optional(tx.conn()).await?
            .ok_or_else(|| AppError::NotFound("That bill is not in the till.".into()))?;
        let lines = sqlx::query_as::<_, Line>(
            "SELECT name, qty, rate, amount FROM bill_lines WHERE bill_id = $1 ORDER BY id",
        )
        .bind(head.id)
        .fetch_all(tx.conn())
        .await?;
        Ok(ReceiptData {
            bill_no: head.bill_no,
            total: head.total,
            sold_at: head.sold_at,
            cashier: if head.cashier.is_empty() {
                "Cashier".into()
            } else {
                head.cashier
            },
            customer: head.customer,
            lines: lines
                .into_iter()
                .map(|line| ReceiptLine {
                    name: line.name,
                    qty: line.qty,
                    rate: line.rate,
                    amount: line.amount,
                })
                .collect(),
        })
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

    async fn bill_count_for_section(
        &self,
        tx: &mut TenantTx<'_>,
        section_id: i32,
    ) -> Result<i64, AppError> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM bills WHERE section_id = $1")
                .bind(section_id)
                .fetch_one(tx.conn())
                .await?,
        )
    }
}

struct PgSalesReporting {
    catalog: Option<CatalogApi>,
}

#[derive(sqlx::FromRow)]
struct SalesBillRow {
    id: i32,
    bill_no: String,
    section_id: i32,
    total: i32,
    created_at: String,
    cashier_name: Option<String>,
    customer: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SalesLineRow {
    bill_id: i32,
    name: String,
    qty: i32,
    rate: i32,
    amount: i32,
    unit_cost: i64,
}

#[derive(sqlx::FromRow)]
struct SalesCostRow {
    bill_id: i32,
    cost: i64,
}

#[async_trait]
impl SalesReporting for PgSalesReporting {
    async fn report(
        &self,
        tx: &mut TenantTx<'_>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<SalesReport, AppError> {
        if from.is_some() != to.is_some() {
            return Err(AppError::BadRequest("Choose a start and an end.".into()));
        }
        let totals: Vec<(i32, i64, i64)> = sqlx::query_as(
            r#"
            SELECT section_id, COUNT(*)::bigint, COALESCE(SUM(total), 0)::bigint
            FROM bills
            WHERE ($1::text IS NULL OR created_at >= $1::timestamptz)
              AND ($2::text IS NULL OR created_at < $2::timestamptz)
            GROUP BY section_id
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(tx.conn())
        .await?;
        let totals: std::collections::HashMap<i32, (i64, i64)> = totals
            .into_iter()
            .map(|(section_id, bills, total)| (section_id, (bills, total)))
            .collect();
        let sections: Vec<SalesSectionTotal> = if let Some(catalog) = &self.catalog {
            let snapshot = catalog.0.snapshot(tx).await?;
            snapshot
                .sections
                .iter()
                .map(|section| {
                    let (bills, total) = totals.get(&section.id).copied().unwrap_or((0, 0));
                    SalesSectionTotal {
                        id: section.id,
                        name: section.name.clone(),
                        short_name: section.short_name.clone(),
                        accent: section.accent.clone(),
                        bills,
                        total,
                    }
                })
                .collect()
        } else {
            let mut sections: Vec<_> = totals
                .iter()
                .map(|(id, (bills, total))| SalesSectionTotal {
                    id: *id,
                    name: format!("Section {id}"),
                    short_name: format!("S{id}"),
                    accent: String::new(),
                    bills: *bills,
                    total: *total,
                })
                .collect();
            sections.sort_by_key(|section| section.id);
            sections
        };
        let bill_count: i64 = sections.iter().map(|section| section.bills).sum();
        let total: i64 = sections.iter().map(|section| section.total).sum();
        let goods = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COALESCE(SUM(bl.cost_total), 0)::bigint
            FROM bill_lines bl
            JOIN bills b ON b.id = bl.bill_id
            WHERE ($1::text IS NULL OR b.created_at >= $1::timestamptz)
              AND ($2::text IS NULL OR b.created_at < $2::timestamptz)
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_one(tx.conn())
        .await?;
        let rows = sqlx::query_as::<_, SalesBillRow>(
            r#"
            SELECT b.id, b.bill_no, b.section_id, b.total,
                   to_char(b.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at,
                   u.name AS cashier_name, b.customer
            FROM bills b
            LEFT JOIN users u ON u.id = b.user_id
            WHERE ($1::text IS NULL OR b.created_at >= $1::timestamptz)
              AND ($2::text IS NULL OR b.created_at < $2::timestamptz)
            ORDER BY b.created_at DESC, b.id DESC
            LIMIT 200
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(tx.conn())
        .await?;
        let bill_ids: Vec<i32> = rows.iter().map(|bill| bill.id).collect();
        let line_rows = if bill_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as::<_, SalesLineRow>(
                "SELECT bill_id, name, qty, rate, amount, unit_cost FROM bill_lines WHERE bill_id = ANY($1) ORDER BY id",
            )
            .bind(&bill_ids)
            .fetch_all(tx.conn())
            .await?
        };
        let cost_rows = if bill_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as::<_, SalesCostRow>(
                "SELECT bill_id, COALESCE(SUM(cost_total), 0)::bigint AS cost FROM bill_lines WHERE bill_id = ANY($1) GROUP BY bill_id",
            )
            .bind(&bill_ids)
            .fetch_all(tx.conn())
            .await?
        };
        let section_info: std::collections::HashMap<i32, (String, String, String)> = sections
            .iter()
            .map(|section| {
                (
                    section.id,
                    (
                        section.name.clone(),
                        section.short_name.clone(),
                        section.accent.clone(),
                    ),
                )
            })
            .collect();
        let bills = rows
            .into_iter()
            .map(|bill| {
                let (section_name, short_name, accent) = section_info
                    .get(&bill.section_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        (
                            format!("Section {}", bill.section_id),
                            format!("S{}", bill.section_id),
                            String::new(),
                        )
                    });
                SalesBill {
                    lines: line_rows
                        .iter()
                        .filter(|line| line.bill_id == bill.id)
                        .map(|line| SalesBillLine {
                            name: line.name.clone(),
                            qty: line.qty,
                            rate: line.rate,
                            amount: line.amount,
                            unit_cost: line.unit_cost,
                        })
                        .collect(),
                    id: bill.id,
                    bill_no: bill.bill_no,
                    section_name,
                    short_name,
                    accent,
                    total: bill.total,
                    cost: cost_rows
                        .iter()
                        .find(|row| row.bill_id == bill.id)
                        .map(|row| row.cost)
                        .unwrap_or(0),
                    created_at: bill.created_at,
                    cashier_name: bill.cashier_name,
                    customer: bill.customer,
                }
            })
            .collect();
        let range_label = if from.is_none() || to.is_none() {
            "All dates".to_string()
        } else {
            sqlx::query_scalar::<_, String>(
                r#"
                SELECT to_char($1::timestamptz AT TIME ZONE 'Asia/Karachi', 'DD Mon YYYY')
                    || ' – ' ||
                    to_char(($2::timestamptz - interval '1 second') AT TIME ZONE 'Asia/Karachi', 'DD Mon YYYY')
                "#,
            )
            .bind(from)
            .bind(to)
            .fetch_one(tx.conn())
            .await?
        };
        Ok(SalesReport {
            sections,
            bills,
            bill_count,
            total,
            profit: total.saturating_mul(100) - goods,
            bills_truncated: bill_count > 200,
            range_label,
        })
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
        let declared = manifest
            .permissions
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        let mut routes = PluginRoutes::new(manifest.id.clone(), declared);
        plugin.routes(&mut routes).unwrap();
        let (_router, specs) = routes.finish();
        let paths: Vec<_> = specs.iter().map(|spec| spec.path.as_str()).collect();
        assert!(paths.contains(&"/api/v1/pos/bills"));
        assert!(paths.contains(&"/api/v1/pos/snapshot"));
        assert!(paths.iter().all(|path| path.starts_with("/api/v1/pos/")));
    }
}
