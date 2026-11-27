use axum::extract::Query;
use axum::response::Response;
use axum::Json;
use inventory_contract::InventoryReportsProvider;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use sales_contract::SalesReportingProvider;
use serde::Deserialize;
use serde_json::json;

use crate::export::{self, ReportFormat, ReportKind};

#[derive(Deserialize)]
struct ReportQuery {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Deserialize)]
struct ExportQuery {
    kind: String,
    format: String,
    from: Option<String>,
    to: Option<String>,
}

async fn summary(
    db: TenantDb,
    Query(query): Query<ReportQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sales = db
        .provider::<SalesReportingProvider>()
        .ok_or_else(|| AppError::Conflict("Point of Sale is not enabled.".into()))?;
    let stock = db
        .provider::<InventoryReportsProvider>()
        .ok_or_else(|| AppError::Conflict("Inventory is not enabled.".into()))?;
    let mut tx = db.begin(true).await?;
    let sales = sales
        .0
        .report(&mut tx, query.from.as_deref(), query.to.as_deref())
        .await?;
    let stock = stock
        .0
        .stock_report(&mut tx, query.from.as_deref(), query.to.as_deref())
        .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "sections": sales.sections,
        "bills": sales.bills,
        "movements": stock.movements,
        "bill_count": sales.bill_count,
        "total": sales.total,
        "stock_units": stock.stock_units,
        "stock_value": stock.stock_value,
        "profit": sales.profit,
        "bills_truncated": sales.bills_truncated,
        "movements_truncated": stock.movements_truncated,
        "mix": stock.mix,
        "mix_truncated": stock.mix_truncated,
        "range_label": sales.range_label,
    })))
}

async fn export_report(
    db: TenantDb,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AppError> {
    let kind = ReportKind::parse(&query.kind)?;
    let format = ReportFormat::parse(&query.format)?;
    let bytes = export::export_report(
        &db,
        kind,
        format,
        query.from.as_deref(),
        query.to.as_deref(),
    )
    .await?;
    let filename = format!("{}.{}", kind.file_stem(), format.extension());
    Ok(export::file_response(
        &filename,
        format.content_type(),
        bytes,
    ))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("summary", summary, "reports.read")?;
    routes.get("export", export_report, "reports.read")?;
    Ok(())
}
