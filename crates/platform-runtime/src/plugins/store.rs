use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_kernel::manifest::PluginId;
use platform_kernel::registry::{PersistedPlugin, PluginCatalog};
use semver::Version;
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

    /// Insert compiled-in plugins that have no row yet.
    ///
    /// A tenant with no plugin rows (a fresh till) gets every plugin enabled. A tenant that
    /// already has rows gets new plugins **disabled**, so code installed later never goes live
    /// until the operator turns it on. Existing rows, including disabled ones, are untouched.
    /// Returns the ids that were added.
    pub async fn install_missing(
        pool: &PgPool,
        tenant_id: TenantId,
        catalog: &PluginCatalog,
    ) -> Result<Vec<PluginId>, AppError> {
        let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(super::BOOT_STATE_LOCK)
            .execute(tx.conn())
            .await?;
        let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenant_plugins")
            .fetch_one(tx.conn())
            .await?;
        let fresh_tenant = existing == 0;
        let mut added = Vec::new();
        for id in catalog.topo_order() {
            let version = catalog
                .manifest(id)
                .map(|manifest| manifest.version.to_string())
                .unwrap_or_else(|| "0.0.0".into());
            let inserted: Option<String> = sqlx::query_scalar(
                r#"
                INSERT INTO tenant_plugins (
                    tenant_id, plugin_id, enabled, version, installed_version, installed_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $4, now(), now())
                ON CONFLICT (tenant_id, plugin_id) DO NOTHING
                RETURNING plugin_id
                "#,
            )
            .bind(tenant_id.as_uuid())
            .bind(id.as_str())
            .bind(fresh_tenant)
            .bind(&version)
            .fetch_optional(tx.conn())
            .await?;
            if inserted.is_some() {
                added.push(id.clone());
            }
        }
        tx.commit().await?;
        Ok(added)
    }

    /// Move stored plugin versions forward to the compiled version.
    ///
    /// `EnabledSet::from_persisted` skips a row whose stored version is not compatible with the
    /// compiled one, so an upgrade would silently disable the plugin. Adoption records that this
    /// binary runs the plugin. A stored version **newer** than the compiled one is a downgrade and
    /// is left alone, so the existing skip still protects data written by a newer binary.
    /// Returns `(plugin_id, from, to)` for every adoption.
    pub async fn adopt_versions(
        pool: &PgPool,
        tenant_id: TenantId,
        catalog: &PluginCatalog,
    ) -> Result<Vec<(String, String, String)>, AppError> {
        let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(super::BOOT_STATE_LOCK)
            .execute(tx.conn())
            .await?;
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT plugin_id, version FROM tenant_plugins")
                .fetch_all(tx.conn())
                .await?;
        let mut adopted = Vec::new();
        for (plugin_id, stored) in rows {
            let Ok(id) = PluginId::parse(&plugin_id) else {
                continue;
            };
            let Some(manifest) = catalog.manifest(&id) else {
                continue;
            };
            let compiled = manifest.version.clone();
            let Ok(stored_version) = Version::parse(&stored) else {
                continue;
            };
            if stored_version == compiled || stored_version > compiled {
                continue;
            }
            sqlx::query(
                r#"
                UPDATE tenant_plugins
                SET version = $2, installed_version = $2, updated_at = now()
                WHERE tenant_id = $1 AND plugin_id = $3
                "#,
            )
            .bind(tenant_id.as_uuid())
            .bind(compiled.to_string())
            .bind(&plugin_id)
            .execute(tx.conn())
            .await?;
            adopted.push((plugin_id, stored, compiled.to_string()));
        }
        tx.commit().await?;
        Ok(adopted)
    }
}
