use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityKind};
use platform_kernel::contract::Registrar;
use platform_kernel::manifest::{Dependency, NavItem, PluginManifest};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use serde_json::json;

pub struct CashPlugin;

impl Plugin for CashPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("cash", "Cash", "0.1.0");
        manifest.description = "Cash shifts and expenses.".into();
        manifest.requires = vec![Dependency::new("pos", "^0.1")];
        manifest.optional = vec![Dependency::new("printing", "^0.1")];
        manifest.permissions = vec![
            Permission::new(builtins::CASH_READ),
            Permission::new(builtins::CASH_WRITE),
        ];
        manifest.capabilities = vec![Capability {
            id: "cash.desk".into(),
            kind: CapabilityKind::Query,
            permission: builtins::CASH_READ.into(),
            effect: CapabilityEffect::Read,
            requires_confirmation: false,
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"object"}),
            description: "Open cash desk state.".into(),
        }];
        manifest.events_consumed = vec!["sales.bill_posted".into()];
        manifest.nav = vec![NavItem {
            id: "cash".into(),
            label: "Cash Closing".into(),
            href: "/cash".into(),
            permission: Some(builtins::CASH_READ.into()),
            order: 40,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _registrar: &mut Registrar) {}
}
