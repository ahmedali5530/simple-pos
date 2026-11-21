use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_kernel::manifest::PluginId;
use platform_kernel::registry::{EnabledSet, PluginCatalog};
use sqlx::PgPool;
use tokio::sync::RwLock;

use super::store::PluginStateStore;

/// Per-tenant enabled plugin ids. Loaded lazily and replaced after a lifecycle change.
pub struct EnabledPlugins {
    inner: RwLock<HashMap<TenantId, HashSet<PluginId>>>,
    pool: PgPool,
    catalog: Arc<PluginCatalog>,
}

impl EnabledPlugins {
    pub fn new(pool: PgPool, catalog: Arc<PluginCatalog>) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            pool,
            catalog,
        }
    }

    pub async fn is_enabled(&self, tenant_id: TenantId, plugin_id: &PluginId) -> Result<bool, AppError> {
        {
            let guard = self.inner.read().await;
            if let Some(set) = guard.get(&tenant_id) {
                return Ok(set.contains(plugin_id));
            }
        }
        let loaded = self.load(tenant_id).await?;
        let mut guard = self.inner.write().await;
        let set = guard.entry(tenant_id).or_insert(loaded);
        Ok(set.contains(plugin_id))
    }

    pub async fn ids(&self, tenant_id: TenantId) -> Result<HashSet<PluginId>, AppError> {
        {
            let guard = self.inner.read().await;
            if let Some(set) = guard.get(&tenant_id) {
                return Ok(set.clone());
            }
        }
        let loaded = self.load(tenant_id).await?;
        let mut guard = self.inner.write().await;
        let set = guard.entry(tenant_id).or_insert(loaded);
        Ok(set.clone())
    }

    pub async fn set(&self, tenant_id: TenantId, enabled: HashSet<PluginId>) {
        self.inner.write().await.insert(tenant_id, enabled);
    }

    async fn load(&self, tenant_id: TenantId) -> Result<HashSet<PluginId>, AppError> {
        let mut tx = TenantTx::begin_with(&self.pool, tenant_id, Actor::system(), true).await?;
        let rows = PluginStateStore::load(&mut tx, false).await?;
        tx.commit().await?;
        let (set, _) = EnabledSet::from_persisted(&self.catalog, &rows);
        Ok(set.ids())
    }
}
