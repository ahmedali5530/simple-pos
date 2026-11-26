//! Test harness for platform plugins: boot a minimal host with a probe plugin.
//!
//! Compatibility surface for authors:
//! - Implement [`platform_kernel::plugin::Plugin`]
//! - Declare deps with [`platform_kernel::manifest::PluginManifest`]
//! - Consume contracts from `crates/contracts/*-contract`
//!
//! Kernel API range is `^1.0` (`PluginManifest::kernel_api`). Manifest schema
//! version 2 is accepted; plugins may still emit v1 until they opt in.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::contract::Registrar;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use platform_kernel::events::PluginEvents;
use platform_kernel::manifest::PluginManifest;
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};
use platform_runtime::http::AuthUser;
use platform_runtime::{BootConfig, Platform, PluginStateStore};
use tower::ServiceExt;
use uuid::Uuid;

/// Minimal plugin used by the testkit and external-plugin smoke proofs.
pub struct ProbePlugin;

impl Plugin for ProbePlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("probe", "Probe", "0.1.0");
        manifest.description = "Testkit probe plugin.".into();
        manifest.permissions = vec![Permission::new("probe.read")];
        manifest.events_consumed = vec!["probe.tick".into()];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, _: &mut Registrar) {}

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes.get("ping", ping, "probe.read")?;
        Ok(())
    }

    fn event_handlers(&self, events: &mut PluginEvents) -> Result<(), RegistrationError> {
        events.on(Arc::new(ProbeTick))?;
        Ok(())
    }
}

async fn ping() -> &'static str {
    "ok"
}

struct ProbeTick;

#[async_trait]
impl EventHandler for ProbeTick {
    fn event_type(&self) -> &str {
        "probe.tick"
    }

    fn subscriber_id(&self) -> &str {
        "tick"
    }

    async fn handle(&self, _: &mut TenantTx<'_>, _: &EventEnvelope) -> Result<(), AppError> {
        Ok(())
    }
}

pub fn probe_plugin() -> Arc<dyn Plugin> {
    Arc::new(ProbePlugin)
}

/// Core + probe — enough to exercise enable/route gating without the full shop.
pub fn baseline_plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(plugin_core::CorePlugin), probe_plugin()]
}

pub fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL")
}

pub async fn boot(plugins: Vec<Arc<dyn Plugin>>) -> Platform {
    let url = database_url();
    Platform::boot(
        BootConfig {
            database_url: url.clone(),
            database_admin_url: Some(url),
            app_password: Some("platform-app-local".into()),
        },
        plugins,
    )
    .await
    .expect("platform boot")
}

pub async fn provision_and_enable_probe(platform: &Platform) -> TenantId {
    let tenant_id = TenantId(Uuid::new_v4());
    sqlx::query("SELECT platform_provision_tenant($1, $2, $3)")
        .bind(tenant_id.as_uuid())
        .bind(format!("kit-{}", &tenant_id.to_string()[..8]))
        .bind("Testkit Shop")
        .execute(platform.migration_pool())
        .await
        .expect("provision");
    PluginStateStore::install_missing(platform.runtime_pool(), tenant_id, &platform.catalog)
        .await
        .expect("install plugins");
    platform
        .lifecycle
        .set_enabled(tenant_id, Actor::system(), "probe", true)
        .await
        .expect("enable probe");
    tenant_id
}

pub async fn probe_ping(
    platform: &Platform,
    tenant_id: TenantId,
    permissions: Vec<String>,
) -> StatusCode {
    let mut request = Request::builder()
        .uri("/api/v1/probe/ping")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthUser {
        tenant_id,
        user_id: 1,
        name: "testkit".into(),
        role: "manager".into(),
        permissions,
    });
    platform
        .plugin_router()
        .oneshot(request)
        .await
        .expect("router")
        .status()
}
