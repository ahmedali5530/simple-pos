use std::collections::HashSet;
use std::sync::Arc;

use axum::Router;
use platform_kernel::context::TenantId;
use platform_kernel::error::AppError;
use platform_kernel::events::PluginEvents;
use platform_kernel::plugin::Plugin;
use platform_kernel::registry::PluginCatalog;
use platform_kernel::routes::PluginRoutes;
use platform_kernel::Registrar;
use sqlx::PgPool;

use crate::db::{rls_validator, roles, Pools};
use crate::events::EventDispatcher;
use crate::gateway::CapabilityGateway;
use crate::http::plugin_gate;
use crate::migrations;
use crate::plugins::{sync_permissions, EnabledPlugins, PluginLifecycle, PluginStateStore};

pub struct BootConfig {
    pub database_url: String,
    pub database_admin_url: Option<String>,
    pub app_password: Option<String>,
}

fn rewrite_user(url: &str, user: &str, password: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let hostpart = rest.split('@').nth(1)?;
    Some(format!("postgres://{user}:{password}@{hostpart}"))
}

pub struct Platform {
    pub pools: Pools,
    pub catalog: Arc<PluginCatalog>,
    pub gateway: CapabilityGateway,
    pub lifecycle: PluginLifecycle,
    pub enabled: Arc<EnabledPlugins>,
    pub plugin_router: Router,
    pub stop_dispatcher: Option<tokio::sync::watch::Sender<bool>>,
}

impl Platform {
    pub async fn boot(config: BootConfig, plugins: Vec<Arc<dyn Plugin>>) -> Result<Self, AppError> {
        let catalog = Arc::new(PluginCatalog::build(plugins)?);

        let admin_url = config
            .database_admin_url
            .clone()
            .unwrap_or_else(|| config.database_url.clone());

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

        migrations::run_owner(&pools.migration, "core", migrations::core::migrations()).await?;
        let _ = roles::provision(&pools.migration, &app_password).await;

        for plugin in catalog.plugins_in_order() {
            let manifest = plugin.manifest();
            migrations::run_owner(&pools.migration, manifest.id.as_str(), plugin.migrations()).await?;
        }
        sync_permissions(&pools.migration, &catalog).await?;
        PluginStateStore::install_missing(&pools.runtime, TenantId::DEFAULT, &catalog).await?;

        let enabled = Arc::new(EnabledPlugins::new(pools.runtime.clone(), catalog.clone()));
        let lifecycle = PluginLifecycle::new(pools.runtime.clone(), catalog.clone(), enabled.clone());

        let mut registrar = Registrar::new();
        let mut plugin_router = Router::new();
        let mut handlers = Vec::new();
        for plugin in catalog.plugins_in_order() {
            plugin.register(&mut registrar);
            let manifest = plugin.manifest();
            let declared: HashSet<String> = manifest
                .permissions
                .iter()
                .map(|permission| permission.as_str().to_string())
                .collect();
            let mut routes = PluginRoutes::new(manifest.id.clone(), declared);
            plugin.routes(&mut routes)?;
            let (router, specs) = routes.finish();
            plugin_router = plugin_router.merge(plugin_gate::wrap(
                manifest.id.clone(),
                specs,
                enabled.clone(),
                router,
            ));

            let consumed: HashSet<String> = manifest.events_consumed.iter().cloned().collect();
            let mut events = PluginEvents::new(manifest.id.clone(), consumed);
            plugin.event_handlers(&mut events)?;
            handlers.extend(events.into_handlers());
        }
        handlers.extend(registrar.event_handlers.drain(..));

        let mut capabilities = std::mem::take(&mut registrar.capabilities);
        let capability_registry = {
            let mut registry = platform_kernel::CapabilityRegistry::new();
            for (spec, handler) in capabilities.drain(..) {
                registry.register(spec, handler);
            }
            registry
        };

        rls_validator::validate(&pools.migration).await?;
        pools.guard_runtime().await.ok();

        let gateway = CapabilityGateway::new(pools.runtime.clone(), capability_registry);
        let dispatcher = EventDispatcher::new(pools.runtime.clone(), handlers, enabled.clone());
        let stop_dispatcher = Some(dispatcher.spawn());

        Ok(Self {
            pools,
            catalog,
            gateway,
            lifecycle,
            enabled,
            plugin_router,
            stop_dispatcher,
        })
    }

    pub fn runtime_pool(&self) -> &PgPool {
        &self.pools.runtime
    }

    pub fn migration_pool(&self) -> &PgPool {
        &self.pools.migration
    }

    pub fn plugin_router(&self) -> Router {
        self.plugin_router.clone()
    }

    pub fn stop(&self) {
        if let Some(stop) = &self.stop_dispatcher {
            let _ = stop.send(true);
        }
    }
}
