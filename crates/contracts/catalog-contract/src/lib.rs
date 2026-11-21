use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRef {
    pub id: i32,
    pub name: String,
    pub section_id: i32,
    pub category_id: i32,
    pub barcode: Option<String>,
    pub rate: i32,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionRef {
    pub id: i32,
    pub name: String,
    pub short_name: String,
}

#[async_trait]
pub trait CatalogQuery: Send + Sync {
    async fn get_item(&self, tx: &mut TenantTx<'_>, id: i32) -> Result<Option<ItemRef>, AppError>;
    async fn list_sections(&self, tx: &mut TenantTx<'_>) -> Result<Vec<SectionRef>, AppError>;
}
