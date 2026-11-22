//! The only place the compiled plugin set is listed.
use std::sync::Arc;

use platform_kernel::plugin::Plugin;

pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(plugin_catalog::CatalogPlugin),
        Arc::new(plugin_inventory::InventoryPlugin),
        Arc::new(plugin_pos::PosPlugin),
        Arc::new(plugin_cash::CashPlugin),
        Arc::new(plugin_printing::PrintingPlugin),
        Arc::new(plugin_reports::ReportsPlugin),
        Arc::new(plugin_customers::CustomersPlugin),
        Arc::new(plugin_tailor::TailorPlugin),
        Arc::new(plugin_installments::InstallmentsPlugin),
        Arc::new(plugin_mobile::MobilePlugin),
    ]
}
