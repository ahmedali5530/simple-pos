use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRef {
    pub id: i32,
    pub name: String,
    pub section_id: i32,
    pub category_id: i32,
    pub barcode: Option<String>,
    pub rate: i32,
    pub active: bool,
    pub item_type: String,
    pub unit_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantRef {
    pub id: i32,
    pub item_id: i32,
    pub name: String,
    pub rate: i32,
    pub units: i32,
    pub barcode: Option<String>,
    pub unit_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionRef {
    pub id: i32,
    pub name: String,
    pub short_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRef {
    pub id: i32,
    pub name: String,
}

#[async_trait]
pub trait CatalogQuery: Send + Sync {
    async fn get_item(&self, tx: &mut TenantTx<'_>, id: i32) -> Result<Option<ItemRef>, AppError>;
    async fn get_variant(
        &self,
        tx: &mut TenantTx<'_>,
        id: i32,
    ) -> Result<Option<VariantRef>, AppError>;
    async fn items_by_ids(
        &self,
        tx: &mut TenantTx<'_>,
        ids: &[i32],
    ) -> Result<Vec<ItemRef>, AppError>;
    async fn find_by_barcode(
        &self,
        tx: &mut TenantTx<'_>,
        barcode: &str,
    ) -> Result<Option<ItemRef>, AppError>;
    async fn list_sections(&self, tx: &mut TenantTx<'_>) -> Result<Vec<SectionRef>, AppError>;
    async fn list_categories(&self, tx: &mut TenantTx<'_>) -> Result<Vec<CategoryRef>, AppError>;
    async fn is_stock_tracked(&self, tx: &mut TenantTx<'_>, item_id: i32)
        -> Result<bool, AppError>;
    async fn snapshot(&self, tx: &mut TenantTx<'_>) -> Result<CatalogSnapshot, AppError>;

    async fn set_pos_layout(
        &self,
        tx: &mut TenantTx<'_>,
        layout: &[(i32, bool, i32)],
    ) -> Result<(), AppError>;
}

pub mod events {
    pub const ITEM_SAVED: &str = "catalog.item_saved";
    pub const ITEM_REMOVED: &str = "catalog.item_removed";
}

/// Handle other plugins resolve from the registrar.
#[derive(Clone)]
pub struct CatalogApi(pub Arc<dyn CatalogQuery>);

/// One section in the catalog snapshot used by the till.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSection {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub kind: String,
    pub accent: String,
    pub show_on_pos: bool,
    pub pos_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogCategory {
    pub id: i32,
    pub name: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogVariant {
    pub id: i32,
    pub item_id: i32,
    pub name: String,
    pub rate: i32,
    pub units: i32,
    pub barcode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: i32,
    pub section_id: i32,
    pub category_id: i32,
    pub category_name: String,
    pub barcode: Option<String>,
    pub name: String,
    pub rate: i32,
    pub cost: i64,
    pub stock: i32,
    pub portion: String,
    pub group_name: Option<String>,
    pub signature: bool,
    pub prompt: String,
    pub base_rate: Option<i32>,
    pub scale: i32,
    pub image_url: Option<String>,
    pub variants: Vec<CatalogVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSnapshot {
    pub sections: Vec<CatalogSection>,
    pub categories: Vec<CatalogCategory>,
    pub items: Vec<CatalogItem>,
}
