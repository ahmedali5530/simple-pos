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

/// How a bill was settled. POS validates amounts; method owners apply side effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleTender {
    pub method: String,
    pub amount: i32,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillPostedV4 {
    pub bill_id: i32,
    pub bill_no: String,
    pub section_id: i32,
    pub user_id: i32,
    pub total: i32,
    pub customer: Option<String>,
    pub posted_at: String,
    pub lines: Vec<SaleLine>,
    #[serde(default)]
    pub tenders: Vec<SaleTender>,
}

impl DomainEvent for BillPostedV4 {
    const TYPE: &'static str = "sales.bill_posted";
    const VERSION: u16 = 4;
}

/// Context passed to tender handlers while the bill post transaction is open.
pub struct TenderApplyContext<'a> {
    pub bill_id: i32,
    pub bill_no: &'a str,
    pub total: i32,
}

/// Apply a non-cash tender inside the bill post transaction.
/// Return `true` when this handler owns the method; `false` to let another try.
#[async_trait]
pub trait TenderApply: Send + Sync {
    async fn apply(
        &self,
        tx: &mut TenantTx<'_>,
        ctx: &TenderApplyContext<'_>,
        tender: &SaleTender,
    ) -> Result<bool, AppError>;
}

/// Composable tender handlers. Plugins push; POS fans out until one returns true.
pub struct TenderApplyProvider {
    handlers: std::sync::Mutex<Vec<std::sync::Arc<dyn TenderApply>>>,
}

impl TenderApplyProvider {
    pub fn new() -> Self {
        Self {
            handlers: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn push(&self, handler: std::sync::Arc<dyn TenderApply>) {
        self.handlers
            .lock()
            .expect("tender apply registry")
            .push(handler);
    }

    pub async fn apply(
        &self,
        tx: &mut TenantTx<'_>,
        ctx: &TenderApplyContext<'_>,
        tender: &SaleTender,
    ) -> Result<bool, AppError> {
        let handlers = self
            .handlers
            .lock()
            .expect("tender apply registry")
            .clone();
        for handler in handlers {
            if handler.apply(tx, ctx, tender).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Default for TenderApplyProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Append a tender handler. First caller creates a shared (unowned) registry.
pub fn register_tender_apply(
    registrar: &mut platform_kernel::contract::Registrar,
    handler: std::sync::Arc<dyn TenderApply>,
) {
    if let Some(existing) = registrar.get::<TenderApplyProvider>() {
        existing.push(handler);
        return;
    }
    let provider = std::sync::Arc::new(TenderApplyProvider::new());
    provider.push(handler);
    registrar.provide_shared(provider);
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
    #[serde(default)]
    pub tenders: Vec<ReceiptTender>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptLine {
    pub name: String,
    pub qty: i32,
    pub rate: i32,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptTender {
    pub method: String,
    pub amount: i32,
    #[serde(default)]
    pub reference: String,
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
