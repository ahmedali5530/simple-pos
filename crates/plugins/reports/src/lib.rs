use platform_kernel::contract::Registrar;
use platform_kernel::manifest::{Dependency, NavItem, PluginManifest, ReportPage};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};

mod routes;

pub struct ReportsPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> std::sync::Arc<dyn Plugin> {
    std::sync::Arc::new(ReportsPlugin)
}

impl Plugin for ReportsPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("reports", "Reports", "0.1.0");
        manifest.default_enabled = true;
        manifest.description = "Sales, ledger and product-mix reports.".into();
        manifest.optional = vec![
            Dependency::new("pos", "^0.1"),
            Dependency::new("inventory", "^0.1"),
            Dependency::new("printing", "^0.1"),
        ];
        manifest.permissions = vec![Permission::new("reports.read")];
        manifest.nav = vec![NavItem {
            id: "reports".into(),
            label: "Reports & Summary".into(),
            href: "/reports/sales".into(),
            permission: Some("reports.read".into()),
            order: 30,
            children: Vec::new(),
        }];
        manifest.report_pages = vec![
            ReportPage {
                id: "sales".into(),
                label: "Sales".into(),
                href: "/reports/sales".into(),
                permission: Some("reports.read".into()),
                order: 10,
            },
            ReportPage {
                id: "ledger".into(),
                label: "Stock ledger".into(),
                href: "/reports/ledger".into(),
                permission: Some("reports.read".into()),
                order: 20,
            },
            ReportPage {
                id: "mix".into(),
                label: "Product mix".into(),
                href: "/reports/mix".into(),
                permission: Some("reports.read".into()),
                order: 30,
            },
        ];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn register(&self, _registrar: &mut Registrar) {}
}
