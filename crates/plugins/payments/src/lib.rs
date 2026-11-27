use std::sync::Arc;

use platform_kernel::contract::Registrar;
use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::{
    Dependency, PluginManifest, ReportPage, SettingsPage, SlotDecl,
};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use sales_contract::register_tender_apply;

mod export;
mod routes;
mod sale;
mod settings;
mod tender;

pub struct PaymentsPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> Arc<dyn Plugin> {
    Arc::new(PaymentsPlugin)
}

impl Plugin for PaymentsPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("payments", "Payments", "0.1.0");
        manifest.description =
            "Configurable payment types (card, bank, third-party) as POS tenders.".into();
        manifest.optional = vec![Dependency::new("pos", "^0.1")];
        manifest.permissions = vec![
            Permission::new("payments.read"),
            Permission::new("payments.write"),
        ];
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.settings_pages = vec![SettingsPage {
            id: "payments".into(),
            label: "Payment types".into(),
            href: "/settings/payments".into(),
            permission: Some("payments.write".into()),
            order: 58,
            children: Vec::new(),
        }];
        manifest.report_pages = vec![ReportPage {
            id: "payments_mix".into(),
            label: "Payment mix".into(),
            href: "/reports/payments".into(),
            permission: Some("payments.read".into()),
            order: 25,
        }];
        manifest.slot_contributions = vec![SlotDecl {
            name: "pos.tender".into(),
            contribution: "payments.tender".into(),
            order: 20,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[Migration::new(
            1,
            "payments",
            include_str!("../migrations/001_payments.sql"),
        )];
        MIGRATIONS
    }

    fn register(&self, registrar: &mut Registrar) {
        register_tender_apply(registrar, Arc::new(tender::PaymentsTenderApply));
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(sale::MirrorTenders))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_declares_tender_settings_and_report() {
        let manifest = PaymentsPlugin.manifest();
        manifest.validate().unwrap();
        assert!(manifest.events_consumed.iter().any(|e| e == "sales.bill_posted"));
        assert_eq!(manifest.slot_contributions.len(), 1);
        assert_eq!(manifest.settings_pages.len(), 1);
        assert_eq!(manifest.report_pages.len(), 1);
    }
}
