//! Plugin lifecycle: persisted enablement, audit, dependency rejection, and route gating.

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

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL")
}

struct Probe;

impl Plugin for Probe {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("probe", "Probe", "0.1.0");
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
        events.on(Arc::new(ProbeHandler))?;
        Ok(())
    }
}

async fn ping() -> &'static str {
    "ok"
}

struct ProbeHandler;

#[async_trait]
impl EventHandler for ProbeHandler {
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

fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(plugin_core::CorePlugin),
        Arc::new(plugin_catalog::CatalogPlugin),
        Arc::new(plugin_inventory::InventoryPlugin),
        Arc::new(plugin_pos::PosPlugin),
        Arc::new(plugin_cash::CashPlugin),
        Arc::new(plugin_printing::PrintingPlugin),
        Arc::new(plugin_reports::ReportsPlugin),
        Arc::new(Probe),
    ]
}

async fn boot() -> Platform {
    Platform::boot(
        BootConfig {
            database_url: database_url(),
            database_admin_url: Some(database_url()),
            app_password: Some("platform-app-local".into()),
        },
        plugins(),
    )
    .await
    .expect("platform boot")
}

async fn fresh_tenant(platform: &Platform) -> TenantId {
    let tenant_id = TenantId(Uuid::new_v4());
    sqlx::query("SELECT platform_provision_tenant($1, $2, $3)")
        .bind(tenant_id.as_uuid())
        .bind(format!("life-{}", &tenant_id.to_string()[..8]))
        .bind("Lifecycle Shop")
        .execute(platform.migration_pool())
        .await
        .expect("provision");
    PluginStateStore::install_missing(platform.runtime_pool(), tenant_id, &platform.catalog)
        .await
        .expect("install plugins");
    // `probe` is the test-only plugin and is intentionally not a default-enabled plugin.
    platform
        .lifecycle
        .set_enabled(tenant_id, Actor::system(), "probe", true)
        .await
        .expect("enable probe");
    tenant_id
}

