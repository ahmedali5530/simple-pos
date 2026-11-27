//! Contracts for receipt rendering. Printing owns the ESC/POS bytes; other plugins
//! contribute footer text and QR payloads without embedding fiscal (or other) logic in printing.

use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};

/// Bill facts printing already loaded — contributors must not read POS tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptBillFact {
    pub bill_id: i32,
    pub bill_no: String,
    pub total: i32,
    pub sold_at: String,
    pub customer: Option<String>,
    pub lines: Vec<ReceiptBillLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptBillLine {
    pub name: String,
    pub qty: i32,
    pub amount: i32,
}

/// Extra content a plugin wants on a cash receipt for one bill.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceiptExtras {
    /// Plain footer lines printed above the thank-you block.
    pub lines: Vec<String>,
    /// QR payloads (URLs or opaque fiscal strings). Printing renders each as an ESC/POS QR.
    pub qr_payloads: Vec<String>,
}

impl ReceiptExtras {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.qr_payloads.is_empty()
    }

    pub fn merge(&mut self, other: ReceiptExtras) {
        self.lines.extend(other.lines);
        self.qr_payloads.extend(
            other
                .qr_payloads
                .into_iter()
                .filter(|payload| !payload.trim().is_empty()),
        );
    }
}

/// Contribute receipt footer / QR for a bill. Called at print time so a fiscal
/// plugin can submit idempotently and return the QR before bytes are built.
#[async_trait]
pub trait ReceiptContribute: Send + Sync {
    async fn contribute(
        &self,
        tx: &mut TenantTx<'_>,
        bill: &ReceiptBillFact,
    ) -> Result<ReceiptExtras, AppError>;
}

/// Composable receipt contributors. Plugins push; printing merges every result.
pub struct ReceiptContributeProvider {
    handlers: std::sync::Mutex<Vec<std::sync::Arc<dyn ReceiptContribute>>>,
}

impl ReceiptContributeProvider {
    pub fn new() -> Self {
        Self {
            handlers: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn push(&self, handler: std::sync::Arc<dyn ReceiptContribute>) {
        self.handlers
            .lock()
            .expect("receipt contribute registry")
            .push(handler);
    }

    pub async fn gather(
        &self,
        tx: &mut TenantTx<'_>,
        bill: &ReceiptBillFact,
    ) -> Result<ReceiptExtras, AppError> {
        let handlers = self
            .handlers
            .lock()
            .expect("receipt contribute registry")
            .clone();
        let mut extras = ReceiptExtras::default();
        for handler in handlers {
            extras.merge(handler.contribute(tx, bill).await?);
        }
        Ok(extras)
    }
}

impl Default for ReceiptContributeProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Append a receipt contributor. First caller creates a shared (unowned) registry.
pub fn register_receipt_contribute(
    registrar: &mut platform_kernel::contract::Registrar,
    handler: std::sync::Arc<dyn ReceiptContribute>,
) {
    if let Some(existing) = registrar.get::<ReceiptContributeProvider>() {
        existing.push(handler);
        return;
    }
    let provider = std::sync::Arc::new(ReceiptContributeProvider::new());
    provider.push(handler);
    registrar.provide_shared(provider);
}
