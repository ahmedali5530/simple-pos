use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use printing_contract::{ReceiptBillFact, ReceiptContribute, ReceiptExtras};

use crate::authority::SubmitLine;
use crate::submit;

pub struct FiscalReceiptContribute;

#[async_trait]
impl ReceiptContribute for FiscalReceiptContribute {
    async fn contribute(
        &self,
        tx: &mut TenantTx<'_>,
        bill: &ReceiptBillFact,
    ) -> Result<ReceiptExtras, AppError> {
        let row = match submit::ensure_for_bill(
            tx,
            bill.bill_id,
            &bill.bill_no,
            bill.total,
            &bill.sold_at,
            bill.customer.as_deref(),
            bill.lines
                .iter()
                .map(|line| SubmitLine {
                    label: line.name.clone(),
                    quantity: line.qty.to_string(),
                    amount: i64::from(line.amount),
                })
                .collect(),
        )
        .await
        {
            Ok(row) => row,
            Err(_) => {
                // Do not block the receipt when the authority is down; omit QR.
                return Ok(ReceiptExtras::default());
            }
        };
        if row.status != "ok" || row.qr_payload.trim().is_empty() {
            return Ok(ReceiptExtras::default());
        }
        let invoice = if row.invoice_no.is_empty() {
            row.bill_no.as_str()
        } else {
            row.invoice_no.as_str()
        };
        Ok(ReceiptExtras {
            lines: vec![format!(
                "{} Invoice: {}",
                row.authority.to_ascii_uppercase(),
                invoice
            )],
            qr_payloads: vec![row.qr_payload],
        })
    }
}
