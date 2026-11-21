use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::DomainEvent;
use serde::{Deserialize, Serialize};

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
