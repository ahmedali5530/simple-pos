//! Cross-tenant isolation suite. Requires DATABASE_URL (admin) and provisions platform_app.

use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_runtime::{BootConfig, Platform};
use std::sync::Arc;
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL")
}

async fn boot() -> Platform {
    use platform_kernel::plugin::Plugin;
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
        true,
    )
    .await
    .expect("platform boot")
}

#[tokio::test]
async fn tenant_a_cannot_read_tenant_b_sections() {
    let platform = boot().await;
    let pool = platform.runtime_pool();

    // Confirm we are not bypassing RLS.
    let (name, bypass): (String, bool) = sqlx::query_as(
        "SELECT rolname, rolbypassrls FROM pg_roles WHERE rolname = current_user",
    )
    .fetch_one(pool)
    .await
    .expect("role");
    assert_eq!(name, "platform_app");
    assert!(!bypass, "platform_app must not bypass RLS");

    let tenant_b = TenantId(Uuid::new_v4());
    // Provision via admin/migration pool (SECURITY DEFINER).
    sqlx::query("SELECT platform_provision_tenant($1, $2, $3)")
        .bind(tenant_b.as_uuid())
        .bind(format!("iso-{}", &tenant_b.to_string()[..8]))
        .bind("Isolation Shop")
        .execute(platform.migration_pool())
        .await
        .expect("provision");

    {
        let mut tx = TenantTx::begin_with(pool, tenant_b, Actor::system(), false)
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO sections (name, short_name, kind, accent, show_on_pos, pos_order)
            VALUES ('Secret', 'SecretIso', 'retail', '#000000', TRUE, 0)
            "#,
        )
        .execute(tx.conn())
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let mut tx = TenantTx::begin_with(pool, TenantId::DEFAULT, Actor::system(), true)
        .await
        .unwrap();
    let found: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM sections WHERE short_name = 'SecretIso'")
            .fetch_optional(tx.conn())
            .await
            .unwrap();
    tx.commit().await.unwrap();
    assert!(found.is_none(), "tenant A must not see tenant B section");
}

#[tokio::test]
async fn insert_without_tenant_context_fails_or_is_invisible() {
    let platform = boot().await;
    let pool = platform.runtime_pool();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.tenant_id', '', true)")
        .execute(&mut *tx)
        .await
        .ok();
    let count: Result<i64, _> = sqlx::query_scalar("SELECT COUNT(*) FROM sections")
        .fetch_one(&mut *tx)
        .await;
    let _ = tx.rollback().await;
    match count {
        Ok(n) => assert_eq!(n, 0, "no-context must see zero rows, got {n}"),
        Err(_) => {}
    }
}
