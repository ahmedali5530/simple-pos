use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityKind};
use platform_kernel::contract::Registrar;
use platform_kernel::manifest::{NavItem, PluginManifest};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use serde_json::json;

pub struct CashPlugin;

impl Plugin for CashPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "cash".into(),
            name: "Cash".into(),
            version: "0.1.0".into(),
            description: "Cash shifts and expenses.".into(),
            requires: vec!["pos".into()],
            optional: vec!["printing".into()],
            permissions: vec![
                Permission::new(builtins::CASH_READ),
                Permission::new(builtins::CASH_WRITE),
            ],
            capabilities: vec![Capability {
                id: "cash.desk".into(),
                kind: CapabilityKind::Query,
                permission: builtins::CASH_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"object"}),
                description: "Open cash desk state.".into(),
            }],
            events_emitted: vec![],
            events_consumed: vec!["sales.bill_posted".into()],
            settings_schema: None,
            nav: vec![NavItem {
                id: "cash".into(),
                label: "Cash Closing".into(),
                href: "/cash".into(),
                permission: Some(builtins::CASH_READ.into()),
                order: 40,
            }],
            settings_pages: vec![],
            report_pages: vec![],
        }
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _registrar: &mut Registrar) {}
}
