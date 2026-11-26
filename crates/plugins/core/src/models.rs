use serde::{Deserialize, Serialize};

/// Core-owned shop identity returned to the UI. Till fields live on POS.
#[derive(Debug, Clone, Serialize)]
pub struct ShopSnapshot {
    pub shop_name: String,
    pub currency: String,
    pub locale: String,
}

#[derive(Debug, Deserialize)]
pub struct ShopProfileForm {
    pub name: String,
    pub currency: String,
    pub locale: String,
}
