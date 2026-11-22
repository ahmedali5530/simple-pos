use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use platform_kernel::context::{Actor, TenantId};
use platform_kernel::plugin::Plugin;
use platform_runtime::admin::provision_tenant;
use platform_runtime::http::AuthUser;
use platform_runtime::{BootConfig, Platform, PluginStateStore};
use tower::ServiceExt;

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL")
}

async fn boot() -> Platform {
    let plugins: Vec<Arc<dyn Plugin>> = vec![
        Arc::new(plugin_catalog::CatalogPlugin),
        Arc::new(plugin_inventory::InventoryPlugin),
        Arc::new(plugin_pos::PosPlugin),
        Arc::new(plugin_cash::CashPlugin),
        Arc::new(plugin_printing::PrintingPlugin),
        Arc::new(plugin_reports::ReportsPlugin),
    ];
    Platform::boot(
        BootConfig {
            database_url: database_url(),
            database_admin_url: Some(database_url()),
            app_password: Some("platform-app-local".into()),
        },
        plugins,
    )
    .await
    .expect("boot")
}

async fn call(platform: &Platform, tenant: TenantId, method: &str, path: &str) -> StatusCode {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::from("{}"))
        .unwrap();
    request.extensions_mut().insert(AuthUser {
        tenant_id: tenant,
        user_id: 1,
        name: "till".into(),
        role: "manager".into(),
        permissions: vec!["*".into()],
    });
    platform.plugin_router().oneshot(request).await.expect("router").status()
}

#[tokio::test]
async fn disabling_pos_unregisters_bill_settlement() {
    let platform = boot().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let tenant = provision_tenant(platform.migration_pool(), &format!("pos-{suffix}"), "POS Shop")
        .await
        .expect("tenant");
    PluginStateStore::install_missing(platform.runtime_pool(), tenant, &platform.catalog)
        .await
        .expect("install");

    assert_eq!(
        call(&platform, tenant, "GET", "/api/v1/pos/snapshot").await,
        StatusCode::OK
    );
    assert_ne!(
        call(&platform, tenant, "POST", "/api/v1/pos/bills").await,
        StatusCode::NOT_FOUND
    );

    for plugin in ["reports", "cash"] {
        platform
            .lifecycle
            .set_enabled(tenant, Actor::system(), plugin, false)
            .await
            .unwrap_or_else(|err| panic!("{plugin}: {err}"));
    }
    platform
        .lifecycle
        .set_enabled(tenant, Actor::system(), "pos", false)
        .await
        .expect("disable pos");

    assert_eq!(
        call(&platform, tenant, "POST", "/api/v1/pos/bills").await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&platform, tenant, "GET", "/api/v1/pos/snapshot").await,
        StatusCode::NOT_FOUND
    );
}
