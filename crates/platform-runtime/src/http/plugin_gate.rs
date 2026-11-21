use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use platform_kernel::error::AppError;

use crate::http::errors::ApiError;

/// Reject requests for routes belonging to a disabled plugin.
pub async fn require_plugin(
    enabled: bool,
    plugin_id: &'static str,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !enabled {
        return Err(ApiError(AppError::NotFound(format!(
            "Plugin `{plugin_id}` is not enabled."
        ))));
    }
    Ok(next.run(request).await)
}
