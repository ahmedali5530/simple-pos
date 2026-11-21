use std::net::SocketAddr;
use std::path::{Path as FilePath, PathBuf};

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{AppendHeaders, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use shop_core::{
    adjust_stock, cash_desk, clear_item_image, export_categories, export_items, export_movements,
    import_categories, import_items, import_movements, item_image_ext, load_pos, load_reports,
    login, logout, needs_first_user, open_session, open_shift, post_bill, print_cash_shift,
    print_product_mix, print_saved_bill, recent_movements, remove_category, remove_section,
    remove_user, reorder_categories, reorder_sections, save_category, save_item, save_section,
    save_settings, save_shift, save_user, set_item_image, set_item_prompt, set_pos_focus,
    set_pos_sections, stock_summary, AppError, CashDesk, CategoryForm, CategoryImport, ErrorBody,
    ItemForm, ItemImport, ItemPromptRequest, LoginForm, LoginSession, MovementImport, OpenShift,
    OrderForm, PgPool, PosFocus, PosSnapshot, PosUser, PostBillRequest, PostedBill, PrintOutcome,
    PrintRequest, Reports, SavedItem, SavedRecord, SectionForm, SetFocusRequest,
    SetSectionsRequest, SettingsForm, ShiftForm, ShopConfig, StockBalance, StockMove,
    StockMovement, StockPage, UserForm,
};
use tower_http::cors::{Any, CorsLayer};

#[cfg(feature = "bundle")]
mod embedded;

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
    manifests: Vec<platform_kernel::manifest::PluginManifest>,
    gateway: std::sync::Arc<tokio::sync::Mutex<platform_runtime::CapabilityGateway>>,
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
    rules: Vec<shop_core::BarcodeRule>,
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
        shop_core::BarcodeRule::defaults()
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
    let pool = shop_core::connect(&database_url).await.map_err(|err| {
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
    use platform_kernel::plugin::Plugin;
    use platform_runtime::{BootConfig, Platform};
    use std::sync::Arc;

    let plugins: Vec<Arc<dyn Plugin>> = vec![
        Arc::new(plugin_catalog::CatalogPlugin),
        Arc::new(plugin_inventory::InventoryPlugin),
        Arc::new(plugin_pos::PosPlugin),
        Arc::new(plugin_cash::CashPlugin),
        Arc::new(plugin_printing::PrintingPlugin),
        Arc::new(plugin_reports::ReportsPlugin),
    ];
    let admin_url = std::env::var("DATABASE_ADMIN_URL").ok();
    match Platform::boot(
        BootConfig {
            database_url: database_url.to_string(),
            database_admin_url: admin_url,
            app_password: std::env::var("PLATFORM_APP_PASSWORD").ok(),
        },
        plugins,
        true,
    )
    .await
    {
        Ok(mut platform) => {
            let manifests = platform.registry.manifests();
            let gateway = Arc::new(tokio::sync::Mutex::new(std::mem::replace(
                &mut platform.gateway,
                platform_runtime::CapabilityGateway::new(
                    platform.pools.runtime.clone(),
                    platform_kernel::capability::CapabilityRegistry::new(),
                ),
            )));
            // Keep dispatcher alive.
            std::mem::forget(platform);
            Some(Arc::new(PlatformState { manifests, gateway }))
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
    let state = AppState {
        pool: pool.clone(),
        shop: shop.clone(),
        platform,
    };
    Router::new()
        .route("/api/pos", get(get_pos))
        .route("/api/pos/focus", put(put_focus))
        .route("/api/pos/sections", put(put_sections))
        .route("/api/bills", axum::routing::post(post_bill_route))
        .route("/api/print", post(post_print))
        .route("/api/print/mix", post(post_print_mix))
        .route("/api/print/cash", post(post_print_cash))
        .route("/api/cash", get(get_cash))
        .route("/api/cash/open", post(post_cash_open))
        .route("/api/cash/draft", post(post_cash_draft))
        .route("/api/cash/close", post(post_cash_close))
        .route("/api/login", post(post_login))
        .route("/api/logout", post(post_logout))
        .route("/api/session", get(get_session))
        .route("/api/bootstrap", get(get_bootstrap))
        .route("/api/sections", post(post_section))
        .route("/api/sections/order", put(put_section_order))
        .route(
            "/api/sections/{id}",
            put(put_section).delete(delete_section),
        )
        .route("/api/categories", post(post_category))
        .route("/api/categories/order", put(put_category_order))
        .route("/api/categories/export", get(get_categories_export))
        .route(
            "/api/categories/import",
            post(post_categories_import).layer(DefaultBodyLimit::max(5 * 1024 * 1024)),
        )
        .route(
            "/api/categories/{id}",
            put(put_category).delete(delete_category),
        )
        .route("/api/users", post(post_user))
        .route("/api/users/{id}", put(put_user).delete(delete_user))
        .route("/api/settings", put(put_settings))
        .route("/api/stock", post(post_stock))
        .route("/api/stock/movements", get(get_movements))
        .route("/api/stock/summary", get(get_stock_summary))
        .route("/api/stock/export", get(get_stock_export))
        .route(
            "/api/stock/import",
            post(post_stock_import).layer(DefaultBodyLimit::max(5 * 1024 * 1024)),
        )
        .route("/api/reports", get(get_reports))
        .route("/api/items", post(post_item))
        .route("/api/items/export", get(get_items_export))
        .route(
            "/api/items/import",
            post(post_items_import).layer(DefaultBodyLimit::max(5 * 1024 * 1024)),
        )
        .route("/api/items/{id}", put(put_item))
        .route("/api/items/{id}/prompt", put(put_item_prompt))
        .route(
            "/api/items/{id}/image",
            get(get_item_image)
                .post(post_item_image)
                .delete(delete_item_image)
                .layer(DefaultBodyLimit::max(3 * 1024 * 1024)),
        )
        // Platform v1 API
        .route("/api/v1/platform/manifest", get(get_platform_manifest))
        .route("/api/v1/platform/tokens", post(post_platform_token))
        .route("/api/v1/platform/plugins/{id}", put(put_platform_plugin))
        .route("/api/v1/capabilities", get(get_capabilities))
        .route("/api/v1/capabilities/{id}/invoke", post(post_capability_invoke))
        .route("/api/v1/mcp", get(get_mcp_descriptor))
        // Legacy aliases under /api/v1/*
        .route("/api/v1/pos", get(get_pos))
        .route("/api/v1/catalog/items", post(post_item))
        .route("/api/v1/inventory/summary", get(get_stock_summary))
        .route("/api/v1/cash", get(get_cash))
        .route("/api/v1/reports", get(get_reports))
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

async fn get_pos(State(state): State<AppState>) -> Result<Json<shop_core::PosSnapshot>, ApiError> {
    Ok(Json(load_pos(&state.pool, &state.shop).await?))
}

async fn put_focus(
    State(state): State<AppState>,
    Json(body): Json<SetFocusRequest>,
) -> Result<Json<PosFocus>, ApiError> {
    let last_section_id = set_pos_focus(&state.pool, body.section_id).await?;
    Ok(Json(PosFocus { last_section_id }))
}

async fn put_sections(
    State(state): State<AppState>,
    Json(body): Json<SetSectionsRequest>,
) -> Result<Json<shop_core::PosSnapshot>, ApiError> {
    Ok(Json(
        set_pos_sections(&state.pool, &state.shop, &body.sections).await?,
    ))
}

#[derive(serde::Serialize)]
struct BootstrapStatus {
    needs_user: bool,
    shop_name: String,
}

async fn get_bootstrap(State(state): State<AppState>) -> Result<Json<BootstrapStatus>, ApiError> {
    let shop = shop_core::current_shop(&state.pool, &state.shop).await?;
    Ok(Json(BootstrapStatus {
        needs_user: needs_first_user(&state.pool).await?,
        shop_name: shop.name,
    }))
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

async fn require_user(State(state): State<AppState>, request: Request, next: Next) -> Response {
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
        Ok(_) => next.run(request).await,
        Err(error) => ApiError(error).into_response(),
    }
}

async fn post_section(
    State(state): State<AppState>,
    Json(body): Json<SectionForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(
        save_section(&state.pool, &state.shop, None, &body).await?,
    ))
}

async fn put_section(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<SectionForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(
        save_section(&state.pool, &state.shop, Some(id), &body).await?,
    ))
}

async fn delete_section(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<PosSnapshot>, ApiError> {
    Ok(Json(remove_section(&state.pool, &state.shop, id).await?))
}

async fn put_section_order(
    State(state): State<AppState>,
    Json(body): Json<OrderForm>,
) -> Result<Json<PosSnapshot>, ApiError> {
    Ok(Json(
        reorder_sections(&state.pool, &state.shop, &body.ids).await?,
    ))
}

async fn post_category(
    State(state): State<AppState>,
    Json(body): Json<CategoryForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(
        save_category(&state.pool, &state.shop, None, &body).await?,
    ))
}

async fn put_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<CategoryForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(
        save_category(&state.pool, &state.shop, Some(id), &body).await?,
    ))
}

async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<PosSnapshot>, ApiError> {
    Ok(Json(remove_category(&state.pool, &state.shop, id).await?))
}

async fn put_category_order(
    State(state): State<AppState>,
    Json(body): Json<OrderForm>,
) -> Result<Json<PosSnapshot>, ApiError> {
    Ok(Json(
        reorder_categories(&state.pool, &state.shop, &body.ids).await?,
    ))
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

async fn put_settings(
    State(state): State<AppState>,
    Json(body): Json<SettingsForm>,
) -> Result<Json<PosSnapshot>, ApiError> {
    Ok(Json(save_settings(&state.pool, &state.shop, &body).await?))
}

#[derive(serde::Deserialize)]
struct MovementQuery {
    item_id: Option<i32>,
}

#[derive(serde::Deserialize)]
struct ReportQuery {
    from: Option<String>,
    to: Option<String>,
}

async fn post_stock(
    State(state): State<AppState>,
    Json(body): Json<StockMove>,
) -> Result<Json<StockPage>, ApiError> {
    Ok(Json(adjust_stock(&state.pool, &state.shop, &body).await?))
}

async fn get_stock_summary(
    State(state): State<AppState>,
) -> Result<Json<Vec<StockBalance>>, ApiError> {
    Ok(Json(stock_summary(&state.pool).await?))
}

async fn get_movements(
    State(state): State<AppState>,
    Query(query): Query<MovementQuery>,
) -> Result<Json<Vec<StockMovement>>, ApiError> {
    Ok(Json(recent_movements(&state.pool, query.item_id).await?))
}

async fn get_reports(
    State(state): State<AppState>,
    Query(query): Query<ReportQuery>,
) -> Result<Json<Reports>, ApiError> {
    Ok(Json(
        load_reports(&state.pool, query.from.as_deref(), query.to.as_deref()).await?,
    ))
}

async fn get_categories_export(State(state): State<AppState>) -> Result<Response, ApiError> {
    let bytes = export_categories(&state.pool).await?;
    Ok(workbook_response("categories.xlsx", bytes))
}

async fn post_categories_import(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<CategoryImport>, ApiError> {
    let bytes = workbook_bytes(multipart).await?;
    Ok(Json(
        import_categories(&state.pool, &state.shop, &bytes).await?,
    ))
}

async fn get_items_export(State(state): State<AppState>) -> Result<Response, ApiError> {
    let bytes = export_items(&state.pool).await?;
    Ok(workbook_response("items.xlsx", bytes))
}

async fn post_items_import(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<ItemImport>, ApiError> {
    let bytes = workbook_bytes(multipart).await?;
    Ok(Json(import_items(&state.pool, &state.shop, &bytes).await?))
}

async fn get_stock_export(State(state): State<AppState>) -> Result<Response, ApiError> {
    let bytes = export_movements(&state.pool).await?;
    Ok(workbook_response("stock-movements.xlsx", bytes))
}

async fn post_stock_import(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<MovementImport>, ApiError> {
    let bytes = workbook_bytes(multipart).await?;
    Ok(Json(
        import_movements(&state.pool, &state.shop, &bytes).await?,
    ))
}

fn workbook_response(filename: &str, bytes: Vec<u8>) -> Response {
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"workbook.xlsx\""));
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                ),
            ),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response()
}

async fn workbook_bytes(mut multipart: Multipart) -> Result<Vec<u8>, AppError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("That workbook could not be read.".into()))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|_| AppError::BadRequest("That workbook could not be read.".into()))?;
            if bytes.is_empty() {
                return Err(AppError::BadRequest("Choose an Excel workbook.".into()));
            }
            return Ok(bytes.to_vec());
        }
    }
    Err(AppError::BadRequest("Choose an Excel workbook.".into()))
}

