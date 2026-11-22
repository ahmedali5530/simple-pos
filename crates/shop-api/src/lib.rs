use std::net::SocketAddr;

use axum::{
    extract::{Path, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{AppendHeaders, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use platform_kernel::error::{AppError, ErrorBody};
use plugin_pos::admin::{login, logout, needs_first_user, open_session, remove_user, save_user};
use plugin_pos::billing::current_shop;
use plugin_pos::models::{
    BarcodeRule, LoginForm, LoginSession, PosSnapshot, PosUser, SavedRecord, ShopConfig, UserForm,
};
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

#[cfg(feature = "bundle")]
mod embedded;

mod compose;

#[cfg(feature = "web")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../dist/"]
struct TillAssets;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    shop: ShopConfig,
    /// Platform gateway and manifests (optional until boot completes).
    platform: Option<std::sync::Arc<PlatformState>>,
}

#[derive(Clone)]
struct PlatformState {
    gateway: std::sync::Arc<tokio::sync::Mutex<platform_runtime::CapabilityGateway>>,
    catalog: std::sync::Arc<platform_kernel::registry::PluginCatalog>,
    lifecycle: platform_runtime::PluginLifecycle,
    enabled: std::sync::Arc<platform_runtime::EnabledPlugins>,
    plugin_router: axum::Router,
    _stop: Option<tokio::sync::watch::Sender<bool>>,
}

#[derive(serde::Deserialize)]
struct FileConfig {
    shop: ShopConfig,
    #[serde(default)]
    barcode: BarcodeFile,
}

#[derive(serde::Deserialize, Default)]
struct BarcodeFile {
    #[serde(default)]
    rules: Vec<BarcodeRule>,
}

fn load_shop_config() -> ShopConfig {
    let text = ["config.toml", "../../config.toml"]
        .into_iter()
        .find(|candidate| std::path::Path::new(candidate).is_file())
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| include_str!("../../../config.toml").to_string());
    let file: FileConfig =
        toml::from_str(&text).unwrap_or_else(|err| panic!("could not parse config.toml: {err}"));
    let mut shop = file.shop.checked().unwrap_or_else(|err| panic!("{err}"));
    shop.barcode_rules = if file.barcode.rules.is_empty() {
        BarcodeRule::defaults()
    } else {
        file.barcode
            .rules
            .into_iter()
            .map(|rule| rule.checked().unwrap_or_else(|err| panic!("{err}")))
            .collect()
    };
    shop
}

pub struct Till {
    url: String,
    shutdown: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
    pool: PgPool,
    #[cfg(feature = "bundle")]
    db: Option<embedded::EmbeddedDb>,
}

impl Till {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn stop(self) {
        let _ = self.shutdown.send(());
        let _ = self.server.await;
        self.pool.close().await;
        #[cfg(feature = "bundle")]
        if let Some(db) = self.db {
            db.stop().await;
        }
    }
}

pub async fn run() {
    let till = open_with(std::env::args().any(|arg| arg == "--embedded"))
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    wait_for_stop().await;
    till.stop().await;
}

pub async fn open() -> Result<Till, String> {
    open_with(true).await
}

/// Open the shop database and apply the legacy schema migrations.
pub async fn connect(database_url: &str) -> Result<PgPool, AppError> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|err| AppError::BadRequest(err.to_string()))?;
    Ok(pool)
}

/// Host and port from a database URL, without the user or password.
fn database_target(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme.split('@').nth(1).unwrap_or(after_scheme);
    host.split(['/', '?']).next().unwrap_or(host).to_string()
}

/// Windows builds pass the small program that starts Postgres with no terminal.
pub fn set_pg_launcher(bytes: &'static [u8]) {
    #[cfg(feature = "bundle")]
    embedded::set_pg_launcher(bytes);
    #[cfg(not(feature = "bundle"))]
    let _ = bytes;
}

