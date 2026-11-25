use async_trait::async_trait;
use platform_kernel::{context::TenantTx, error::AppError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashExpense {
    pub description: String,
    pub category: String,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashClosing {
    pub id: i32,
    pub opened_by: String,
    pub closed_by: Option<String>,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub opening_cash: i32,
    pub petty_cash: i32,
    pub sales: i64,
    pub expenses: Option<i32>,
    pub cash_taken: Option<i32>,
    pub manual_count: Option<i32>,
    pub system_count: Option<i64>,
    pub difference: Option<i64>,
    pub cash_left: Option<i32>,
    pub lines: Vec<CashExpense>,
}

#[async_trait]
pub trait CashClosingQuery: Send + Sync {
    async fn closing(&self, tx: &mut TenantTx<'_>, id: i32) -> Result<CashClosing, AppError>;
}

pub struct CashClosingProvider(pub Arc<dyn CashClosingQuery>);
