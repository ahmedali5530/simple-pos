use std::sync::Arc;

use platform_kernel::contract::Registrar;
use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::{
    Dependency, PluginConsents, PluginManifest, ReportPage, SettingsPage, SwitchConsent,
};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use printing_contract::register_receipt_contribute;

mod authority;
mod export;
mod receipt;
mod routes;
mod sale;
mod settings;
mod submit;

pub struct FiscalPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> Arc<dyn Plugin> {
    Arc::new(FiscalPlugin)
}

impl Plugin for FiscalPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("fiscal", "Fiscal", "0.1.0");
        manifest.description =
            "Submit sales to FBR/PRA and print the fiscal QR on cash receipts.".into();
        manifest.optional = vec![
            Dependency::new("pos", "^0.1"),
            Dependency::new("printing", "^0.1"),
        ];
        manifest.permissions = vec![
            Permission::new("fiscal.read"),
            Permission::new("fiscal.write"),
        ];
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.consents = PluginConsents {
            enable: Some(SwitchConsent {
                title: "Enable fiscal submission?".into(),
                body: "Sales will be sent to FBR or PRA. Incorrect settings can reject invoices or share shop data with the tax authority.".into(),
                acknowledge: true,
            }),
            disable: Some(SwitchConsent {
                title: "Disable fiscal submission?".into(),
                body: "New sales will no longer be submitted. Past fiscal records stay on this till.".into(),
                acknowledge: false,
            }),
        };
        manifest.settings_pages = vec![SettingsPage {
            id: "fiscal".into(),
            label: "Fiscal (FBR/PRA)".into(),
            href: "/settings/fiscal".into(),
            permission: Some("fiscal.write".into()),
            order: 57,
            children: Vec::new(),
        }];
        manifest.report_pages = vec![ReportPage {
            id: "fiscal_submissions".into(),
            label: "Fiscal submissions".into(),
            href: "/reports/fiscal".into(),
            permission: Some("fiscal.read".into()),
            order: 35,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[
            Migration::new(1, "fiscal", include_str!("../migrations/001_fiscal.sql")),
            Migration::new(
                2,
                "fiscal_seller",
                include_str!("../migrations/002_fiscal_seller.sql"),
            ),
        ];
        MIGRATIONS
    }

    fn register(&self, registrar: &mut Registrar) {
        register_receipt_contribute(registrar, Arc::new(receipt::FiscalReceiptContribute));
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(sale::SubmitFiscalInvoice))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_requires_consent_and_pages() {
        let manifest = FiscalPlugin.manifest();
        manifest.validate().unwrap();
        assert!(manifest.consents.enable.is_some());
        assert!(manifest.consents.disable.is_some());
        assert_eq!(manifest.settings_pages.len(), 1);
        assert_eq!(manifest.report_pages.len(), 1);
        assert!(manifest.events_consumed.iter().any(|e| e == "sales.bill_posted"));
    }
}