async fn open_with(embedded: bool) -> Result<Till, String> {
    let _ = dotenvy::from_filename(".env");
    let _ = dotenvy::from_filename("../../.env");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    #[cfg(feature = "bundle")]
    let db = if embedded {
        Some(embedded::EmbeddedDb::start().await?)
    } else {
        None
    };
    #[cfg(feature = "bundle")]
    let database_url = match &db {
        Some(db) => db.url.clone(),
        None => {
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required".to_string())?
        }
    };
    #[cfg(not(feature = "bundle"))]
    let database_url = {
        if embedded {
            return Err(
                "This build has no embedded database. Rebuild with --features bundle".into(),
            );
        }
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required".to_string())?
    };
    let pool = connect(&database_url).await.map_err(|err| {
        format!(
            "Could not open the shop database at {}: {err}",
            database_target(&database_url)
        )
    })?;
    let shop = load_shop_config();
    let platform = boot_platform(&database_url).await;
    let app = serve_till(build_router_with_platform(pool.clone(), shop, platform));

    let addr: SocketAddr = std::env::var("API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".into())
        .parse()
        .map_err(|_| "API_ADDR is not a socket address".to_string())?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| format!("could not listen on {addr}: {err}"))?;
    let bound = listener
        .local_addr()
        .map_err(|err| format!("could not read the listen address: {err}"))?;
    tracing::info!("Till listening on http://{bound}");
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });
    Ok(Till {
        url: format!("http://{bound}"),
        shutdown,
        server,
        pool,
        #[cfg(feature = "bundle")]
        db,
    })
}

async fn wait_for_stop() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn boot_platform(database_url: &str) -> Option<std::sync::Arc<PlatformState>> {
    use platform_runtime::{BootConfig, Platform};
    use std::sync::Arc;

    let plugins = compose::plugins();
    let admin_url = std::env::var("DATABASE_ADMIN_URL").ok();
    match Platform::boot(
        BootConfig {
            database_url: database_url.to_string(),
            database_admin_url: admin_url,
            app_password: std::env::var("PLATFORM_APP_PASSWORD").ok(),
        },
        plugins,
    )
    .await
    {
        Ok(platform) => {
            let platform_runtime::Platform {
                gateway,
                catalog,
                lifecycle,
                enabled,
                plugin_router,
                stop_dispatcher,
                ..
            } = platform;
            Some(Arc::new(PlatformState {
                gateway: Arc::new(tokio::sync::Mutex::new(gateway)),
                catalog,
                lifecycle,
                enabled,
                plugin_router,
                _stop: stop_dispatcher,
            }))
        }
        Err(err) => {
            tracing::warn!(error = %err, "platform boot skipped; legacy API only");
            None
        }
    }
}

/// Build the HTTP router for the till API. Used by the server and by characterization tests.
pub fn build_router(pool: PgPool, shop: ShopConfig) -> Router {
    build_router_inner(pool, shop, None)
}

fn build_router_with_platform(
    pool: PgPool,
    shop: ShopConfig,
    platform: Option<std::sync::Arc<PlatformState>>,
) -> Router {
    build_router_inner(pool, shop, platform)
}

