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

mod routes;
mod sale;
mod tender;

pub struct GiftPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> Arc<dyn Plugin> {
    Arc::new(GiftPlugin)
}

impl Plugin for GiftPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("gift", "Gift Cards", "0.1.0");
        manifest.description =
            "Create gift cards in settings and redeem them as POS tenders.".into();
        manifest.optional = vec![
            Dependency::new("pos", "^0.1"),
            Dependency::new("printing", "^0.1"),
        ];
        manifest.permissions = vec![
            Permission::new("gift.read"),
            Permission::new("gift.write"),
        ];
        // Kept for bills that still carry legacy gift.issue lines.
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.settings_pages = vec![SettingsPage {
            id: "gift".into(),
            label: "Gift cards".into(),
            href: "/settings/gift".into(),
            permission: Some("gift.write".into()),
            order: 62,
            children: Vec::new(),
        }];
        manifest.report_pages = vec![ReportPage {
            id: "gift_liability".into(),
            label: "Gift liability".into(),
            href: "/reports/gift".into(),
            permission: Some("gift.read".into()),
            order: 40,
        }];
        manifest.slot_contributions = vec![SlotDecl {
            name: "pos.tender".into(),
            contribution: "gift.redeem".into(),
            order: 10,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] =
            &[Migration::new(1, "gift", include_str!("../migrations/001_gift.sql"))];
        MIGRATIONS
    }

    fn register(&self, registrar: &mut Registrar) {
        register_tender_apply(registrar, Arc::new(tender::GiftTenderApply));
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(sale::IssueCards))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_declares_tender_slot() {
        let manifest = GiftPlugin.manifest();
        manifest.validate().unwrap();
        assert!(manifest.events_consumed.iter().any(|e| e == "sales.bill_posted"));
        assert_eq!(manifest.slot_contributions.len(), 1);
        assert_eq!(manifest.slot_contributions[0].contribution, "gift.redeem");
        assert_eq!(manifest.settings_pages.len(), 1);
        assert_eq!(manifest.report_pages.len(), 1);
    }
}
