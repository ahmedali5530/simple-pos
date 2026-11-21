use std::sync::Arc;

use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_kernel::manifest::PluginId;
use platform_kernel::permission::builtins;
use platform_kernel::registry::{EnabledSet, PluginCatalog};
use sqlx::PgPool;

use super::cache::EnabledPlugins;
use super::store::PluginStateStore;

/// Enables and disables plugins for one tenant.
///
/// The row update and the audit entry commit together. A rejected change writes nothing.
/// Disabling never deletes the plugin row or its data.
#[derive(Clone)]
pub struct PluginLifecycle {
    pool: PgPool,
    catalog: Arc<PluginCatalog>,
    enabled: Arc<EnabledPlugins>,
}

impl PluginLifecycle {
    pub fn new(pool: PgPool, catalog: Arc<PluginCatalog>, enabled: Arc<EnabledPlugins>) -> Self {
        Self {
            pool,
            catalog,
            enabled,
        }
    }

    pub async fn set_enabled(
        &self,
        tenant_id: TenantId,
        actor: Actor,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        if !actor.has_permission(builtins::PLATFORM_PLUGINS) {
            return Err(AppError::Forbidden("You cannot manage plugins.".into()));
        }
        let id = PluginId::parse(plugin_id).map_err(|err| AppError::BadRequest(err.to_string()))?;
        let version = self
            .catalog
            .manifest(&id)
            .map(|manifest| manifest.version.to_string())
            .ok_or_else(|| AppError::NotFound(format!("Plugin `{id}` is not compiled in.")))?;

        let mut tx = TenantTx::begin_with(&self.pool, tenant_id, actor.clone(), false).await?;
        let rows = PluginStateStore::load(&mut tx, true).await?;
        let (mut set, _) = EnabledSet::from_persisted(&self.catalog, &rows);
        let was_enabled = set.contains(&id);
        if enabled {
            set.enable(&self.catalog, &id)?;
        } else {
            set.disable(&self.catalog, &id)?;
        }
        if was_enabled == enabled {
            tx.commit().await?;
            self.enabled.set(tenant_id, set.ids()).await;
            return Ok(());
        }

        PluginStateStore::upsert(&mut tx, tenant_id, &id, enabled, &version, &actor).await?;
        let action = if enabled { "plugin.enabled" } else { "plugin.disabled" };
        sqlx::query(
            r#"
            INSERT INTO platform_audit_log (tenant_id, actor_id, action, detail)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(tenant_id.as_uuid())
        .bind(actor.user_id)
        .bind(action)
        .bind(serde_json::json!({
            "plugin_id": id.as_str(),
            "version": version,
            "previous": was_enabled,
        }))
        .execute(tx.conn())
        .await?;
        let ids = set.ids();
        tx.commit().await?;
        self.enabled.set(tenant_id, ids).await;
        Ok(())
    }
}
