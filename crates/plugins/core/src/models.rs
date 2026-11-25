use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};

fn default_bill_prefix() -> String {
    "BILL".into()
}

#[derive(Debug, Clone)]
pub struct ShopConfig {
    pub name: String,
    pub currency: String,
    pub locale: String,
    pub bill_prefix: String,
    pub barcode_rules: Vec<BarcodeRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarcodeRule {
    pub prefix: String,
    pub mode: String,
    pub scale: i32,
}

impl BarcodeRule {
    pub fn checked(self) -> Result<Self, AppError> {
        if self.prefix.is_empty()
            || self.prefix.len() > 4
            || !self.prefix.chars().all(|ch| ch.is_ascii_digit())
        {
            return Err(AppError::BadRequest(
                "A barcode prefix is 1 to 4 digits.".into(),
            ));
        }
        if self.mode != "weight" && self.mode != "price" {
            return Err(AppError::BadRequest(
                "Each barcode rule is weight or price.".into(),
            ));
        }
        if self.scale < 1 {
            return Err(AppError::BadRequest(
                "Barcode scale must be at least 1.".into(),
            ));
        }
        if 12 - self.prefix.len() <= 5 {
            return Err(AppError::BadRequest(
                "That prefix leaves no room for an item code.".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ShopSnapshot {
    pub shop_name: String,
    pub bill_prefix: String,
    pub currency: String,
    pub locale: String,
    pub barcode_rules: Vec<BarcodeRule>,
}

#[derive(Debug, Deserialize)]
pub struct ShopProfileForm {
    pub name: String,
    pub currency: String,
    pub locale: String,
    #[serde(default = "default_bill_prefix")]
    pub bill_prefix: String,
    #[serde(default)]
    pub barcode_rules: Vec<BarcodeRule>,
}
