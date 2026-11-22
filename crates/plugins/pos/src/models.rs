use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SectionChoice {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub kind: String,
    pub accent: String,
    pub show_on_pos: bool,
    pub pos_order: i32,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Category {
    pub id: i32,
    pub name: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ItemVariant {
    pub id: i32,
    pub item_id: i32,
    pub name: String,
    pub rate: i32,
    pub units: i32,
    pub barcode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PosItem {
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
    pub variants: Vec<ItemVariant>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PosSection {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub kind: String,
    pub accent: String,
    pub items: Vec<PosItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarcodeRule {
    pub prefix: String,
    pub mode: String,
    pub scale: i32,
}

impl BarcodeRule {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                prefix: "21".into(),
                mode: "weight".into(),
                scale: 1000,
            },
            Self {
                prefix: "22".into(),
                mode: "price".into(),
                scale: 1,
            },
        ]
    }

    pub fn checked(self) -> Result<Self, String> {
        if self.prefix.is_empty()
            || self.prefix.len() > 4
            || !self.prefix.chars().all(|ch| ch.is_ascii_digit())
        {
            return Err("barcode prefix must be 1 to 4 digits".into());
        }
        if self.mode != "weight" && self.mode != "price" {
            return Err("barcode mode must be weight or price".into());
        }
        if self.scale < 1 {
            return Err("barcode scale must be at least 1".into());
        }
        if 12 - self.prefix.len() <= 5 {
            return Err("barcode prefix leaves no room for an item code".into());
        }
        Ok(self)
    }
}

fn default_bill_prefix() -> String {
    "BILL".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopConfig {
    pub name: String,
    pub currency: String,
    pub locale: String,
    #[serde(default = "default_bill_prefix")]
    pub bill_prefix: String,
    #[serde(default)]
    pub barcode_rules: Vec<BarcodeRule>,
}

impl ShopConfig {
    pub fn checked(self) -> Result<Self, String> {
        let name = self.name.trim().to_string();
        let currency = self.currency.trim().to_uppercase();
        let locale = self.locale.trim().to_string();
        let bill_prefix = self.bill_prefix.trim().to_uppercase();
        if name.is_empty() {
            return Err("shop.name is required".into());
        }
        if bill_prefix.len() < 1
            || bill_prefix.len() > 8
            || !bill_prefix.chars().all(|ch| ch.is_ascii_alphanumeric())
        {
            return Err("shop.bill_prefix is 1 to 8 letters or digits".into());
        }
        if currency.len() != 3 || !currency.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err("shop.currency must be a 3-letter ISO 4217 code".into());
        }
        if locale.is_empty() {
            return Err("shop.locale is required".into());
        }
        Ok(Self {
            name,
            currency,
            locale,
            bill_prefix,
            barcode_rules: self.barcode_rules,
        })
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PosUser {
    pub id: i32,
    pub name: String,
    pub role: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterSettings {
    pub enabled: bool,
    pub kind: String,
    pub target: String,
    pub port: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PosSnapshot {
    pub shop_name: String,
    pub bill_prefix: String,
    pub currency: String,
    pub locale: String,
    pub barcode_rules: Vec<BarcodeRule>,
    pub printer: PrinterSettings,
    pub sections: Vec<PosSection>,
    pub all_sections: Vec<SectionChoice>,
    pub categories: Vec<Category>,
    pub users: Vec<PosUser>,
    pub last_section_id: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct SavedRecord {
    pub id: i32,
    pub pos: PosSnapshot,
}

#[derive(Debug, Deserialize)]
pub struct SectionForm {
    pub name: String,
    pub short_name: String,
    pub kind: String,
    pub accent: String,
}

#[derive(Debug, Deserialize)]
pub struct CategoryForm {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UserForm {
    pub name: String,
    #[serde(default)]
    pub password: Option<String>,
    pub role: String,
    #[serde(default = "user_active")]
    pub active: bool,
}

fn user_active() -> bool {
    true
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

#[derive(Debug, Deserialize)]
pub struct PrinterForm {
    #[serde(default)]
    pub print_enabled: bool,
    #[serde(default = "network_printer")]
    pub printer_kind: String,
    #[serde(default)]
    pub printer_target: String,
    #[serde(default = "printer_port")]
    pub printer_port: i32,
}

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
    pub name: String,
    pub currency: String,
    pub locale: String,
    #[serde(default = "default_bill_prefix")]
    pub bill_prefix: String,
    #[serde(default)]
    pub barcode_rules: Vec<BarcodeRule>,
    #[serde(default)]
    pub print_enabled: bool,
    #[serde(default = "network_printer")]
    pub printer_kind: String,
    #[serde(default)]
    pub printer_target: String,
    #[serde(default = "printer_port")]
    pub printer_port: i32,
}

fn network_printer() -> String {
    "network".into()
}

fn printer_port() -> i32 {
    9100
}

#[derive(Debug, Serialize)]
pub struct LoginSession {
    pub id: i32,
    pub name: String,
    pub role: String,
    pub active: bool,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderForm {
    pub ids: Vec<i32>,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub name: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SectionLayout {
    pub id: i32,
    pub show_on_pos: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetSectionsRequest {
    pub sections: Vec<SectionLayout>,
}

#[derive(Debug, Deserialize)]
pub struct SetFocusRequest {
    pub section_id: i32,
}

#[derive(Debug, Serialize)]
pub struct PosFocus {
    pub last_section_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct BillLineInput {
    pub item_id: i32,
    pub variant_id: Option<i32>,
    pub rate: i32,
    pub qty: i32,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
    /// Plugin-owned line source. Defaults to `catalog.product`.
    #[serde(default)]
    pub source: Option<String>,
    /// Plugin-owned reference, e.g. a device id or job number.
    #[serde(default)]
    pub reference: Option<String>,
    /// Plugin-owned JSON detail. POS stores it and never interprets it.
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ItemPromptRequest {
    pub prompt: String,
    pub base_rate: Option<i32>,
    pub scale: i32,
}

#[derive(Debug, Deserialize)]
pub struct VariantInput {
    #[serde(default)]
    pub id: Option<i32>,
    pub name: String,
    pub rate: i32,
    pub units: i32,
    #[serde(default)]
    pub barcode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ItemForm {
    pub name: String,
    #[serde(default)]
    pub barcode: Option<String>,
    pub rate: i32,
    #[serde(default)]
    pub cost: i64,
    pub section_id: i32,
    pub category_id: i32,
    pub prompt: String,
    #[serde(default)]
    pub base_rate: Option<i32>,
    pub scale: i32,
    #[serde(default)]
    pub variants: Vec<VariantInput>,
}

#[derive(Debug, Serialize)]
pub struct SavedItem {
    pub item_id: i32,
    pub pos: PosSnapshot,
}

#[derive(Debug, Deserialize)]
pub struct PostBillRequest {
    pub section_id: i32,
    pub lines: Vec<BillLineInput>,
    #[serde(default)]
    pub customer: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PostedBill {
    pub bill_no: String,
    pub total: i32,
    pub pos: PosSnapshot,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct LockedItem {
    pub id: i32,
    pub name: String,
    pub barcode: Option<String>,
    pub stock: i32,
    pub rate: i32,
    pub avg_cost: i64,
    pub variant_count: i32,
    pub prompt: String,
    pub base_rate: Option<i32>,
    pub scale: i32,
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct LockedVariant {
    pub id: i32,
    pub name: String,
    pub rate: i32,
    pub units: i32,
    pub barcode: Option<String>,
}
