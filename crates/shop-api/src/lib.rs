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
    adjust_stock, cash_desk, clear_item_image, export_items, export_movements, import_items,
    import_movements, item_image_ext, load_pos, load_reports, login, logout, needs_first_user, open_session,
    open_shift, post_bill, print_cash_shift, print_product_mix, print_saved_bill, recent_movements, save_shift,
    remove_category, remove_section, stock_summary, remove_user, reorder_categories, reorder_sections,
    save_category, save_item, save_section, save_settings, save_user, set_item_image, set_item_prompt,
    set_pos_focus, set_pos_sections, AppError, CashDesk, CategoryForm, ErrorBody,
    ItemForm, ItemPromptRequest, LoginForm, OrderForm, PgPool, LoginSession, OpenShift, PosFocus, ShiftForm,
    PosSnapshot, PosUser, PostBillRequest, PostedBill, Reports, SavedItem, StockBalance, SavedRecord,
    StockMove, StockMovement, StockPage, ItemImport, MovementImport, PrintOutcome, PrintRequest,
    SectionForm, SetFocusRequest, SetSectionsRequest, SettingsForm, ShopConfig, UserForm,
};
use tower_http::cors::{Any, CorsLayer};

#[cfg(feature = "bundle")]
mod embedded;

#[cfg(feature = "bundle")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../dist/"]
struct TillAssets;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    shop: ShopConfig,
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
    let file: FileConfig = toml::from_str(&text).unwrap_or_else(|err| {
        panic!("could not parse config.toml: {err}")
    });
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

async fn open_with(embedded: bool) -> Result<Till, String> {
    let _ = dotenvy::from_filename(".env");
    let _ = dotenvy::from_filename("../../.env");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
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
        None => std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required".to_string())?,
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
    let pool = shop_core::connect(&database_url)
        .await
        .map_err(|err| err.to_string())?;
    let shop = load_shop_config();

    let app = serve_till(Router::new()
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
        .route("/api/sections/{id}", put(put_section).delete(delete_section))
        .route("/api/categories", post(post_category))
        .route("/api/categories/order", put(put_category_order))
        .route("/api/categories/{id}", put(put_category).delete(delete_category))
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
        .layer(from_fn_with_state(
            AppState {
                pool: pool.clone(),
                shop: shop.clone(),
            },
            require_user,
        ))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(AppState {
            pool: pool.clone(),
            shop,
        }));

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

#[cfg(feature = "bundle")]
fn serve_till(api: Router) -> Router {
    use axum_frontend::{SpaOptions, embed_router_with};
    api.merge(embed_router_with::<TillAssets>(
        SpaOptions::new()
            .exclude(["/api"])
            .immutable(["/assets/"]),
    ))
}

#[cfg(not(feature = "bundle"))]
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
    Ok(Json(set_pos_sections(&state.pool, &state.shop, &body.sections).await?))
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
) -> Result<(AppendHeaders<[(header::HeaderName, HeaderValue); 1]>, Json<LoginSession>), ApiError> {
    let session = login(&state.pool, &body).await?;
    Ok((
        AppendHeaders([(header::SET_COOKIE, session_cookie(&session.token, false))]),
        Json(session),
    ))
}

async fn post_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(AppendHeaders<[(header::HeaderName, HeaderValue); 1]>, StatusCode), ApiError> {
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
    Ok(Json(save_section(&state.pool, &state.shop, None, &body).await?))
}

async fn put_section(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<SectionForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(save_section(&state.pool, &state.shop, Some(id), &body).await?))
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
    Ok(Json(reorder_sections(&state.pool, &state.shop, &body.ids).await?))
}

async fn post_category(
    State(state): State<AppState>,
    Json(body): Json<CategoryForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(save_category(&state.pool, &state.shop, None, &body).await?))
}

async fn put_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<CategoryForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(save_category(&state.pool, &state.shop, Some(id), &body).await?))
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
    Ok(Json(reorder_categories(&state.pool, &state.shop, &body.ids).await?))
}

async fn post_user(
    State(state): State<AppState>,
    Json(body): Json<UserForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(save_user(&state.pool, &state.shop, None, &body).await?))
}

async fn put_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<UserForm>,
) -> Result<Json<SavedRecord>, ApiError> {
    Ok(Json(save_user(&state.pool, &state.shop, Some(id), &body).await?))
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
    Ok(Json(import_movements(&state.pool, &state.shop, &bytes).await?))
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
    Ok(Json(save_item(&state.pool, &state.shop, None, &body).await?))
}

async fn put_item(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<ItemForm>,
) -> Result<Json<SavedItem>, ApiError> {
    Ok(Json(save_item(&state.pool, &state.shop, Some(id), &body).await?))
}

async fn put_item_prompt(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<ItemPromptRequest>,
) -> Result<Json<shop_core::PosSnapshot>, ApiError> {
    Ok(Json(set_item_prompt(&state.pool, &state.shop, id, &body).await?))
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
    std::fs::create_dir_all(&dir).map_err(|_| {
        AppError::BadRequest("The picture folder could not be created.".into())
    })?;
    let path = image_file(id, ext).ok_or_else(|| {
        AppError::BadRequest("Use a JPEG, PNG, WebP, or GIF.".into())
    })?;
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
    let bytes = std::fs::read(&path)
        .map_err(|_| AppError::NotFound("This item has no picture.".into()))?;
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
    while let Some(field) = multipart.next_field().await.map_err(|_| {
        AppError::BadRequest("That picture could not be read.".into())
    })? {
        if field.name() == Some("picture") {
            let data = field.bytes().await.map_err(|_| {
                AppError::BadRequest("That picture could not be read.".into())
            })?;
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
    Ok(Json(print_saved_bill(&state.pool, &state.shop, &body.bill_no).await?))
}

async fn post_print_mix(
    State(state): State<AppState>,
    Json(body): Json<ReportQuery>,
) -> Result<Json<PrintOutcome>, ApiError> {
    Ok(Json(
        print_product_mix(&state.pool, &state.shop, body.from.as_deref(), body.to.as_deref()).await?,
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
    Ok(Json(print_cash_shift(&state.pool, &state.shop, body.id).await?))
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
    Ok(Json(save_shift(&state.pool, cashier.id, &body, false).await?))
}

async fn post_cash_close(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ShiftForm>,
) -> Result<Json<CashDesk>, ApiError> {
    let cashier = open_session(&state.pool, &session_token(&headers)).await?;
    Ok(Json(save_shift(&state.pool, cashier.id, &body, true).await?))
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
