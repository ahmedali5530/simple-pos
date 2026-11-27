//! Matrix SKU stock is applied by the `matrix_sku_apply_sale` trigger on
//! `bill_lines` so oversell rolls back inside the POS transaction. This
//! handler stays as a safety subscriber for the sales event contract.

use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use sales_contract::BillPostedV3;
use tracing::debug;

pub struct MatrixSaleHook;

#[async_trait]
impl EventHandler for MatrixSaleHook {
    fn event_type(&self) -> &str {
        "sales.bill_posted"
    }

    fn subscriber_id(&self) -> &str {
        "matrix_sku_sale"
    }

    async fn handle(
        &self,
        _tx: &mut TenantTx<'_>,
        envelope: &EventEnvelope,
    ) -> Result<(), AppError> {
        if envelope.version < 3 {
            return Ok(());
        }
        let event: BillPostedV3 = serde_json::from_value(envelope.payload.clone())
            .map_err(|error| AppError::BadRequest(format!("invalid sales event: {error}")))?;
        let count = event
            .lines
            .iter()
            .filter(|line| line.source == "matrix.sku")
            .count();
        if count > 0 {
            debug!(bill_id = event.bill_id, count, "matrix SKU lines already applied at insert");
        }
        Ok(())
    }
}