fn build_router_inner(
    pool: PgPool,
    shop: ShopConfig,
    platform: Option<std::sync::Arc<PlatformState>>,
) -> Router {
    let plugin_router = platform
        .as_ref()
        .map(|platform| platform.plugin_router.clone())
        .unwrap_or_default();
    let state = AppState {
        pool: pool.clone(),
        shop: shop.clone(),
        platform,
    };
    Router::new()
        .route("/api/login", post(post_login))
        .route("/api/logout", post(post_logout))
        .route("/api/session", get(get_session))
        .route("/api/v1/identity/login", post(post_identity_login))
        .route("/api/v1/identity/logout", post(post_identity_logout))
        .route("/api/v1/platform/settings/{plugin_id}", get(get_plugin_settings).put(put_plugin_settings))
        .route("/api/v1/platform/shop", put(put_shop_profile))
        .route("/api/bootstrap", get(get_bootstrap))
        .route("/api/users", post(post_user))
        .route("/api/users/{id}", put(put_user).delete(delete_user))
        .route("/api/v1/platform/manifest", get(get_platform_manifest))
        .route("/api/v1/platform/tokens", post(post_platform_token))
        .route("/api/v1/platform/plugins/{id}", put(put_platform_plugin))
        .route("/api/v1/capabilities", get(get_capabilities))
        .route("/api/v1/capabilities/{id}/invoke", post(post_capability_invoke))
        .route("/api/v1/mcp", get(get_mcp_descriptor))
        .merge(plugin_router.with_state(()))
        .layer(from_fn_with_state(state.clone(), require_user))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

#[cfg(feature = "web")]
fn serve_till(api: Router) -> Router {
    use axum_frontend::{embed_router_with, SpaOptions};
    api.merge(embed_router_with::<TillAssets>(
        SpaOptions::new().exclude(["/api"]).immutable(["/assets/"]),
    ))
}

#[cfg(not(feature = "web"))]
fn serve_till(api: Router) -> Router {
    api
}

#[derive(serde::Serialize)]
struct BootstrapStatus {
    needs_user: bool,
    shop_name: String,
}

async fn get_bootstrap(State(state): State<AppState>) -> Result<Json<BootstrapStatus>, ApiError> {
    let shop = current_shop(&state.pool, &state.shop).await?;
    Ok(Json(BootstrapStatus {
        needs_user: needs_first_user(&state.pool).await?,
        shop_name: shop.name,
    }))
}

fn lift_platform(err: platform_kernel::error::AppError) -> AppError {
    match err {
        platform_kernel::error::AppError::BadRequest(message) => AppError::BadRequest(message),
        platform_kernel::error::AppError::NotFound(message) => AppError::NotFound(message),
        platform_kernel::error::AppError::Unauthorized(message) => AppError::Unauthorized(message),
        platform_kernel::error::AppError::Forbidden(message) => AppError::Unauthorized(message),
        platform_kernel::error::AppError::Conflict(message) => AppError::BadRequest(message),
        platform_kernel::error::AppError::Db(error) => AppError::Db(error),
    }
}

async fn post_identity_login(
    State(state): State<AppState>,
    Json(body): Json<platform_identity::LoginForm>,
) -> Result<Json<platform_identity::LoginSession>, ApiError> {
    platform_identity::login(
        &state.pool,
        platform_kernel::context::TenantId::DEFAULT,
        &body,
    )
    .await
    .map(Json)
    .map_err(|err| ApiError(lift_platform(err)))
}

async fn post_identity_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    platform_identity::logout(
        &state.pool,
        platform_kernel::context::TenantId::DEFAULT,
        token,
    )
    .await
    .map_err(|err| ApiError(lift_platform(err)))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_plugin_settings(
    State(state): State<AppState>,
    Path(plugin_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = platform_kernel::context::TenantTx::begin_with(
        &state.pool,
        platform_kernel::context::TenantId::DEFAULT,
        platform_kernel::context::Actor::system(),
        true,
    )
    .await
    .map_err(|err| ApiError(lift_platform(err)))?;
    let value = platform_runtime::settings::load(&mut tx, &plugin_id)
        .await
        .map_err(|err| ApiError(lift_platform(err)))?;
    tx.commit().await.map_err(|err| ApiError(lift_platform(err)))?;
    Ok(Json(value))
}

async fn put_shop_profile(
    State(state): State<AppState>,
    Json(body): Json<plugin_pos::models::ShopProfileForm>,
) -> Result<Json<PosSnapshot>, ApiError> {
    let mut tx = platform_kernel::context::TenantTx::begin_with(
        &state.pool,
        platform_kernel::context::TenantId::DEFAULT,
        platform_kernel::context::Actor::system(),
        false,
    )
    .await
    .map_err(|err| ApiError(lift_platform(err)))?;
    let pos = plugin_pos::admin::save_shop_profile(tx.transaction(), &state.shop, &body)
        .await
        .map_err(ApiError::from)?;
    tx.commit().await.map_err(|err| ApiError(lift_platform(err)))?;
    Ok(Json(pos))
}

async fn put_plugin_settings(
    State(state): State<AppState>,
    Path(plugin_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut tx = platform_kernel::context::TenantTx::begin_with(
        &state.pool,
        platform_kernel::context::TenantId::DEFAULT,
        platform_kernel::context::Actor::system(),
        false,
    )
    .await
    .map_err(|err| ApiError(lift_platform(err)))?;
    platform_runtime::settings::save(&mut tx, &plugin_id, &body)
        .await
        .map_err(|err| ApiError(lift_platform(err)))?;
    tx.commit().await.map_err(|err| ApiError(lift_platform(err)))?;
    Ok(Json(body))
}

async fn post_login(
    State(state): State<AppState>,
    Json(body): Json<LoginForm>,
) -> Result<
    (
        AppendHeaders<[(header::HeaderName, HeaderValue); 1]>,
        Json<LoginSession>,
    ),
    ApiError,
> {
    let session = login(&state.pool, &body).await?;
    Ok((
        AppendHeaders([(header::SET_COOKIE, session_cookie(&session.token, false))]),
        Json(session),
    ))
}

async fn post_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<
    (
        AppendHeaders<[(header::HeaderName, HeaderValue); 1]>,
        StatusCode,
    ),
    ApiError,
> {
    let token = session_token(&headers);
    if !token.is_empty() {
        logout(&state.pool, &token).await?;
    }
    Ok((
        AppendHeaders([(header::SET_COOKIE, session_cookie("", true))]),
        StatusCode::NO_CONTENT,
    ))
}

async fn get_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PosUser>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    Ok(Json(PosUser {
        id: cashier.id,
        name: cashier.name,
        role: cashier.role,
        active: true,
    }))
}

fn session_token(headers: &HeaderMap) -> String {
    if let Some(token) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        return token.to_string();
    }
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookie| {
            cookie.split(';').find_map(|part| {
                let part = part.trim();
                part.strip_prefix("till_session=")
                    .map(str::trim)
                    .filter(|token| !token.is_empty())
                    .map(str::to_string)
            })
        })
        .unwrap_or_default()
}

