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
    #[serde(default)]
    pub item_id: Option<i32>,
    /// Inventory units consumed, including the item's catalog unit multiplier.
    #[serde(default)]
    pub stock_quantity: i32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillPostedV3 {
    pub bill_id: i32,
    pub bill_no: String,
    pub section_id: i32,
    pub user_id: i32,
    pub total: i32,
    pub customer: Option<String>,
    pub posted_at: String,
    pub lines: Vec<SaleLine>,
}

impl DomainEvent for BillPostedV3 {
    const TYPE: &'static str = "sales.bill_posted";
    const VERSION: u16 = 3;
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

    async fn bill_count_for_section(
        &self,
        tx: &mut TenantTx<'_>,
        section_id: i32,
    ) -> Result<i64, AppError>;
}

pub struct SalesQueryProvider(pub std::sync::Arc<dyn SalesQuery>);

/// POS-owned receipt data. Consumers render it without reading POS tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptData {
    pub bill_no: String,
    pub total: i32,
    pub sold_at: String,
    pub cashier: String,
    pub customer: Option<String>,
    pub lines: Vec<ReceiptLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptLine {
    pub name: String,
    pub qty: i32,
    pub rate: i32,
    pub amount: i32,
}

#[async_trait]
pub trait SalesReceiptQuery: Send + Sync {
    async fn receipt(&self, tx: &mut TenantTx<'_>, bill_no: &str) -> Result<ReceiptData, AppError>;
}

pub struct SalesReceiptProvider(pub std::sync::Arc<dyn SalesReceiptQuery>);

/// One section row in the sales report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesSectionTotal {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub accent: String,
    pub bills: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesBillLine {
    pub name: String,
    pub qty: i32,
    pub rate: i32,
    pub amount: i32,
    pub unit_cost: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesBill {
    pub id: i32,
    pub bill_no: String,
    pub section_name: String,
    pub short_name: String,
    pub accent: String,
    pub total: i32,
    pub cost: i64,
    pub created_at: String,
    pub cashier_name: Option<String>,
    pub customer: Option<String>,
    pub lines: Vec<SalesBillLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesReport {
    pub sections: Vec<SalesSectionTotal>,
    pub bills: Vec<SalesBill>,
    pub bill_count: i64,
    pub total: i64,
    pub profit: i64,
    pub bills_truncated: bool,
    pub range_label: String,
}

#[async_trait]
pub trait SalesReporting: Send + Sync {
    async fn report(
        &self,
        tx: &mut TenantTx<'_>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<SalesReport, AppError>;
}

pub struct SalesReportingProvider(pub std::sync::Arc<dyn SalesReporting>);
