pub mod print;
mod routes;

use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityKind};
use platform_kernel::contract::Registrar;
use platform_kernel::manifest::{PluginManifest, SettingsPage};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use serde_json::json;

pub struct PrintingPlugin;

impl Plugin for PrintingPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("printing", "Printing", "0.1.0");
        manifest.description = "Receipt printer settings and ESC/POS dispatch.".into();
        manifest.permissions = vec![
            Permission::new("printing.use"),
            Permission::new("settings.write"),
        ];
        manifest.capabilities = vec![Capability {
            id: "printing.status".into(),
            kind: CapabilityKind::Query,
            permission: "printing.use".into(),
            effect: CapabilityEffect::Read,
            requires_confirmation: false,
            input_schema: json!({"type":"object"}),
            output_schema: json!({"type":"object"}),
            description: "Printer enabled status.".into(),
        }];
        manifest.settings_pages = vec![SettingsPage {
            id: "printing".into(),
            label: "Printing".into(),
            href: "/settings/printing".into(),
            permission: Some("settings.write".into()),
            order: 55,
        }];
        manifest
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _registrar: &mut Registrar) {}
}
