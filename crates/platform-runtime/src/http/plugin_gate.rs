use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Router;
use platform_kernel::error::AppError;
use platform_kernel::manifest::PluginId;
use platform_kernel::routes::{path_matches, RouteSpec};

use crate::http::auth::AuthUser;
use crate::http::errors::ApiError;
use crate::plugins::EnabledPlugins;

/// Wrap one plugin's router so a disabled plugin returns 404 and a missing permission returns 403.
pub fn wrap(
    plugin_id: PluginId,
    specs: Vec<RouteSpec>,
    enabled: Arc<EnabledPlugins>,
    router: Router,
) -> Router {
    let specs = Arc::new(specs);
    router.layer(axum::middleware::from_fn(move |request: Request, next: Next| {
        let plugin_id = plugin_id.clone();
        let enabled = enabled.clone();
        let specs = specs.clone();
        async move { gate(plugin_id, specs, enabled, request, next).await }
    }))
}

async fn gate(
    plugin_id: PluginId,
    specs: Arc<Vec<RouteSpec>>,
    enabled: Arc<EnabledPlugins>,
    request: Request,
    next: Next,
) -> Response {
    let Some(user) = request.extensions().get::<AuthUser>().cloned() else {
        return ApiError(AppError::Unauthorized("Sign in before continuing.".into())).into_response();
    };
    let tenant_id = user.tenant_id;
    match enabled.is_enabled(tenant_id, &plugin_id).await {
        Ok(true) => {}
        Ok(false) => {
            return ApiError(AppError::NotFound(format!("Plugin `{plugin_id}` is not enabled."))).into_response();
        }
        Err(err) => return ApiError(err).into_response(),
    }
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let Some(spec) = specs
        .iter()
        .find(|spec| spec.method == method && path_matches(&spec.path, &path))
    else {
        return ApiError(AppError::Forbidden("You do not have permission for that route.".into())).into_response();
    };
    if !user.actor().has_permission(&spec.permission) {
        return ApiError(AppError::Forbidden("You do not have permission for that route.".into())).into_response();
    }
    next.run(request).await
}
