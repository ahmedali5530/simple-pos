use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_kernel::manifest::PluginManifest;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct PlatformManifest {
    pub tenant_id: Uuid,
    pub plugins: Vec<PluginManifest>,
    pub permissions: Vec<String>,
}

pub async fn platform_manifest(
    _pool: &PgPool,
    tenant_id: TenantId,
    actor: &Actor,
    manifests: Vec<PluginManifest>,
) -> Result<PlatformManifest, AppError> {
    Ok(PlatformManifest {
        tenant_id: tenant_id.as_uuid(),
        plugins: manifests,
        permissions: actor.permissions.clone(),
    })
}

pub async fn set_plugin_enabled(
    pool: &PgPool,
    tenant_id: TenantId,
    actor: Actor,
    plugin_id: &str,
    enabled: bool,
) -> Result<(), AppError> {
    if !actor.has_permission(platform_kernel::permission::builtins::PLATFORM_PLUGINS) {
        return Err(AppError::Forbidden(
            "You cannot manage plugins.".into(),
        ));
    }
    let mut tx = TenantTx::begin_with(pool, tenant_id, actor, false).await?;
    sqlx::query(
        r#"
        INSERT INTO tenant_plugins (tenant_id, plugin_id, enabled, version)
        VALUES ($1, $2, $3, '0.1.0')
        ON CONFLICT (tenant_id, plugin_id) DO UPDATE SET enabled = EXCLUDED.enabled
        "#,
    )
    .bind(tenant_id.as_uuid())
    .bind(plugin_id)
    .bind(enabled)
    .execute(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn provision_tenant(
    pool: &PgPool,
    code: &str,
    name: &str,
) -> Result<TenantId, AppError> {
    let id = Uuid::new_v4();
    let row: (Uuid,) = sqlx::query_as("SELECT platform_provision_tenant($1, $2, $3)")
        .bind(id)
        .bind(code)
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(TenantId(row.0))
}
