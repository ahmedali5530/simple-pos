use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use sales_contract::SalesReceiptProvider;
use serde::Deserialize;

use crate::authority::SubmitLine;
use crate::settings::{self, FiscalSettingsForm};
use crate::submit;

async fn get_settings(db: TenantDb) -> Result<Json<settings::FiscalSettings>, AppError> {
    let mut tx = db.begin(true).await?;
    let settings = settings::load(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(settings))
}

async fn put_settings(
    db: TenantDb,
    Json(body): Json<FiscalSettingsForm>,
) -> Result<Json<settings::FiscalSettings>, AppError> {
    let settings = body.checked()?;
    let mut tx = db.begin(false).await?;
    settings::save(&mut tx, &settings).await?;
    tx.commit().await?;
    Ok(Json(settings))
}

async fn list_submissions(db: TenantDb) -> Result<Json<Vec<submit::FiscalSubmission>>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows = submit::list_recent(&mut tx, 100).await?;
    tx.commit().await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct RetryBody {
    bill_no: String,
}

async fn retry_submission(
    db: TenantDb,
    Json(body): Json<RetryBody>,
) -> Result<Json<submit::FiscalSubmission>, AppError> {
    let bill_no = body.bill_no.trim();
    if bill_no.is_empty() {
        return Err(AppError::BadRequest("Enter a bill number to retry.".into()));
    }
    let receipts = db.provider::<SalesReceiptProvider>();
    let mut tx = db.begin(false).await?;
    sqlx::query("DELETE FROM fiscal_submissions WHERE bill_no = $1 AND status = 'failed'")
        .bind(bill_no)
        .execute(tx.conn())
        .await?;
    if let Some(existing) = submit::load_ok(&mut tx, bill_no).await? {
        tx.commit().await?;
        return Ok(Json(existing));
    }
    let Some(receipts) = receipts else {
        return Err(AppError::BadRequest(
            "The POS plugin is required to retry a fiscal submission.".into(),
        ));
    };
    let data = receipts.0.receipt(&mut tx, bill_no).await?;
    let row = submit::ensure_for_bill(
        &mut tx,
        data.bill_id,
        bill_no,
        data.total,
        &data.sold_at,
        data.customer.as_deref(),
        data.lines
            .iter()
            .map(|line| SubmitLine {
                label: line.name.clone(),
                quantity: line.qty.to_string(),
                amount: i64::from(line.amount),
            })
            .collect(),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(row))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("settings", get_settings, "fiscal.write")?;
    routes.put("settings", put_settings, "fiscal.write")?;
    routes.get("submissions", list_submissions, "fiscal.read")?;
    routes.post("retry", retry_submission, "fiscal.write")?;
    Ok(())
}
