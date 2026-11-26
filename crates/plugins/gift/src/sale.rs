//! Activate gift cards from stock-neutral POS issue lines.

use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use sales_contract::BillPostedV4;
use tracing::warn;

use crate::tender::normalize_code;

pub struct IssueCards;

#[async_trait]
impl EventHandler for IssueCards {
    fn event_type(&self) -> &str {
        "sales.bill_posted"
    }

    fn subscriber_id(&self) -> &str {
        "gift_issue_cards"
    }

    async fn handle(
        &self,
        tx: &mut TenantTx<'_>,
        envelope: &EventEnvelope,
    ) -> Result<(), AppError> {
        if envelope.version < 3 {
            return Ok(());
        }
        let event: BillPostedV4 = serde_json::from_value(envelope.payload.clone())
            .map_err(|error| AppError::BadRequest(format!("invalid sales event: {error}")))?;
        apply_bill(tx, &event).await
    }
}

async fn apply_bill(tx: &mut TenantTx<'_>, event: &BillPostedV4) -> Result<(), AppError> {
    let lines: Vec<_> = event
        .lines
        .iter()
        .filter(|line| line.source == "gift.issue")
        .collect();
    if lines.is_empty() {
        return Ok(());
    }

    for (line_index, line) in lines.into_iter().enumerate() {
        let code = match normalize_code(&line.reference) {
            Ok(code) => code,
            Err(_) => {
                warn!(bill_id = event.bill_id, line_index, "gift issue line has no card code");
                continue;
            }
        };
        let amount = line.amount.max(0) as i32;
        if amount < 1 {
            warn!(bill_id = event.bill_id, line_index, "gift issue line has no value");
            continue;
        }
        let existing: Option<(i32, String)> =
            sqlx::query_as("SELECT id, status FROM gift_cards WHERE code = $1")
                .bind(&code)
                .fetch_optional(tx.conn())
                .await?;
        if let Some((id, status)) = existing {
            if status == "void" {
                return Err(AppError::BadRequest(format!(
                    "Gift card {code} is void and cannot be reissued."
                )));
            }
            // Already issued (idempotent replay): skip.
            let _ = id;
            continue;
        }
        let card_id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO gift_cards
                (code, balance, status, issued_bill_id, issued_bill_no, issued_at)
            VALUES ($1, $2, 'active', $3, $4, now())
            RETURNING id
            "#,
        )
        .bind(&code)
        .bind(amount)
        .bind(event.bill_id)
        .bind(&event.bill_no)
        .fetch_one(tx.conn())
        .await?;
        sqlx::query(
            r#"
            INSERT INTO gift_ledger
                (card_id, kind, amount, balance_after, bill_id, bill_no, note)
            VALUES ($1, 'issue', $2, $2, $3, $4, '')
            "#,
        )
        .bind(card_id)
        .bind(amount)
        .bind(event.bill_id)
        .bind(&event.bill_no)
        .execute(tx.conn())
        .await?;
    }
    Ok(())
}
