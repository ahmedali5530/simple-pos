use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use sales_contract::{SaleTender, TenderApply, TenderApplyContext};

use crate::settings::{self, find_enabled, type_id_from_method};

pub struct PaymentsTenderApply;

#[async_trait]
impl TenderApply for PaymentsTenderApply {
    async fn apply(
        &self,
        tx: &mut TenantTx<'_>,
        _ctx: &TenderApplyContext<'_>,
        tender: &SaleTender,
    ) -> Result<bool, AppError> {
        let Some(type_id) = type_id_from_method(&tender.method) else {
            return Ok(false);
        };
        let config = settings::load(tx).await?;
        let Some(payment_type) = find_enabled(&config, type_id) else {
            return Err(AppError::BadRequest(format!(
                "Payment type `{type_id}` is not enabled."
            )));
        };
        if payment_type.requires_reference && tender.reference.trim().is_empty() {
            return Err(AppError::BadRequest(format!(
                "{} needs a reference (auth code, slip no, etc).",
                payment_type.label
            )));
        }
        // Recording-only for now: POS already persisted bill_tenders; side effects /
        // gateway capture can plug in here later without changing the POS host.
        Ok(true)
    }
}
