use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityKind};
use platform_kernel::contract::Registrar;
use platform_kernel::manifest::{NavItem, PluginManifest, ReportPage};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use serde_json::json;

pub struct ReportsPlugin;

impl Plugin for ReportsPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "reports".into(),
            name: "Reports".into(),
            version: "0.1.0".into(),
            description: "Sales, ledger and product-mix reports.".into(),
            requires: vec!["pos".into(), "inventory".into()],
            optional: vec!["printing".into()],
            permissions: vec![Permission::new(builtins::REPORTS_READ)],
            capabilities: vec![Capability {
                id: "reports.summary".into(),
                kind: CapabilityKind::Query,
                permission: builtins::REPORTS_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({
                    "type":"object",
                    "properties":{
                        "from":{"type":"string"},
                        "to":{"type":"string"}
                    }
                }),
                output_schema: json!({"type":"object"}),
                description: "Sales and stock report summary.".into(),
            }],
            events_emitted: vec![],
            events_consumed: vec!["sales.bill_posted".into()],
            settings_schema: None,
            nav: vec![NavItem {
                id: "reports".into(),
                label: "Reports & Summary".into(),
                href: "/reports/sales".into(),
                permission: Some(builtins::REPORTS_READ.into()),
                order: 30,
            }],
            settings_pages: vec![],
            report_pages: vec![
                ReportPage {
                    id: "sales".into(),
                    label: "Sales".into(),
                    href: "/reports/sales".into(),
                    permission: Some(builtins::REPORTS_READ.into()),
                    order: 10,
                },
                ReportPage {
                    id: "ledger".into(),
                    label: "Stock ledger".into(),
                    href: "/reports/ledger".into(),
                    permission: Some(builtins::REPORTS_READ.into()),
                    order: 20,
                },
                ReportPage {
                    id: "mix".into(),
                    label: "Product mix".into(),
                    href: "/reports/mix".into(),
                    permission: Some(builtins::REPORTS_READ.into()),
                    order: 30,
                },
            ],
        }
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _registrar: &mut Registrar) {}
}