fn session_cookie(token: &str, clear: bool) -> HeaderValue {
    let value = if clear {
        "till_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0".to_string()
    } else {
        format!("till_session={token}; Path=/; HttpOnly; SameSite=Lax")
    };
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static(""))
}

async fn require_user(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();
    let open = (method == Method::POST && (path == "/api/login" || path == "/api/logout"))
        || (method == Method::GET && path == "/api/bootstrap");
    if open {
        return next.run(request).await;
    }
    if method == Method::POST && path == "/api/users" {
        match needs_first_user(&state.pool).await {
            Ok(true) => return next.run(request).await,
            Ok(false) => {}
            Err(error) => return ApiError(error).into_response(),
        }
    }
    match open_session(&state.pool, &session_token(request.headers())).await {
        Ok(cashier) => {
            request.extensions_mut().insert(platform_runtime::http::AuthUser {
                tenant_id: platform_kernel::context::TenantId::DEFAULT,
                user_id: cashier.id,
                name: cashier.name,
                role: cashier.role,
                permissions: vec!["*".into()],
            });
            next.run(request).await
        }
        Err(error) => ApiError(error).into_response(),
    }
}

async fn post_user(
    State(state): State<AppState>,
    Json(body): Json<UserForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(
        save_user(&state.pool, &state.shop, None, &body).await?,
    ))
}

async fn put_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<UserForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(
        save_user(&state.pool, &state.shop, Some(id), &body).await?,
    ))
}

