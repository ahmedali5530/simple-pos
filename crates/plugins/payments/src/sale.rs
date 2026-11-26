//! Mirror posted tenders into plugin-owned rows for reports (no POS table reads).

use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use sales_contract::BillPostedV4;

use crate::settings::{self, find_enabled, type_id_from_method};

pub struct MirrorTenders;

#[async_trait]
impl EventHandler for MirrorTenders {
    fn event_type(&self) -> &str {
        "sales.bill_posted"
    }

    fn subscriber_id(&self) -> &str {
        "payments_mirror_tenders"
    }

    async fn handle(
        &self,
        tx: &mut TenantTx<'_>,
        envelope: &EventEnvelope,
    ) -> Result<(), AppError> {
        if envelope.version < 4 {
            return Ok(());
        }
        let event: BillPostedV4 = serde_json::from_value(envelope.payload.clone())
            .map_err(|error| AppError::BadRequest(format!("invalid sales event: {error}")))?;
        mirror(tx, &event).await
    }
}

async fn mirror(tx: &mut TenantTx<'_>, event: &BillPostedV4) -> Result<(), AppError> {
    let config = settings::load(tx).await?;
    let existing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM payment_entries WHERE bill_id = $1",
    )
    .bind(event.bill_id)
    .fetch_one(tx.conn())
    .await?;
    if existing > 0 {
        return Ok(());
    }

    for tender in &event.tenders {
        let (label, details) = if tender.method == "cash" {
            ("Cash".to_string(), tender.details.clone())
        } else if let Some(type_id) = type_id_from_method(&tender.method) {
            let label = find_enabled(&config, type_id)
                .map(|entry| entry.label.clone())
                .unwrap_or_else(|| type_id.to_string());
            (label, tender.details.clone())
        } else {
            // Leave gift / other plugin methods alone — they own their reports.
            continue;
        };
        sqlx::query(
            r#"
            INSERT INTO payment_entries
                (bill_id, bill_no, method, label, amount, reference, details, posted_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8::timestamptz)
            "#,
        )
        .bind(event.bill_id)
        .bind(&event.bill_no)
        .bind(&tender.method)
        .bind(&label)
        .bind(tender.amount)
        .bind(&tender.reference)
        .bind(&details)
        .bind(&event.posted_at)
        .execute(tx.conn())
        .await?;
    }
    Ok(())
}