async fn probe(platform: &Platform, tenant_id: TenantId, permissions: Vec<String>) -> StatusCode {
    let mut request = Request::builder()
        .uri("/api/v1/probe/ping")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthUser {
        tenant_id,
        user_id: 1,
        name: "lifecycle".into(),
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

#[tokio::test]
async fn disable_keeps_the_row_and_writes_an_audit_entry() {
    let platform = boot().await;
    let tenant_id = fresh_tenant(&platform).await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements WHERE tenant_id = $1")
        .bind(tenant_id.as_uuid())
        .fetch_one(platform.migration_pool())
        .await
        .unwrap();

    // Disable dependents before their dependency: reports and cash require pos,
    // and pos now requires inventory.
    for plugin in ["reports", "cash", "pos", "inventory"] {
        platform
            .lifecycle
            .set_enabled(tenant_id, Actor::system(), plugin, false)
            .await
            .unwrap_or_else(|error| panic!("disable {plugin}: {error}"));
    }

    let (enabled,): (bool,) = sqlx::query_as(
        "SELECT enabled FROM tenant_plugins WHERE tenant_id = $1 AND plugin_id = 'inventory'",
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert!(!enabled, "disabling must keep the plugin row");

    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements WHERE tenant_id = $1")
        .bind(tenant_id.as_uuid())
        .fetch_one(platform.migration_pool())
        .await
        .unwrap();
    assert_eq!(before, after, "disabling must not delete plugin data");

    let audits: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM platform_audit_log
        WHERE tenant_id = $1 AND action = 'plugin.disabled'
          AND detail->>'plugin_id' = 'inventory'
        "#,
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert_eq!(audits, 1);
}

#[tokio::test]
async fn cannot_disable_a_plugin_that_enabled_plugins_require() {
    let platform = boot().await;
    let tenant_id = fresh_tenant(&platform).await;
    let err = platform
        .lifecycle
        .set_enabled(tenant_id, Actor::system(), "catalog", false)
        .await
        .expect_err("catalog is required");
    assert!(matches!(err, AppError::Conflict(_)), "{err}");
    assert!(err.to_string().contains("pos") || err.to_string().contains("inventory"));

    let audits: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM platform_audit_log
        WHERE tenant_id = $1 AND action = 'plugin.disabled'
          AND detail->>'plugin_id' = 'catalog'
        "#,
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert_eq!(audits, 0);

    let (enabled,): (bool,) = sqlx::query_as(
        "SELECT enabled FROM tenant_plugins WHERE tenant_id = $1 AND plugin_id = 'catalog'",
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert!(enabled);
}

#[tokio::test]
async fn cannot_enable_a_plugin_whose_dependency_is_disabled() {
    let platform = boot().await;
    let tenant_id = fresh_tenant(&platform).await;
    for plugin in ["reports", "cash", "pos", "inventory", "catalog"] {
        platform
            .lifecycle
            .set_enabled(tenant_id, Actor::system(), plugin, false)
            .await
            .unwrap_or_else(|err| panic!("disable {plugin}: {err}"));
    }
    let err = platform
        .lifecycle
        .set_enabled(tenant_id, Actor::system(), "pos", true)
        .await
        .expect_err("catalog is off");
    assert!(matches!(err, AppError::Conflict(_)), "{err}");
    assert!(err.to_string().contains("catalog"));
}

#[tokio::test]
async fn route_follows_enablement_and_permission() {
    let platform = boot().await;
    let tenant_id = fresh_tenant(&platform).await;
    assert_eq!(probe(&platform, tenant_id, vec!["*".into()]).await, StatusCode::OK);
    assert_eq!(
        probe(&platform, tenant_id, vec!["catalog.read".into()]).await,
        StatusCode::FORBIDDEN
    );
    platform
        .lifecycle
        .set_enabled(tenant_id, Actor::system(), "probe", false)
        .await
        .unwrap();
    assert_eq!(
        probe(&platform, tenant_id, vec!["*".into()]).await,
        StatusCode::NOT_FOUND
    );
    platform
        .lifecycle
        .set_enabled(tenant_id, Actor::system(), "probe", true)
        .await
        .unwrap();
    assert_eq!(probe(&platform, tenant_id, vec!["probe.read".into()]).await, StatusCode::OK);
}

#[tokio::test]
async fn disabling_one_tenant_does_not_disable_another() {
    let platform = boot().await;
    let tenant_a = fresh_tenant(&platform).await;
    let tenant_b = fresh_tenant(&platform).await;
    platform
        .lifecycle
        .set_enabled(tenant_b, Actor::system(), "probe", false)
        .await
        .unwrap();
    assert_eq!(probe(&platform, tenant_a, vec!["*".into()]).await, StatusCode::OK);
    assert_eq!(probe(&platform, tenant_b, vec!["*".into()]).await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_plugin_added_to_an_existing_tenant_starts_disabled() {
    let platform = boot().await;
    let tenant_id = fresh_tenant(&platform).await;

    // Simulate a later build that compiles one more plugin: drop its row, then install again.
    // The tenant already has rows, so the newly discovered plugin must come up off.
    sqlx::query("DELETE FROM tenant_plugins WHERE tenant_id = $1 AND plugin_id = 'probe'")
        .bind(tenant_id.as_uuid())
        .execute(platform.migration_pool())
        .await
        .unwrap();

    let added = PluginStateStore::install_missing(platform.runtime_pool(), tenant_id, &platform.catalog)
        .await
        .unwrap();
    assert_eq!(added.len(), 1, "only the missing plugin is inserted");

    let (enabled,): (bool,) = sqlx::query_as(
        "SELECT enabled FROM tenant_plugins WHERE tenant_id = $1 AND plugin_id = 'probe'",
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert!(!enabled, "a plugin added later must not go live without consent");
}

#[tokio::test]
async fn core_is_enabled_for_existing_tenants_and_cannot_be_disabled() {
    let platform = boot().await;
    let tenant_id = TenantId(Uuid::new_v4());
    sqlx::query("SELECT platform_provision_tenant($1, $2, $3)")
        .bind(tenant_id.as_uuid())
        .bind(format!("core-{}", &tenant_id.to_string()[..8]))
        .bind("Core Shop")
        .execute(platform.migration_pool())
        .await
        .expect("provision");
    sqlx::query(
        "INSERT INTO tenant_plugins (tenant_id, plugin_id, enabled, version) VALUES ($1, 'core', FALSE, '0.1.0'), ($1, 'probe', FALSE, '0.1.0')",
    )
    .bind(tenant_id.as_uuid())
    .execute(platform.migration_pool())
    .await
    .expect("existing plugin rows");
    sqlx::raw_sql(include_str!("../src/migrations/core/016_enable_core.sql"))
        .execute(platform.migration_pool())
        .await
        .expect("repair core enablement");

    PluginStateStore::install_missing(platform.runtime_pool(), tenant_id, &platform.catalog)
        .await
        .expect("install plugins");
    let enabled = platform.enabled.ids(tenant_id).await.expect("enabled ids");
    assert!(enabled.iter().any(|id| id.as_str() == "core"));
    assert!(!enabled.iter().any(|id| id.as_str() == "probe"));

    let error = platform
        .lifecycle
        .set_enabled(tenant_id, Actor::system(), "core", false)
        .await
        .expect_err("core cannot be disabled");
    assert!(matches!(error, AppError::Forbidden(_)), "{error}");
}

#[tokio::test]
async fn core_settings_work_without_catalog_or_pos() {
    let platform = boot().await;
    let tenant_id = fresh_tenant(&platform).await;
    for plugin in ["reports", "cash", "pos", "inventory", "catalog"] {
        platform
            .lifecycle
            .set_enabled(tenant_id, Actor::system(), plugin, false)
            .await
            .unwrap_or_else(|error| panic!("disable {plugin}: {error}"));
    }

    let mut request = Request::builder()
        .uri("/api/v1/core/snapshot")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthUser {
        tenant_id,
        user_id: 1,
        name: "core-settings".into(),
        role: "manager".into(),
        permissions: vec!["*".into()],
    });
    let response = platform
        .plugin_router()
        .oneshot(request)
        .await
        .expect("core snapshot");
    assert_eq!(response.status(), StatusCode::OK);

    let mut request = Request::builder()
        .method("PUT")
        .uri("/api/v1/core/settings")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "name": "Independent Shop",
                "bill_prefix": "CORE",
                "currency": "PKR",
                "locale": "en-PK",
                "barcode_rules": [{ "prefix": "21", "mode": "weight", "scale": 1000 }]
            })
            .to_string(),
        ))
        .unwrap();
    request.extensions_mut().insert(AuthUser {
        tenant_id,
        user_id: 1,
        name: "core-settings".into(),
        role: "manager".into(),
        permissions: vec!["*".into()],
    });
    let response = platform
        .plugin_router()
        .oneshot(request)
        .await
        .expect("core settings");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
    let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved["shop_name"], "Independent Shop");
    assert_eq!(saved["bill_prefix"], "CORE");
    assert_eq!(saved["barcode_rules"][0]["scale"], 1000);
}

#[tokio::test]
async fn upgrade_adopts_the_compiled_version_but_refuses_a_downgrade() {
    let platform = boot().await;
    let tenant_id = fresh_tenant(&platform).await;

    // Older than the compiled version: adoption moves it forward so the plugin stays enabled.
    sqlx::query(
        "UPDATE tenant_plugins SET version = '0.0.9', installed_version = '0.0.9' WHERE tenant_id = $1 AND plugin_id = 'probe'",
    )
    .bind(tenant_id.as_uuid())
    .execute(platform.migration_pool())
    .await
    .unwrap();

    let adopted = PluginStateStore::adopt_versions(platform.runtime_pool(), tenant_id, &platform.catalog)
        .await
        .unwrap();
    assert!(
        adopted
            .iter()
            .any(|(plugin, from, to)| plugin == "probe" && from == "0.0.9" && to == "0.1.0"),
        "an upgrade is adopted: {adopted:?}"
    );
    let stored: String = sqlx::query_scalar(
        "SELECT version FROM tenant_plugins WHERE tenant_id = $1 AND plugin_id = 'probe'",
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert_eq!(stored, "0.1.0");

    // Newer than the compiled version: a downgrade is left alone for the skip path to protect.
    sqlx::query(
        "UPDATE tenant_plugins SET version = '9.0.0', installed_version = '9.0.0' WHERE tenant_id = $1 AND plugin_id = 'probe'",
    )
    .bind(tenant_id.as_uuid())
    .execute(platform.migration_pool())
    .await
    .unwrap();

    let adopted = PluginStateStore::adopt_versions(platform.runtime_pool(), tenant_id, &platform.catalog)
        .await
        .unwrap();
    assert!(
        adopted.iter().all(|(plugin, _, _)| plugin != "probe"),
        "a downgrade must not be adopted"
    );
    let stored: String = sqlx::query_scalar(
        "SELECT version FROM tenant_plugins WHERE tenant_id = $1 AND plugin_id = 'probe'",
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert_eq!(stored, "9.0.0");
}

#[tokio::test]
async fn permission_sync_is_idempotent() {
    let platform = boot().await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM platform_permissions")
        .fetch_one(platform.migration_pool())
        .await
        .unwrap();
    let again = boot().await;
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM platform_permissions")
        .fetch_one(again.migration_pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    let owner: String = sqlx::query_scalar("SELECT plugin_id FROM platform_permissions WHERE name = 'probe.read'")
        .fetch_one(again.migration_pool())
        .await
        .unwrap();
    assert_eq!(owner, "probe");
}
