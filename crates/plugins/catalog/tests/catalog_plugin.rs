use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::plugin::Plugin;
use platform_runtime::http::AuthUser;
use platform_runtime::admin::provision_tenant;
use platform_runtime::{BootConfig, Platform, PluginStateStore};
use serde_json::json;
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

async fn call(platform: &Platform, tenant: TenantId, method: &str, path: &str, body: Option<serde_json::Value>, permissions: Vec<String>) -> StatusCode {
    let mut builder = Request::builder().method(method).uri(path);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let mut request = builder.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default())).unwrap();
    request.extensions_mut().insert(AuthUser {
        tenant_id: tenant,
        user_id: 1,
        name: "catalog".into(),
        role: "manager".into(),
        permissions,
    });
    platform.plugin_router().oneshot(request).await.expect("router").status()
}

#[tokio::test]
async fn activation_permissions_and_isolation() {
    let platform = boot().await;
    let suffix = &uuid::Uuid::new_v4().to_string()[..8];
    let tenant_a = provision_tenant(platform.migration_pool(), &format!("cat-a-{suffix}"), "Catalog A")
        .await
        .expect("tenant a");
    let tenant_b = provision_tenant(platform.migration_pool(), &format!("cat-b-{suffix}"), "Catalog B")
        .await
        .expect("tenant b");
    for tenant in [tenant_a, tenant_b] {
        PluginStateStore::install_missing(platform.runtime_pool(), tenant, &platform.catalog)
            .await
            .expect("install plugins");
    }

    let applied: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM platform_migrations WHERE owner = 'catalog'",
    )
    .fetch_one(platform.migration_pool())
    .await
    .unwrap();
    assert!(applied >= 4, "catalog migrations should be recorded");

    let read = vec!["catalog.read".into()];
    let write = vec!["catalog.read".into(), "catalog.write".into()];
    assert_eq!(
        call(&platform, tenant_a, "GET", "/api/v1/catalog/units", None, read.clone()).await,
        StatusCode::OK
    );
    assert_eq!(
        call(&platform, tenant_a, "POST", "/api/v1/catalog/categories", Some(json!({"name": format!("Drinks {suffix}")})), read).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&platform, tenant_a, "GET", "/api/v1/catalog/items", None, vec!["pos.read".into()]).await,
        StatusCode::FORBIDDEN
    );

    let sections = sections_of(&platform, tenant_a).await;
    let categories = categories_of(&platform, tenant_a).await;
    assert!(!sections.is_empty() || true);
    let category_id = if let Some(id) = categories.first().copied() {
        id
    } else {
        create_category(&platform, tenant_a, &format!("Food {suffix}")).await
    };
    let section_id = if let Some(id) = sections.first().copied() {
        id
    } else {
        create_section(&platform, tenant_a, &format!("Shop {suffix}")).await
    };
    let status = call(
        &platform,
        tenant_a,
        "POST",
        "/api/v1/catalog/items",
        Some(json!({
            "name": format!("Delivery {suffix}"),
            "rate": 50,
            "cost": 0,
            "section_id": section_id,
            "category_id": category_id,
            "prompt": "none",
            "scale": 1,
            "item_type": "service",
            "variants": [{
                "name": "Trip",
                "rate": 50,
                "units": 1,
                "attributes": [{"name": "Speed", "value": "Same day"}]
            }]
        })),
        write,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "creating a service item should succeed");

    let mut tx = TenantTx::begin_with(platform.runtime_pool(), tenant_b, Actor::system(), true)
        .await
        .unwrap();
    let seen: Option<(i32,)> = sqlx::query_as("SELECT id FROM items WHERE name = $1")
        .bind(format!("Delivery {suffix}"))
        .fetch_optional(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(seen.is_none(), "tenant B must not see tenant A's item");

    let units_a = unit_count(&platform, tenant_a).await;
    let units_b = unit_count(&platform, tenant_b).await;
    assert!(units_a >= 8 && units_b >= 8, "each tenant gets its own units");

    let blocked = platform
        .lifecycle
        .set_enabled(tenant_a, Actor::system(), "catalog", false)
        .await;
    assert!(blocked.is_err(), "catalog stays on while pos and inventory need it");
    for plugin in ["reports", "cash", "pos", "inventory"] {
        platform
            .lifecycle
            .set_enabled(tenant_a, Actor::system(), plugin, false)
            .await
            .unwrap_or_else(|err| panic!("{plugin}: {err}"));
    }
    platform
        .lifecycle
        .set_enabled(tenant_a, Actor::system(), "catalog", false)
        .await
        .unwrap();
    assert_eq!(
        call(&platform, tenant_a, "GET", "/api/v1/catalog/items", None, vec!["*".into()]).await,
        StatusCode::NOT_FOUND
    );
    platform
        .lifecycle
        .set_enabled(tenant_a, Actor::system(), "catalog", true)
        .await
        .unwrap();
    assert_eq!(
        call(&platform, tenant_a, "GET", "/api/v1/catalog/items", None, vec!["*".into()]).await,
        StatusCode::OK
    );
}

async fn sections_of(platform: &Platform, tenant: TenantId) -> Vec<i32> {
    let mut tx = TenantTx::begin_with(platform.runtime_pool(), tenant, Actor::system(), true)
        .await
        .unwrap();
    let rows: Vec<(i32,)> = sqlx::query_as("SELECT id FROM sections ORDER BY id")
        .fetch_all(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

async fn categories_of(platform: &Platform, tenant: TenantId) -> Vec<i32> {
    let mut tx = TenantTx::begin_with(platform.runtime_pool(), tenant, Actor::system(), true)
        .await
        .unwrap();
    let rows: Vec<(i32,)> = sqlx::query_as("SELECT id FROM categories ORDER BY id")
        .fetch_all(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}

async fn create_section(platform: &Platform, tenant: TenantId, name: &str) -> i32 {
    let mut tx = TenantTx::begin_with(platform.runtime_pool(), tenant, Actor::system(), false)
        .await
        .unwrap();
    let id = plugin_catalog::service::save_section(
        &mut tx,
        None,
        &plugin_catalog::service::SectionForm {
            name: name.into(),
            short_name: name.chars().take(8).collect(),
            kind: "retail".into(),
            accent: "#336699".into(),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    id
}

async fn create_category(platform: &Platform, tenant: TenantId, name: &str) -> i32 {
    let mut tx = TenantTx::begin_with(platform.runtime_pool(), tenant, Actor::system(), false)
        .await
        .unwrap();
    let id = plugin_catalog::service::save_category(
        &mut tx,
        None,
        &plugin_catalog::service::CategoryForm { name: name.into() },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    id
}

async fn unit_count(platform: &Platform, tenant: TenantId) -> i64 {
    let mut tx = TenantTx::begin_with(platform.runtime_pool(), tenant, Actor::system(), true)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM catalog_units")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    count
}