async fn post_item(
    State(state): State<AppState>,
    Json(body): Json<ItemForm>,
) -> Result<Json<SavedItem>, ApiError> {
    Ok(Json(
        save_item(&state.pool, &state.shop, None, &body).await?,
    ))
}

async fn put_item(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<ItemForm>,
) -> Result<Json<SavedItem>, ApiError> {
    Ok(Json(
        save_item(&state.pool, &state.shop, Some(id), &body).await?,
    ))
}

async fn put_item_prompt(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<ItemPromptRequest>,
) -> Result<Json<shop_core::PosSnapshot>, ApiError> {
    Ok(Json(
        set_item_prompt(&state.pool, &state.shop, id, &body).await?,
    ))
}

fn shop_root() -> PathBuf {
    for candidate in [".", "../.."] {
        let root = FilePath::new(candidate);
        if root.join("config.toml").is_file() {
            return root.to_path_buf();
        }
    }
    PathBuf::from(".")
}

fn image_root() -> PathBuf {
    if let Ok(dir) = std::env::var("IMAGE_DIR") {
        return PathBuf::from(dir);
    }
    shop_root().join("data/item-images")
}

fn known_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "jpg" => Some("jpg"),
        "png" => Some("png"),
        "webp" => Some("webp"),
        "gif" => Some("gif"),
        _ => None,
    }
}

