mod admin;
mod barcode;
mod billing;
mod cash;
mod error;
mod ledger;
mod models;
mod print;
mod sheets;

pub use billing::{
    clear_item_image, connect, current_shop, item_image_ext, load_pos, post_bill, save_item, set_item_image,
    set_item_prompt,
    set_pos_focus, set_pos_sections,
};
pub use sqlx::PgPool;
pub use error::{AppError, ErrorBody};
pub use sheets::{
    export_categories, export_items, export_movements, import_categories, import_items, import_movements,
    CategoryImport, ItemImport, MovementImport,
};
pub use ledger::{
    adjust_stock, load_reports, product_mix, recent_movements, stock_summary, MixLine, ProductMix, Reports,
    StockBalance, StockMove, StockMovement, StockPage,
};
pub use cash::{cash_desk, load_shift, open_shift, save_shift, CashDesk, CashExpense, CashShift, OpenShift, ShiftForm};
pub use admin::{
    login, logout, needs_first_user, open_session, remove_category, remove_section, remove_user,
    reorder_categories,
    reorder_sections, save_category, save_section, save_settings, save_user,
};
pub use print::{
    print_cash_shift, print_product_mix, print_saved_bill, print_test, PrintOutcome, PrintRequest,
    TestPrintRequest,
};
pub use models::{
    BarcodeRule, BillLineInput, CategoryForm, ItemForm, ItemPromptRequest, LoginForm, OrderForm,
    LoginSession, PosFocus, PosSnapshot, PosUser, PostBillRequest, PostedBill, PrinterSettings,
    SavedItem, SavedRecord,
    SectionForm, SetFocusRequest, SetSectionsRequest, SettingsForm, ShopConfig, UserForm,
};
