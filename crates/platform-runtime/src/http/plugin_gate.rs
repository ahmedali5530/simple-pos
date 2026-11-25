use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Router;
use platform_kernel::error::AppError;
use platform_kernel::manifest::PluginId;
use platform_kernel::routes::{path_matches, RouteSpec, TenantDbSource};
use sqlx::PgPool;
use tower::Service;
use tower::ServiceExt;

use crate::http::auth::AuthUser;
use crate::http::errors::ApiError;
use crate::plugins::EnabledPlugins;
use platform_kernel::contract::ContractRegistry;

struct PluginMount {
    id: PluginId,
    specs: Vec<RouteSpec>,
    router: Router,
}

/// Routes a plugin registered at boot.
///
/// Enablement is per tenant, so the process keeps the handlers, but a tenant's
/// route table contains only the plugins that tenant has enabled. A disabled
/// plugin is not matched. The response is the same 404 as a path that was never
/// registered.
#[derive(Clone)]
pub struct PluginRouteTable {
    mounts: Arc<Vec<PluginMount>>,
    enabled: Arc<EnabledPlugins>,
    pool: PgPool,
    fallback: Option<Router>,
    contracts: ContractRegistry,
}

impl PluginRouteTable {
    pub fn build_router(
        mounts: Vec<(PluginId, Vec<RouteSpec>, Router)>,
        enabled: Arc<EnabledPlugins>,
        pool: PgPool,
    ) -> Router {
        Self::new(mounts, enabled, pool, ContractRegistry::default()).router()
    }

    pub fn new(
        mounts: Vec<(PluginId, Vec<RouteSpec>, Router)>,
        enabled: Arc<EnabledPlugins>,
        pool: PgPool,
        contracts: ContractRegistry,
    ) -> Self {
        Self {
            mounts: Arc::new(
                mounts
                    .into_iter()
                    .map(|(id, specs, router)| PluginMount { id, specs, router })
                    .collect(),
            ),
            enabled,
            pool,
            fallback: None,
            contracts,
        }
    }

    pub fn router_with_fallback(&self, fallback: Router) -> Router {
        let mut table = self.clone();
        table.fallback = Some(fallback);
        Router::new().fallback_service(table)
    }

    pub fn router(&self) -> Router {
        Router::new().fallback_service(self.clone())
    }

    async fn dispatch(self, mut request: Request) -> Response {
        let Some(user) = request.extensions().get::<AuthUser>().cloned() else {
            if let Some(fallback) = &self.fallback {
                return match fallback.clone().oneshot(request).await {
                    Ok(response) => response,
                    Err(err) => match err {},
                };
            }
            return ApiError(AppError::Unauthorized("Sign in before continuing.".into())).into_response();
        };
        let enabled = match self.enabled.ids(user.tenant_id).await {
            Ok(ids) => ids,
            Err(err) => return ApiError(err).into_response(),
        };
        let method = request.method().clone();
        let path = request.uri().path().to_string();
        let Some(mount) = self.mounts.iter().find(|mount| {
            enabled.contains(&mount.id)
                && mount
                    .specs
                    .iter()
                    .any(|spec| spec.method == method && path_matches(&spec.path, &path))
        }) else {
            if let Some(fallback) = &self.fallback {
                return match fallback.clone().oneshot(request).await {
                    Ok(response) => response,
                    Err(err) => match err {},
                };
            }
            return StatusCode::NOT_FOUND.into_response();
        };
        let spec = mount
            .specs
            .iter()
            .find(|spec| spec.method == method && path_matches(&spec.path, &path))
            .expect("matched mount has this route");
        if !user.actor().has_permission(&spec.permission) {
            return ApiError(AppError::Forbidden("You do not have permission for that route.".into())).into_response();
        }
        request.extensions_mut().insert(TenantDbSource::new(
            self.pool.clone(),
            user.tenant_id,
            user.actor(),
            self.contracts.clone(),
            enabled,
        ));
        match mount.router.clone().oneshot(request).await {
            Ok(response) => response,
            Err(err) => match err {},
        }
    }
}

impl Service<Request> for PluginRouteTable {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let table = self.clone();
        Box::pin(async move { Ok(table.dispatch(request).await) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use platform_kernel::registry::PluginCatalog;
    use tower::ServiceExt;

    #[tokio::test]
    async fn plugin_fallback_can_be_followed_by_a_shell_fallback() {
        let pool = PgPool::connect_lazy("postgres://localhost/chaska_test").unwrap();
        let catalog = Arc::new(PluginCatalog::build(Vec::new()).unwrap());
        let enabled = Arc::new(EnabledPlugins::new(pool.clone(), catalog));
        let table = PluginRouteTable::new(Vec::new(), enabled, pool, ContractRegistry::default());
        let shell = Router::new().route("/", get(|| async { "shell" }));
        let app = Router::new().route("/api/ping", get(|| async { "api" }))
            .merge(table.router_with_fallback(shell));

        let response = app.oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
