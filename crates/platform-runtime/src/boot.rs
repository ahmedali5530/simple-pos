use std::sync::Arc;

use platform_kernel::error::AppError;
use platform_kernel::plugin::Plugin;
use platform_kernel::registry::PluginRegistry;
use sqlx::PgPool;

use crate::db::{rls_validator, roles, Pools};
use crate::events::EventDispatcher;
use crate::gateway::CapabilityGateway;
use crate::migrations;

pub struct BootConfig {
    pub database_url: String,
    pub database_admin_url: Option<String>,
    pub app_password: Option<String>,
}

fn rewrite_user(url: &str, user: &str, password: &str) -> Option<String> {
    // postgres://user:pass@host/db → postgres://platform_app:pass@host/db
    let rest = url.split("://").nth(1)?;
    let hostpart = rest.split('@').nth(1)?;
    Some(format!("postgres://{user}:{password}@{hostpart}"))
}

pub struct Platform {
    pub pools: Pools,
    pub registry: PluginRegistry,
    pub gateway: CapabilityGateway,
    pub stop_dispatcher: Option<tokio::sync::watch::Sender<bool>>,
}

impl Platform {
    pub async fn boot(
        config: BootConfig,
        plugins: Vec<Arc<dyn Plugin>>,
        enable_all: bool,
    ) -> Result<Self, AppError> {
        // 1. Legacy sqlx migrations are run by the caller (shop-core::connect) first.
        let admin_url = config
            .database_admin_url
            .clone()
            .unwrap_or_else(|| config.database_url.clone());

        // Always open an admin pool first so we can provision platform_app.
        let admin_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&admin_url)
            .await?;

        let app_password = config
            .app_password
            .clone()
            .unwrap_or_else(|| "platform-app-local".into());
        let _ = roles::provision(&admin_pool, &app_password).await;

        let app_url = if config.database_admin_url.is_some()
            && config.database_admin_url.as_deref() != Some(config.database_url.as_str())
        {
            config.database_url.clone()
        } else {
            // Rewrite admin URL to log in as platform_app for the runtime pool.
            rewrite_user(&config.database_url, "platform_app", &app_password)
                .unwrap_or_else(|| config.database_url.clone())
        };

        let pools = match Pools::connect(&admin_url, &app_url).await {
            Ok(pools) => pools,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "could not connect as platform_app; falling back to shared admin pool (RLS may be bypassed)"
                );
                Pools {
                    migration: admin_pool.clone(),
                    runtime: admin_pool.clone(),
                }
            }
        };

        // 2. Platform migrations (as admin), then refresh grants for platform_app.
        migrations::run_owner(&pools.migration, "core", migrations::core::migrations()).await?;
        let _ = roles::provision(&pools.migration, &app_password).await;

        // 3. Plugin registry
        let mut registry = PluginRegistry::new();
        for plugin in &plugins {
            registry.add(plugin.clone());
        }
        if enable_all {
            // Enable in dependency order.
            for id in ["catalog", "inventory", "pos", "cash", "printing", "reports"] {
                let _ = registry.enable(id);
            }
            for plugin in &plugins {
                let id = plugin.manifest().id;
                if !registry.is_enabled(&id) {
                    let _ = registry.enable(&id);
                }
            }
        } else {
            // Load enabled set from default tenant
            let enabled: Vec<(String,)> = sqlx::query_as(
                r#"
                SELECT plugin_id FROM tenant_plugins
                WHERE tenant_id = '00000000-0000-4000-8000-000000000001' AND enabled
                "#,
            )
            .fetch_all(&pools.migration)
            .await
            .unwrap_or_default();
            for (id,) in enabled {
                let _ = registry.enable(&id);
            }
        }

        let enabled_plugins: Vec<_> = registry
            .enabled_ids()
            .into_iter()
            .filter_map(|id| registry.plugin(&id))
            .collect();
        migrations::run_plugins(&pools.migration, &enabled_plugins).await?;

        registry.register_all();
        rls_validator::validate(&pools.migration).await?;
        pools.guard_runtime().await.ok();

        let gateway = CapabilityGateway::new(
            pools.runtime.clone(),
            std::mem::take(&mut registry.capabilities),
        );

        let handlers = registry.registrar.event_handlers.clone();
        let dispatcher = EventDispatcher::new(pools.runtime.clone(), handlers);
        let stop_dispatcher = Some(dispatcher.spawn());

        Ok(Self {
            pools,
            registry,
            gateway,
            stop_dispatcher,
        })
    }

    pub fn runtime_pool(&self) -> &PgPool {
        &self.pools.runtime
    }

    pub fn migration_pool(&self) -> &PgPool {
        &self.pools.migration
    }

    pub fn stop(&self) {
        if let Some(stop) = &self.stop_dispatcher {
            let _ = stop.send(true);
        }
    }
}