async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<PosSnapshot>, ApiError> {
    Ok(Json(remove_user(&state.pool, &state.shop, id).await?))
}

#[derive(serde::Deserialize)]
struct TokenForm {
    name: String,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    allow_destructive: bool,
}

async fn post_platform_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TokenForm>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    let actor = platform_kernel::context::Actor {
        user_id: Some(cashier.id),
        name: cashier.name,
        role: cashier.role,
        permissions: vec!["*".into()],
    };
    let (token, id) = platform_identity::create_api_token(
        &state.pool,
        platform_kernel::context::TenantId::DEFAULT,
        actor,
        &body.name,
        &body.permissions,
        body.allow_destructive,
    )
    .await
    .map_err(|e| {
        ApiError(match e {
            platform_kernel::error::AppError::BadRequest(m) => AppError::BadRequest(m),
            platform_kernel::error::AppError::NotFound(m) => AppError::NotFound(m),
            platform_kernel::error::AppError::Unauthorized(m) => AppError::Unauthorized(m),
            platform_kernel::error::AppError::Forbidden(m) => AppError::BadRequest(m),
            platform_kernel::error::AppError::Conflict(m) => AppError::BadRequest(m),
            platform_kernel::error::AppError::Db(err) => AppError::Db(err),
        })
    })?;
    Ok(Json(serde_json::json!({ "id": id, "token": token })))
}

#[derive(serde::Deserialize)]
struct PluginToggle {
    enabled: bool,
}

async fn put_platform_plugin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<PluginToggle>,
) -> Result<StatusCode, Response> {
    let Some(platform) = &state.platform else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Platform is not available."})),
        )
            .into_response());
    };
    let cashier = open_session(&state.pool, &session_token(&headers))
        .await
        .map_err(|err| ApiError(err).into_response())?;
    let actor = platform_kernel::context::Actor {
        user_id: Some(cashier.id),
        name: cashier.name,
        role: cashier.role,
        permissions: vec!["*".into()],
    };
    platform_runtime::admin::set_plugin_enabled(
        &platform.lifecycle,
        platform_kernel::context::TenantId::DEFAULT,
        actor,
        &id,
        body.enabled,
    )
    .await
    .map_err(|err| {
        let status = StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(serde_json::json!({"error": err.to_string()}))).into_response()
    })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_mcp_descriptor(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = open_session(&state.pool, &session_token(&headers)).await?;
    let Some(platform) = &state.platform else {
        return Ok(Json(serde_json::json!({ "tools": [] })));
    };
    let gateway = platform.gateway.lock().await;
    let tools: Vec<_> = gateway
        .registry
        .list()
        .into_iter()
        .map(|cap| {
            serde_json::json!({
                "name": cap.id,
                "description": cap.description,
                "inputSchema": cap.input_schema,
                "annotations": {
                    "readOnlyHint": matches!(cap.effect, platform_kernel::capability::CapabilityEffect::Read),
                    "destructiveHint": matches!(cap.effect, platform_kernel::capability::CapabilityEffect::Destructive),
                }
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "name": "chaska-platform",
        "version": "0.1.0",
        "tools": tools
    })))
}

