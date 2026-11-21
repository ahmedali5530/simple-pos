use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_kernel::manifest::PluginId;
use platform_kernel::registry::{PersistedPlugin, PluginCatalog};
use sqlx::PgPool;

pub struct PluginStateStore;

impl PluginStateStore {
    pub async fn load(tx: &mut TenantTx<'_>, lock: bool) -> Result<Vec<PersistedPlugin>, AppError> {
        let sql = if lock {
            "SELECT plugin_id, enabled, version FROM tenant_plugins FOR UPDATE"
        } else {
            "SELECT plugin_id, enabled, version FROM tenant_plugins"
        };
        let rows: Vec<(String, bool, String)> = sqlx::query_as(sql).fetch_all(tx.conn()).await?;
        let mut plugins = Vec::new();
        for (id, enabled, version) in rows {
            let Ok(id) = PluginId::parse(&id) else {
                continue;
            };
            plugins.push(PersistedPlugin { id, enabled, version });
        }
        Ok(plugins)
    }

    pub async fn upsert(
        tx: &mut TenantTx<'_>,
        tenant_id: TenantId,
        plugin_id: &PluginId,
        enabled: bool,
        version: &str,
        actor: &Actor,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO tenant_plugins (
                tenant_id, plugin_id, enabled, version, installed_version, updated_at, updated_by
            )
            VALUES ($1, $2, $3, $4, $4, now(), $5)
            ON CONFLICT (tenant_id, plugin_id) DO UPDATE
            SET enabled = EXCLUDED.enabled,
                version = EXCLUDED.version,
                installed_version = EXCLUDED.installed_version,
                updated_at = now(),
                updated_by = EXCLUDED.updated_by
            "#,
        )
        .bind(tenant_id.as_uuid())
        .bind(plugin_id.as_str())
        .bind(enabled)
        .bind(version)
        .bind(actor.user_id)
        .execute(tx.conn())
        .await?;
        Ok(())
    }

    /// Insert compiled-in plugins that have no row yet, as enabled.
    /// Existing rows, including disabled ones, are left untouched.
    pub async fn install_missing(
        pool: &PgPool,
        tenant_id: TenantId,
        catalog: &PluginCatalog,
    ) -> Result<(), AppError> {
        let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
        for id in catalog.topo_order() {
            let version = catalog
                .manifest(id)
                .map(|manifest| manifest.version.to_string())
                .unwrap_or_else(|| "0.0.0".into());
            sqlx::query(
                r#"
                INSERT INTO tenant_plugins (
                    tenant_id, plugin_id, enabled, version, installed_version, installed_at, updated_at
                )
                VALUES ($1, $2, TRUE, $3, $3, now(), now())
                ON CONFLICT (tenant_id, plugin_id) DO NOTHING
                "#,
            )
            .bind(tenant_id.as_uuid())
            .bind(id.as_str())
            .bind(&version)
            .execute(tx.conn())
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
