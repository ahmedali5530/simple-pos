use std::collections::{HashMap, HashSet};
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
use crate::http::plugin_gate::PluginRouteTable;
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
    pub plugin_routes: PluginRouteTable,
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
        roles::grant_existing(&pools.migration).await?;
        sync_permissions(&pools.migration, &catalog).await?;
        for id in PluginStateStore::install_missing(&pools.runtime, TenantId::DEFAULT, &catalog).await? {
            tracing::info!(plugin = id.as_str(), "plugin row added for this tenant");
        }
        for (plugin, from, to) in
            PluginStateStore::adopt_versions(&pools.runtime, TenantId::DEFAULT, &catalog).await?
        {
            tracing::info!(plugin = %plugin, from = %from, to = %to, "plugin version adopted");
        }

        let enabled = Arc::new(EnabledPlugins::new(pools.runtime.clone(), catalog.clone()));
        let lifecycle = PluginLifecycle::new(pools.runtime.clone(), catalog.clone(), enabled.clone());

        let mut registrar = Registrar::new();
        let mut route_mounts = Vec::new();
        let mut handlers = Vec::new();
        for plugin in catalog.plugins_in_order() {
            let manifest = plugin.manifest();
            registrar.begin_plugin(manifest.id.clone());
            plugin.register(&mut registrar);
            let declared: HashSet<String> = manifest
                .permissions
                .iter()
                .map(|permission| permission.as_str().to_string())
                .collect();
            let mut routes = PluginRoutes::new(manifest.id.clone(), declared);
            plugin.routes(&mut routes)?;
            let (router, specs) = routes.finish();
            route_mounts.push((manifest.id.clone(), specs, router));

            let consumed: HashSet<String> = manifest.events_consumed.iter().cloned().collect();
            let mut events = PluginEvents::new(manifest.id.clone(), consumed);
            plugin.event_handlers(&mut events)?;
            handlers.extend(events.into_handlers());
        }
        handlers.extend(registrar.event_handlers.drain(..));

        validate_registered_capabilities(&catalog, &registrar)?;

        let mut capabilities = std::mem::take(&mut registrar.capabilities);
        let capability_registry = {
            let mut registry = platform_kernel::CapabilityRegistry::new();
            let owners = std::mem::take(&mut registrar.capability_owners);
            for ((spec, handler), owner) in capabilities.drain(..).zip(owners) {
                if let Some(owner) = owner {
                    registry.register_owned(owner, spec, handler);
                } else {
                    registry.register(spec, handler);
                }
            }
            registry
        };

        rls_validator::validate(&pools.migration).await?;
        pools.guard_runtime().await.ok();

        let plugin_routes = PluginRouteTable::new(
            route_mounts,
            enabled.clone(),
            pools.runtime.clone(),
            registrar.contracts(),
        );
        let plugin_router = plugin_routes.router();
        let gateway = CapabilityGateway::new(pools.runtime.clone(), capability_registry, enabled.clone());
        let dispatcher = EventDispatcher::new(pools.runtime.clone(), handlers, enabled.clone());
        let stop_dispatcher = Some(dispatcher.spawn());

        Ok(Self {
            pools,
            catalog,
            gateway,
            lifecycle,
            enabled,
            plugin_router,
            plugin_routes,
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

fn validate_registered_capabilities(
    catalog: &PluginCatalog,
    registrar: &Registrar,
) -> Result<(), AppError> {
    let mut registered: HashMap<_, HashMap<String, (String, platform_kernel::capability::CapabilityEffect)>> = HashMap::new();
    for (index, (spec, _)) in registrar.capabilities.iter().enumerate() {
        let Some(owner) = registrar.capability_owners.get(index).and_then(Option::as_ref) else {
            return Err(AppError::BadRequest(format!("capability `{}` has no plugin owner", spec.id)));
        };
        if registered.entry(owner.clone()).or_default()
            .insert(spec.id.clone(), (spec.permission.clone(), spec.effect)).is_some()
        {
            return Err(AppError::BadRequest(format!("capability `{}` is registered more than once", spec.id)));
        }
    }
    for manifest in catalog.manifests() {
        let actual = registered.remove(&manifest.id).unwrap_or_default();
        for declared in &manifest.capabilities {
            match actual.get(&declared.id) {
                Some((permission, effect)) if permission == &declared.permission && effect == &declared.effect => {}
                _ => return Err(AppError::BadRequest(format!("plugin `{}` capability `{}` does not match its manifest declaration", manifest.id, declared.id))),
            }
        }
        if let Some(unexpected) = actual.keys().find(|id| !manifest.capabilities.iter().any(|cap| &cap.id == *id)) {
            return Err(AppError::BadRequest(format!("plugin `{}` registers undeclared capability `{unexpected}`", manifest.id)));
        }
    }
    Ok(())
}
