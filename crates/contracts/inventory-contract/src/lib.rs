use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockIssue {
    pub item_id: i32,
    pub qty: i32,
    pub reason: String,
    pub source_type: String,
    pub source_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockBalance {
    pub item_id: i32,
    pub stock: i32,
    pub avg_cost: i64,
}

#[async_trait]
pub trait StockLedger: Send + Sync {
    async fn issue(&self, tx: &mut TenantTx<'_>, issues: &[StockIssue]) -> Result<(), AppError>;
    async fn receive(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
        qty: i32,
        total_major: Option<i64>,
        reason: &str,
    ) -> Result<StockBalance, AppError>;
    async fn balance(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
    ) -> Result<Option<StockBalance>, AppError>;
    async fn lock_for_sale(
        &self,
        tx: &mut TenantTx<'_>,
        item_ids: &[i32],
    ) -> Result<Vec<StockBalance>, AppError>;
}

/// No-op ledger when inventory plugin is disabled.
#[derive(Default)]
pub struct NoopStockLedger;

#[async_trait]
impl StockLedger for NoopStockLedger {
    async fn issue(&self, _tx: &mut TenantTx<'_>, _issues: &[StockIssue]) -> Result<(), AppError> {
        Ok(())
    }
    async fn receive(
        &self,
        _tx: &mut TenantTx<'_>,
        item_id: i32,
        _qty: i32,
        _total_major: Option<i64>,
        _reason: &str,
    ) -> Result<StockBalance, AppError> {
        Ok(StockBalance {
            item_id,
            stock: 0,
            avg_cost: 0,
        })
    }
    async fn balance(
        &self,
        _tx: &mut TenantTx<'_>,
        item_id: i32,
    ) -> Result<Option<StockBalance>, AppError> {
        Ok(Some(StockBalance {
            item_id,
            stock: i32::MAX / 4,
            avg_cost: 0,
        }))
    }
    async fn lock_for_sale(
        &self,
        _tx: &mut TenantTx<'_>,
        item_ids: &[i32],
    ) -> Result<Vec<StockBalance>, AppError> {
        Ok(item_ids
            .iter()
            .map(|id| StockBalance {
                item_id: *id,
                stock: i32::MAX / 4,
                avg_cost: 0,
            })
            .collect())
    }
}
