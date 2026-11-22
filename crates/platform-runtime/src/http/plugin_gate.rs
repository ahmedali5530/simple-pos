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
}

impl PluginRouteTable {
    pub fn router(
        mounts: Vec<(PluginId, Vec<RouteSpec>, Router)>,
        enabled: Arc<EnabledPlugins>,
        pool: PgPool,
    ) -> Router {
        let table = Self {
            mounts: Arc::new(
                mounts
                    .into_iter()
                    .map(|(id, specs, router)| PluginMount { id, specs, router })
                    .collect(),
            ),
            enabled,
            pool,
        };
        Router::new().fallback_service(table)
    }

    async fn dispatch(self, mut request: Request) -> Response {
        let Some(user) = request.extensions().get::<AuthUser>().cloned() else {
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
