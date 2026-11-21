use platform_kernel::error::AppError;
use platform_kernel::registry::PluginCatalog;
use sqlx::PgPool;

/// Upsert every permission a compiled-in plugin declares. Safe to run on every boot.
pub async fn sync(pool: &PgPool, catalog: &PluginCatalog) -> Result<(), AppError> {
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
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}
