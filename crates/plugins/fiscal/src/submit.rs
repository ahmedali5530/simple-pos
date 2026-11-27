//! Idempotent fiscal submission for a posted bill.

use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use sales_contract::BillPostedV4;
use serde::Serialize;
use sqlx::FromRow;

use crate::authority::{self, SubmitLine, SubmitRequest};
use crate::settings::{self, FiscalSettings};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FiscalSubmission {
    pub id: i32,
    pub bill_id: i32,
    pub bill_no: String,
    pub authority: String,
    pub invoice_no: String,
    pub qr_payload: String,
    pub status: String,
    pub error_message: String,
    pub submitted_at: String,
}

/// Ensure a bill has a fiscal row. Successful rows are reused; failures are retried.
pub async fn ensure_for_bill(
    tx: &mut TenantTx<'_>,
    bill_id: i32,
    bill_no: &str,
    total: i32,
    posted_at: &str,
    customer: Option<&str>,
    lines: Vec<SubmitLine>,
) -> Result<FiscalSubmission, AppError> {
    if let Some(existing) = load_ok(tx, bill_no).await? {
        return Ok(existing);
    }

    let cfg = settings::load(tx).await?;
    let shop_name = platform_runtime::shop_profile::load_tx(tx)
        .await
        .map(|profile| profile.name)
        .unwrap_or_default();
    let request = SubmitRequest {
        bill_id,
        bill_no: bill_no.to_string(),
        total,
        posted_at: posted_at.to_string(),
        customer: customer.map(str::to_string),
        lines,
        shop_name,
    };

    match authority::submit(&cfg, &request).await {
        Ok(result) => {
            upsert(
                tx,
                &cfg,
                bill_id,
                bill_no,
                &result.invoice_no,
                &result.qr_payload,
                "ok",
                "",
                &result.raw,
            )
            .await?;
            load_ok(tx, bill_no)
                .await?
                .ok_or_else(|| AppError::BadRequest("Fiscal submission did not save.".into()))
        }
        Err(error) => {
            let message = error.to_string();
            let _ = upsert(
                tx,
                &cfg,
                bill_id,
                bill_no,
                "",
                "",
                "failed",
                &message,
                &serde_json::json!({ "error": message }),
            )
            .await;
            Err(error)
        }
    }
}

pub async fn ensure_from_event(
    tx: &mut TenantTx<'_>,
    event: &BillPostedV4,
) -> Result<FiscalSubmission, AppError> {
    let lines = event
        .lines
        .iter()
        .map(|line| SubmitLine {
            label: line.label.clone(),
            quantity: line.quantity.clone(),
            amount: line.amount,
        })
        .collect();
    ensure_for_bill(
        tx,
        event.bill_id,
        &event.bill_no,
        event.total,
        &event.posted_at,
        event.customer.as_deref(),
        lines,
    )
    .await
}

pub async fn load_ok(tx: &mut TenantTx<'_>, bill_no: &str) -> Result<Option<FiscalSubmission>, AppError> {
    let row = load_row(tx, bill_no).await?;
    Ok(row.filter(|entry| entry.status == "ok"))
}

pub async fn load_row(
    tx: &mut TenantTx<'_>,
    bill_no: &str,
) -> Result<Option<FiscalSubmission>, AppError> {
    let row = sqlx::query_as::<_, FiscalSubmission>(
        r#"
        SELECT id, bill_id, bill_no, authority, invoice_no, qr_payload, status, error_message,
               submitted_at::text AS submitted_at
        FROM fiscal_submissions
        WHERE bill_no = $1
        "#,
    )
    .bind(bill_no)
    .fetch_optional(tx.conn())
    .await?;
    Ok(row)
}

pub async fn list_recent(tx: &mut TenantTx<'_>, limit: i64) -> Result<Vec<FiscalSubmission>, AppError> {
    let rows = sqlx::query_as::<_, FiscalSubmission>(
        r#"
        SELECT id, bill_id, bill_no, authority, invoice_no, qr_payload, status, error_message,
               submitted_at::text AS submitted_at
        FROM fiscal_submissions
        ORDER BY submitted_at DESC, id DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(tx.conn())
    .await?;
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
async fn upsert(
    tx: &mut TenantTx<'_>,
    cfg: &FiscalSettings,
    bill_id: i32,
    bill_no: &str,
    invoice_no: &str,
    qr_payload: &str,
    status: &str,
    error_message: &str,
    raw: &serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO fiscal_submissions
            (bill_id, bill_no, authority, invoice_no, qr_payload, status, error_message, raw_response, submitted_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
        ON CONFLICT (tenant_id, bill_no) DO UPDATE SET
            authority = EXCLUDED.authority,
            invoice_no = EXCLUDED.invoice_no,
            qr_payload = EXCLUDED.qr_payload,
            status = EXCLUDED.status,
            error_message = EXCLUDED.error_message,
            raw_response = EXCLUDED.raw_response,
            submitted_at = now()
        "#,
    )
    .bind(bill_id)
    .bind(bill_no)
    .bind(&cfg.authority)
    .bind(invoice_no)
    .bind(qr_payload)
    .bind(status)
    .bind(error_message)
    .bind(raw)
    .execute(tx.conn())
    .await?;
    Ok(())
}
