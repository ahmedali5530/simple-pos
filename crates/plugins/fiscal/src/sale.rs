use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use sales_contract::BillPostedV4;
use tracing::warn;

use crate::submit;

pub struct SubmitFiscalInvoice;

#[async_trait]
impl EventHandler for SubmitFiscalInvoice {
    fn event_type(&self) -> &str {
        "sales.bill_posted"
    }

    fn subscriber_id(&self) -> &str {
        "fiscal_submit_invoice"
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
        if let Err(error) = submit::ensure_from_event(tx, &event).await {
            // Outbox must not poison on authority outages; failed rows are kept for retry.
            warn!(
                bill_no = %event.bill_no,
                error = %error,
                "fiscal submission failed after bill post"
            );
        }
        Ok(())
    }
}