fn image_file(id: i32, ext: &str) -> Option<PathBuf> {
    let ext = known_ext(ext)?;
    Some(image_root().join(format!("{id}.{ext}")))
}

fn sniff_image(bytes: &[u8]) -> Result<&'static str, AppError> {
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(AppError::BadRequest("Use a picture under 2 MB.".into()));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("jpg");
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Ok("png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok("webp");
    }
    Err(AppError::BadRequest(
        "Use a JPEG, PNG, WebP, or GIF.".into(),
    ))
}

fn content_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    }
}

fn write_image(id: i32, ext: &str, bytes: &[u8]) -> Result<(), AppError> {
    let dir = image_root();
    std::fs::create_dir_all(&dir)
        .map_err(|_| AppError::BadRequest("The picture folder could not be created.".into()))?;
    let path = image_file(id, ext)
        .ok_or_else(|| AppError::BadRequest("Use a JPEG, PNG, WebP, or GIF.".into()))?;
    let tmp = dir.join(format!("{id}.{ext}.tmp"));
    std::fs::write(&tmp, bytes)
        .map_err(|_| AppError::BadRequest("That picture could not be stored.".into()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|_| AppError::BadRequest("That picture could not be stored.".into()))?;
    Ok(())
}

fn remove_image_file(id: i32, ext: &str) {
    if let Some(path) = image_file(id, ext) {
        let _ = std::fs::remove_file(path);
    }
}

async fn get_item_image(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<axum::response::Response, ApiError> {
    let Some(ext) = item_image_ext(&state.pool, id).await? else {
        return Err(AppError::NotFound("This item has no picture.".into()).into());
    };
    let Some(path) = image_file(id, &ext) else {
        return Err(AppError::NotFound("This item has no picture.".into()).into());
    };
    let bytes =
        std::fs::read(&path).map_err(|_| AppError::NotFound("This item has no picture.".into()))?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type(&ext)),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        bytes,
    )
        .into_response())
}

