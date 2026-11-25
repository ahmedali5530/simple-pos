use axum::Json;
use cash_contract::CashClosingProvider;
use inventory_contract::InventoryReportsProvider;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use sales_contract::SalesReceiptProvider;

use crate::print::{print_cash_shift, print_product_mix, print_saved_bill, PrintRequest};

async fn receipt(
    db: TenantDb,
    Json(body): Json<PrintRequest>,
) -> Result<Json<crate::print::PrintOutcome>, AppError> {
    let bill_no = body.bill_no.trim();
    if bill_no.is_empty() {
        return Err(AppError::BadRequest("That bill has no number.".into()));
    }
    let provider = db.provider::<SalesReceiptProvider>().ok_or_else(|| {
        AppError::BadRequest("The POS plugin is required to print a bill.".into())
    })?;
    let mut tx = db.begin(false).await?;
    let profile = platform_runtime::shop_profile::load_tx(&mut tx).await?;
    let receipt = provider.0.receipt(&mut tx, bill_no).await?;
    let outcome = print_saved_bill(tx.transaction(), &profile, receipt).await?;
    tx.commit().await?;
    Ok(Json(outcome))
}

#[derive(serde::Deserialize)]
struct MixPrint {
    from: Option<String>,
    to: Option<String>,
}

async fn mix(
    db: TenantDb,
    Json(body): Json<MixPrint>,
) -> Result<Json<crate::print::PrintOutcome>, AppError> {
    let provider = db.provider::<InventoryReportsProvider>().ok_or_else(|| {
        AppError::BadRequest("The Inventory plugin is required to print product mix.".into())
    })?;
    let mut tx = db.begin(true).await?;
    let profile = platform_runtime::shop_profile::load_tx(&mut tx).await?;
    let mix = provider
        .0
        .product_mix(&mut tx, body.from.as_deref(), body.to.as_deref())
        .await?;
    let outcome = print_product_mix(tx.transaction(), &profile, mix).await?;
    tx.commit().await?;
    Ok(Json(outcome))
}

#[derive(serde::Deserialize)]
struct ShiftPrint {
    id: i32,
}

async fn cash(
    db: TenantDb,
    Json(body): Json<ShiftPrint>,
) -> Result<Json<crate::print::PrintOutcome>, AppError> {
    let provider = db.provider::<CashClosingProvider>().ok_or_else(|| {
        AppError::BadRequest("The Cash plugin is required to print a closing.".into())
    })?;
    let mut tx = db.begin(true).await?;
    let profile = platform_runtime::shop_profile::load_tx(&mut tx).await?;
    let shift = provider.0.closing(&mut tx, body.id).await?;
    let outcome = print_cash_shift(tx.transaction(), &profile, shift).await?;
    tx.commit().await?;
    Ok(Json(outcome))
}

async fn settings(db: TenantDb) -> Result<Json<crate::print::PrinterSettings>, AppError> {
    let mut tx = db.begin(true).await?;
    let settings = crate::print::load_printer_settings(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(settings))
}

async fn save_settings(
    db: TenantDb,
    Json(body): Json<crate::print::PrinterForm>,
) -> Result<Json<crate::print::PrinterSettings>, AppError> {
    let settings = body.checked()?;
    let mut tx = db.begin(false).await?;
    sqlx::query(
        "INSERT INTO printing_settings (tenant_id, enabled, kind, target, port) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (tenant_id) DO UPDATE SET enabled = EXCLUDED.enabled, kind = EXCLUDED.kind, target = EXCLUDED.target, port = EXCLUDED.port",
    ).bind(tx.tenant_id.as_uuid()).bind(settings.enabled).bind(&settings.kind).bind(&settings.target).bind(settings.port)
        .execute(tx.conn()).await?;
    tx.commit().await?;
    Ok(Json(settings))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.post("receipt", receipt, "printing.use")?;
    routes.post("mix", mix, "printing.use")?;
    routes.post("cash", cash, "printing.use")?;
    routes.get("settings", settings, "settings.write")?;
    routes.put("settings", save_settings, "settings.write")?;
    Ok(())
}
