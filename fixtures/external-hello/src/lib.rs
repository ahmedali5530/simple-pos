use std::sync::Arc;

use axum::Json;
use platform_kernel::contract::Registrar;
use platform_kernel::manifest::PluginManifest;
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use serde_json::{json, Value};

pub struct HelloPlugin;

impl Plugin for HelloPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("hello", "Hello", "0.1.0");
        manifest.description = "External clean-repo sample plugin.".into();
        manifest.permissions = vec![Permission::new("hello.read")];
        manifest.default_enabled = true;
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _: &mut Registrar) {}

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes.get("ping", ping, "hello.read")?;
        Ok(())
    }
}

async fn ping() -> Json<Value> {
    Json(json!({ "ok": true, "plugin": "hello" }))
}

pub fn plugin() -> Arc<dyn Plugin> {
    Arc::new(HelloPlugin)
}
