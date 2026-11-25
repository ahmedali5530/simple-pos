use platform_kernel::error::AppError;
use platform_kernel::registry::PluginCatalog;
use sqlx::PgPool;

use super::BOOT_STATE_LOCK;

/// Upsert every permission a compiled-in plugin declares. Safe to run on every boot.
/// The advisory lock keeps parallel boots from updating the same rows at once.
pub async fn sync(pool: &PgPool, catalog: &PluginCatalog) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(BOOT_STATE_LOCK)
        .execute(&mut *tx)
        .await?;
    for manifest in catalog.manifests() {
        for permission in &manifest.permissions {
            sqlx::query(
                r#"
                INSERT INTO platform_permissions (name, description, plugin_id)
                VALUES ($1, $2, $3)
                ON CONFLICT (name) DO UPDATE
                SET plugin_id = EXCLUDED.plugin_id
                "#,
            )
            .bind(permission.as_str())
            .bind(format!("{} permission", manifest.name))
            .bind(manifest.id.as_str())
            .execute(&mut *tx)
            .await?;
        }
    }
    // A newly compiled plugin's permissions must reach existing managers without
    // requiring an unrelated user edit. Cashiers keep the explicit role template.
    sqlx::query(
        r#"
        INSERT INTO role_permissions (tenant_id, role_id, permission)
        SELECT r.tenant_id, r.id, p.name
        FROM roles r
        CROSS JOIN platform_permissions p
        WHERE r.name = 'manager'
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