async fn get_platform_manifest(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(platform) = &state.platform else {
        return Ok(Json(serde_json::json!({
            "tenant_id": "00000000-0000-4000-8000-000000000001",
            "plugins": [],
            "permissions": ["*"]
        })));
    };
    let _ = open_session(&state.pool, &session_token(&headers)).await?;
    let tenant_id = platform_kernel::context::TenantId::DEFAULT;
    let enabled = platform.enabled.ids(tenant_id).await.map_err(platform_to_api)?;
    let manifests = platform.catalog.manifests();
    let active: Vec<_> = manifests
        .iter()
        .filter(|manifest| enabled.contains(&manifest.id))
        .cloned()
        .collect::<Vec<_>>();
    let nav: Vec<_> = active.iter().flat_map(|manifest| manifest.nav.clone()).collect();
    let settings_pages: Vec<_> = active
        .iter()
        .flat_map(|manifest| manifest.settings_pages.clone())
        .collect();
    let report_pages: Vec<_> = active
        .iter()
        .flat_map(|manifest| manifest.report_pages.clone())
        .collect();
    let available: Vec<_> = manifests
        .iter()
        .map(|manifest| {
            serde_json::json!({
                "id": manifest.id,
                "name": manifest.name,
                "version": manifest.version.to_string(),
                "description": manifest.description,
                "consents": manifest.consents,
                "enabled": enabled.contains(&manifest.id),
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "tenant_id": tenant_id.to_string(),
        "plugins": active,
        "available": available,
        "permissions": ["*"],
        "nav": nav,
        "settings_pages": settings_pages,
        "report_pages": report_pages,
    })))
}

fn platform_to_api(err: platform_kernel::error::AppError) -> ApiError {
    let message = err.to_string();
    ApiError(match err {
        platform_kernel::error::AppError::BadRequest(_) => AppError::BadRequest(message),
        platform_kernel::error::AppError::NotFound(_) => AppError::NotFound(message),
        platform_kernel::error::AppError::Unauthorized(_) => AppError::Unauthorized(message),
        platform_kernel::error::AppError::Forbidden(_) => AppError::BadRequest(message),
        platform_kernel::error::AppError::Conflict(_) => AppError::BadRequest(message),
        platform_kernel::error::AppError::Db(error) => AppError::Db(error),
    })
}

async fn get_capabilities(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = open_session(&state.pool, &session_token(&headers)).await?;
    let Some(platform) = &state.platform else {
        return Ok(Json(serde_json::json!({ "capabilities": [] })));
    };
    let gateway = platform.gateway.lock().await;
    let caps: Vec<_> = gateway
        .registry
        .list()
        .into_iter()
        .filter(|c| matches!(c.effect, platform_kernel::capability::CapabilityEffect::Read))
        .collect();
    Ok(Json(serde_json::json!({ "capabilities": caps })))
}

#[derive(serde::Deserialize)]
struct InvokeBody {
    #[serde(default)]
    input: serde_json::Value,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    confirm: Option<bool>,
}

async fn post_capability_invoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<InvokeBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    let Some(platform) = &state.platform else {
        return Err(AppError::NotFound("Platform is not available.".into()).into());
    };
    let user = platform_runtime::http::auth::AuthUser {
        tenant_id: platform_kernel::context::TenantId::DEFAULT,
        user_id: cashier.id,
        name: cashier.name,
        role: cashier.role,
        permissions: vec!["*".into()],
    };
    let mut input = body.input;
    if let Some(confirm) = body.confirm {
        if let Some(obj) = input.as_object_mut() {
            obj.insert("confirm".into(), serde_json::json!(confirm));
        }
    }
    let gateway = platform.gateway.lock().await;
    let output = gateway
        .invoke(
            &user,
            &id,
            input,
            body.idempotency_key.as_deref(),
            true,
        )
        .await
        .map_err(|e| {
            ApiError(match e {
                platform_kernel::error::AppError::BadRequest(m) => AppError::BadRequest(m),
                platform_kernel::error::AppError::NotFound(m) => AppError::NotFound(m),
                platform_kernel::error::AppError::Unauthorized(m) => AppError::Unauthorized(m),
                platform_kernel::error::AppError::Forbidden(m) => AppError::BadRequest(m),
                platform_kernel::error::AppError::Conflict(m) => AppError::BadRequest(m),
                platform_kernel::error::AppError::Db(err) => AppError::Db(err),
            })
        })?;
    Ok(Json(output))
}

struct ApiError(AppError);

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        ApiError(value)
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self.0 {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) | AppError::Forbidden(_) => StatusCode::UNAUTHORIZED,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(ErrorBody {
            error: self.0.to_string(),
        });
        (status, body).into_response()
    }
}
