use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityKind};
use platform_kernel::contract::Registrar;
use platform_kernel::manifest::{PluginManifest, SettingsPage};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use serde_json::json;

pub struct PrintingPlugin;

impl Plugin for PrintingPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "printing".into(),
            name: "Printing".into(),
            version: "0.1.0".into(),
            description: "Receipt printer settings and ESC/POS dispatch.".into(),
            requires: vec![],
            optional: vec![],
            permissions: vec![Permission::new(builtins::PRINTING_USE)],
            capabilities: vec![Capability {
                id: "printing.status".into(),
                kind: CapabilityKind::Query,
                permission: builtins::PRINTING_USE.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object"}),
                output_schema: json!({"type":"object"}),
                description: "Printer enabled status.".into(),
            }],
            events_emitted: vec![],
            events_consumed: vec![],
            settings_schema: None,
            nav: vec![],
            settings_pages: vec![SettingsPage {
                id: "system".into(),
                label: "System".into(),
                href: "/settings/system".into(),
                permission: Some(builtins::SETTINGS_WRITE.into()),
                order: 50,
            }],
            report_pages: vec![],
        }
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _registrar: &mut Registrar) {}
}
