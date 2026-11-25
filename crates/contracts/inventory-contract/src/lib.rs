use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockIssue {
    pub item_id: i32,
    pub qty: i32,
    pub reason: String,
    pub source_type: String,
    pub source_id: Option<i32>,
    #[serde(default)]
    pub bill_id: Option<i32>,
    #[serde(default)]
    pub bill_no: Option<String>,
}

/// Runtime-registered provider wrapper for the stock ledger contract.
pub struct StockLedgerProvider(pub Arc<dyn StockLedger>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductMix {
    pub lines: Vec<ProductMixLine>,
    pub truncated: bool,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductMixLine {
    pub name: String,
    pub qty_in: i64,
    pub qty_out: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockMovement {
    pub id: i32,
    pub item_id: i32,
    pub item_name: String,
    pub section_name: String,
    pub qty: i32,
    pub reason: String,
    pub bill_no: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixLine {
    pub id: i32,
    pub barcode: Option<String>,
    pub name: String,
    pub section_name: String,
    pub qty_in: i64,
    pub qty_out: i64,
    pub opening: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockReport {
    pub movements: Vec<StockMovement>,
    pub stock_units: i64,
    pub stock_value: i64,
    pub movements_truncated: bool,
    pub mix: Vec<MixLine>,
    pub mix_truncated: bool,
    pub range_label: String,
}

#[async_trait]
pub trait InventoryReporting: Send + Sync {
    async fn stock_report(
        &self,
        tx: &mut TenantTx<'_>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<StockReport, AppError>;

    async fn product_mix(
        &self,
        tx: &mut TenantTx<'_>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<ProductMix, AppError>;
}

pub struct InventoryReportsProvider(pub Arc<dyn InventoryReporting>);

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

    /// Create the inventory row for a catalog item, optionally setting average cost.
    /// Stock stays at zero unless a later receive/adjust moves it.
    async fn ensure_balance(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
        avg_cost: Option<i64>,
    ) -> Result<(), AppError>;

    /// Every balance row for this tenant. Used to enrich catalog snapshots and sheets.
    async fn all_balances(&self, tx: &mut TenantTx<'_>) -> Result<Vec<StockBalance>, AppError>;
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

    async fn ensure_balance(
        &self,
        _tx: &mut TenantTx<'_>,
        _item_id: i32,
        _avg_cost: Option<i64>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn all_balances(&self, _tx: &mut TenantTx<'_>) -> Result<Vec<StockBalance>, AppError> {
        Ok(Vec::new())
    }
}

/// One row of the stock-movement workbook, owned by Inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementExportRow {
    pub id: i32,
    pub time_utc: String,
    pub barcode: Option<String>,
    pub item: String,
    pub section: String,
    pub qty: i32,
    pub reason: String,
    pub bill_no: Option<String>,
}

/// A parsed workbook row waiting for Inventory to resolve and apply it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementImportDraft {
    pub row: u32,
    pub barcode: Option<String>,
    pub item_name: String,
    pub qty: i32,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementImportResult {
    pub applied: i64,
}

#[async_trait]
pub trait InventoryWorkbook: Send + Sync {
    async fn movement_rows(
        &self,
        tx: &mut TenantTx<'_>,
    ) -> Result<Vec<MovementExportRow>, AppError>;

    async fn apply_movements(
        &self,
        tx: &mut TenantTx<'_>,
        drafts: &[MovementImportDraft],
    ) -> Result<MovementImportResult, AppError>;

    /// Apply one stock delta from a Catalog workbook. Inventory owns the movement record.
    async fn apply_stock_delta(
        &self,
        tx: &mut TenantTx<'_>,
        item_id: i32,
        delta: i32,
        note: &str,
    ) -> Result<(), AppError>;
}

pub struct InventoryWorkbookProvider(pub Arc<dyn InventoryWorkbook>);
