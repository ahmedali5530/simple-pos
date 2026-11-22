use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use plugin_pos::models::{BarcodeRule, PrinterForm, ShopConfig};

use crate::print::{print_cash_shift, print_product_mix, print_saved_bill, PrintRequest};

fn shop() -> ShopConfig {
    ShopConfig {
        name: "Shop".into(),
        currency: "PKR".into(),
        locale: "en-PK".into(),
        bill_prefix: "BILL".into(),
        barcode_rules: BarcodeRule::defaults(),
    }
}

async fn receipt(db: TenantDb, Json(body): Json<PrintRequest>) -> Result<Json<crate::print::PrintOutcome>, AppError> {
    let mut tx = db.begin(false).await?;
    let outcome = print_saved_bill(tx.transaction(), &shop(), &body.bill_no).await?;
    tx.commit().await?;
    Ok(Json(outcome))
}

#[derive(serde::Deserialize)]
struct MixPrint {
    from: Option<String>,
    to: Option<String>,
}

async fn mix(db: TenantDb, Json(body): Json<MixPrint>) -> Result<Json<crate::print::PrintOutcome>, AppError> {
    let mut tx = db.begin(true).await?;
    let outcome = print_product_mix(tx.transaction(), &shop(), body.from.as_deref(), body.to.as_deref()).await?;
    tx.commit().await?;
    Ok(Json(outcome))
}

#[derive(serde::Deserialize)]
struct ShiftPrint {
    id: i32,
}

async fn cash(db: TenantDb, Json(body): Json<ShiftPrint>) -> Result<Json<crate::print::PrintOutcome>, AppError> {
    let mut tx = db.begin(true).await?;
    let outcome = print_cash_shift(tx.transaction(), &shop(), body.id).await?;
    tx.commit().await?;
    Ok(Json(outcome))
}

async fn settings(db: TenantDb, Json(body): Json<PrinterForm>) -> Result<Json<plugin_pos::models::PosSnapshot>, AppError> {
    let mut tx = db.begin(false).await?;
    let pos = plugin_pos::admin::save_printer(tx.transaction(), &shop(), &body).await?;
    tx.commit().await?;
    Ok(Json(pos))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.post("receipt", receipt, "printing.use")?;
    routes.post("mix", mix, "printing.use")?;
    routes.post("cash", cash, "printing.use")?;
    routes.put("settings", settings, "settings.write")?;
    Ok(())
}