async fn post_item_image(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    mut multipart: Multipart,
) -> Result<Json<SavedItem>, ApiError> {
    let previous = item_image_ext(&state.pool, id).await?;
    let mut picture: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("That picture could not be read.".into()))?
    {
        if field.name() == Some("picture") {
            let data = field
                .bytes()
                .await
                .map_err(|_| AppError::BadRequest("That picture could not be read.".into()))?;
            picture = Some(data.to_vec());
            break;
        }
    }
    let Some(bytes) = picture else {
        return Err(AppError::BadRequest("Choose a picture.".into()).into());
    };
    if bytes.is_empty() {
        return Err(AppError::BadRequest("Choose a picture.".into()).into());
    }
    let ext = sniff_image(&bytes)?;
    write_image(id, ext, &bytes)?;
    match set_item_image(&state.pool, &state.shop, id, ext).await {
        Ok(saved) => {
            if let Some(old) = previous.as_deref() {
                if old != ext {
                    remove_image_file(id, old);
                }
            }
            Ok(Json(saved))
        }
        Err(error) => {
            remove_image_file(id, ext);
            Err(error.into())
        }
    }
}

async fn delete_item_image(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<SavedItem>, ApiError> {
    let previous = item_image_ext(&state.pool, id).await?;
    let saved = clear_item_image(&state.pool, &state.shop, id).await?;
    if let Some(ext) = previous.as_deref() {
        remove_image_file(id, ext);
    }
    Ok(Json(saved))
}

async fn post_print(
    State(state): State<AppState>,
    Json(body): Json<PrintRequest>,
) -> Result<Json<PrintOutcome>, ApiError> {
    Ok(Json(
        print_saved_bill(&state.pool, &state.shop, &body.bill_no).await?,
    ))
}

async fn post_print_mix(
    State(state): State<AppState>,
    Json(body): Json<ReportQuery>,
) -> Result<Json<PrintOutcome>, ApiError> {
    Ok(Json(
        print_product_mix(
            &state.pool,
            &state.shop,
            body.from.as_deref(),
            body.to.as_deref(),
        )
        .await?,
    ))
}

#[derive(serde::Deserialize)]
struct ShiftPrint {
    id: i32,
}

async fn post_print_cash(
    State(state): State<AppState>,
    Json(body): Json<ShiftPrint>,
) -> Result<Json<PrintOutcome>, ApiError> {
    Ok(Json(
        print_cash_shift(&state.pool, &state.shop, body.id).await?,
    ))
}

async fn get_cash(State(state): State<AppState>) -> Result<Json<CashDesk>, ApiError> {
    Ok(Json(cash_desk(&state.pool).await?))
}

async fn post_cash_open(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OpenShift>,
) -> Result<Json<CashDesk>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    Ok(Json(open_shift(&state.pool, cashier.id, &body).await?))
}

