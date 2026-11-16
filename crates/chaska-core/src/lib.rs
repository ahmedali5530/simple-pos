mod barcode;
mod billing;
mod error;
mod models;

pub use billing::{connect, load_pos, post_bill, set_item_prompt, set_pos_focus, set_pos_sections};
pub use sqlx::PgPool;
pub use error::{AppError, ErrorBody};
pub use models::{
    BarcodeRule, BillLineInput, ItemPromptRequest, PosFocus, PosSnapshot, PostBillRequest,
    PostedBill, SetFocusRequest, SetSectionsRequest, ShopConfig,
};
