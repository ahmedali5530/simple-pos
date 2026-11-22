use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::DomainEvent;
use serde::{Deserialize, Serialize};

/// A sale line whose meaning belongs to the plugin named by `source`.
/// Point of sale stores and totals it, and does not read `details`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleLine {
    pub source: String,
    pub reference: String,
    pub label: String,
    /// Decimal quantity as text, so the engine and point of sale stay free of a trade's units.
    pub quantity: String,
    pub unit_amount: i64,
    pub amount: i64,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillPosted {
    pub bill_id: i32,
    pub bill_no: String,
    pub section_id: i32,
    pub user_id: i32,
    pub total: i32,
}

impl DomainEvent for BillPosted {
    const TYPE: &'static str = "sales.bill_posted";
    const VERSION: u16 = 1;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillPostedV2 {
    pub bill_id: i32,
    pub bill_no: String,
    pub section_id: i32,
    pub user_id: i32,
    pub total: i32,
    pub lines: Vec<SaleLine>,
}

impl DomainEvent for BillPostedV2 {
    const TYPE: &'static str = "sales.bill_posted";
    const VERSION: u16 = 2;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesTotal {
    pub from: Option<String>,
    pub total: i64,
    pub bill_count: i64,
}

#[async_trait]
pub trait SalesQuery: Send + Sync {
    async fn total_since(
        &self,
        tx: &mut TenantTx<'_>,
        since_iso: &str,
    ) -> Result<SalesTotal, AppError>;
}