async fn post_cash_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ShiftForm>,
) -> Result<Json<CashDesk>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    Ok(Json(
        save_shift(&state.pool, cashier.id, &body, false).await?,
    ))
}

async fn post_cash_close(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ShiftForm>,
) -> Result<Json<CashDesk>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    Ok(Json(
        save_shift(&state.pool, cashier.id, &body, true).await?,
    ))
}

async fn post_bill_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PostBillRequest>,
) -> Result<Json<PostedBill>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    Ok(Json(
        post_bill(
            &state.pool,
            &state.shop,
            cashier.id,
            body.section_id,
            &body.lines,
            body.customer.as_deref(),
        )
        .await?,
    ))
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
) -> Result<StatusCode, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    let actor = platform_kernel::context::Actor {
        user_id: Some(cashier.id),
        name: cashier.name,
        role: cashier.role,
        permissions: vec!["*".into()],
    };
    platform_runtime::admin::set_plugin_enabled(
        &state.pool,
        platform_kernel::context::TenantId::DEFAULT,
        actor,
        &id,
        body.enabled,
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
    let nav: Vec<_> = platform
        .manifests
        .iter()
        .flat_map(|m| m.nav.clone())
        .collect();
    let settings_pages: Vec<_> = platform
        .manifests
        .iter()
        .flat_map(|m| m.settings_pages.clone())
        .collect();
    let report_pages: Vec<_> = platform
        .manifests
        .iter()
        .flat_map(|m| m.report_pages.clone())
        .collect();
    Ok(Json(serde_json::json!({
        "tenant_id": "00000000-0000-4000-8000-000000000001",
        "plugins": platform.manifests,
        "permissions": ["*"],
        "nav": nav,
        "settings_pages": settings_pages,
        "report_pages": report_pages,
    })))
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
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(ErrorBody {
            error: self.0.to_string(),
        });
        (status, body).into_response()
    }
}
