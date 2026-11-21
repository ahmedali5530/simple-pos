use std::collections::HashSet;

use axum::handler::Handler;
use axum::http::Method;
use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::manifest::PluginId;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    #[error("plugin route `{0}` must be a relative path without `..`")]
    InvalidPath(String),
    #[error("plugin `{plugin}` cannot require permission `{permission}` on a route")]
    UndeclaredPermission { plugin: String, permission: String },
    #[error("plugin `{plugin}` does not consume event `{event}`")]
    UndeclaredEvent { plugin: String, event: String },
}

impl From<RegistrationError> for crate::error::AppError {
    fn from(value: RegistrationError) -> Self {
        crate::error::AppError::BadRequest(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct RouteSpec {
    pub method: Method,
    pub path: String,
    pub permission: String,
}

/// The only way a plugin may mount HTTP routes.
///
/// Paths are relative (`items`, `items/{id}`). Absolute paths and `..` are rejected.
/// The kernel prefixes `/api/v1/{plugin_id}/`.
pub struct PluginRoutes {
    plugin_id: PluginId,
    declared: HashSet<String>,
    router: Router,
    specs: Vec<RouteSpec>,
}

impl PluginRoutes {
    pub fn new(plugin_id: PluginId, declared: HashSet<String>) -> Self {
        Self {
            plugin_id,
            declared,
            router: Router::new(),
            specs: Vec::new(),
        }
    }

    pub fn get<H, T>(&mut self, path: &str, handler: H, permission: &str) -> Result<(), RegistrationError>
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.add(Method::GET, path, get(handler), permission)
    }

    pub fn post<H, T>(&mut self, path: &str, handler: H, permission: &str) -> Result<(), RegistrationError>
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.add(Method::POST, path, post(handler), permission)
    }

    pub fn put<H, T>(&mut self, path: &str, handler: H, permission: &str) -> Result<(), RegistrationError>
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.add(Method::PUT, path, put(handler), permission)
    }

    pub fn delete<H, T>(&mut self, path: &str, handler: H, permission: &str) -> Result<(), RegistrationError>
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.add(Method::DELETE, path, delete(handler), permission)
    }

    fn add(
        &mut self,
        method: Method,
        path: &str,
        route: axum::routing::MethodRouter,
        permission: &str,
    ) -> Result<(), RegistrationError> {
        if !self.declared.contains(permission) {
            return Err(RegistrationError::UndeclaredPermission {
                plugin: self.plugin_id.to_string(),
                permission: permission.to_string(),
            });
        }
        let full = self.full_path(path)?;
        self.router = std::mem::take(&mut self.router).route(&full, route);
        self.specs.push(RouteSpec {
            method,
            path: full,
            permission: permission.to_string(),
        });
        Ok(())
    }

    fn full_path(&self, path: &str) -> Result<String, RegistrationError> {
        if path.starts_with('/') || path.contains("..") || path.contains('\\') {
            return Err(RegistrationError::InvalidPath(path.to_string()));
        }
        let relative = path.trim_matches('/');
        if relative.is_empty() {
            Ok(format!("/api/v1/{}", self.plugin_id))
        } else {
            Ok(format!("/api/v1/{}/{relative}", self.plugin_id))
        }
    }

    pub fn finish(self) -> (Router, Vec<RouteSpec>) {
        (self.router, self.specs)
    }
}

/// Match a registered template (`/api/v1/probe/items/{id}`) against a request path.
pub fn path_matches(template: &str, path: &str) -> bool {
    let template: Vec<_> = template.split('/').collect();
    let path: Vec<_> = path.split('/').collect();
    if template.len() != path.len() {
        return false;
    }
    template.iter().zip(path).all(|(expected, actual)| {
        (expected.starts_with('{') && expected.ends_with('}') && expected.len() > 2) || *expected == actual
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    async fn ok() -> &'static str {
        "ok"
    }

    fn routes() -> PluginRoutes {
        let mut declared = HashSet::new();
        declared.insert("catalog.read".into());
        PluginRoutes::new(PluginId::must("catalog"), declared)
    }

    #[test]
    fn prefixes_relative_paths() {
        let mut routes = routes();
        routes.get("items", ok, "catalog.read").unwrap();
        let (_router, specs) = routes.finish();
        assert_eq!(specs[0].path, "/api/v1/catalog/items");
        assert_eq!(specs[0].permission, "catalog.read");
    }

    #[test]
    fn rejects_absolute_and_parent_paths() {
        let mut routes = routes();
        assert!(matches!(
            routes.get("/api/v1/catalog/items", ok, "catalog.read"),
            Err(RegistrationError::InvalidPath(_))
        ));
        assert!(matches!(
            routes.get("../other", ok, "catalog.read"),
            Err(RegistrationError::InvalidPath(_))
        ));
    }

    #[test]
    fn rejects_undeclared_permission() {
        let mut routes = routes();
        assert!(matches!(
            routes.get("items", ok, "inventory.write"),
            Err(RegistrationError::UndeclaredPermission { .. })
        ));
    }

    #[test]
    fn path_template_matches_params_only() {
        assert!(path_matches("/api/v1/catalog/items/{id}", "/api/v1/catalog/items/4"));
        assert!(!path_matches("/api/v1/catalog/items/{id}", "/api/v1/other/items/4"));
    }
}
