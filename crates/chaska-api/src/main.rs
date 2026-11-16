use std::net::SocketAddr;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use chaska_core::{
    load_pos, post_bill, set_item_prompt, set_pos_focus, set_pos_sections, AppError, ErrorBody,
    ItemPromptRequest, PgPool, PosFocus, PostBillRequest, PostedBill, SetFocusRequest,
    SetSectionsRequest, ShopConfig,
};
use tower_http::cors::{Any, CorsLayer};

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
    rules: Vec<chaska_core::BarcodeRule>,
}

fn load_shop_config() -> ShopConfig {
    let path = ["config.toml", "../../config.toml"]
        .into_iter()
        .find(|candidate| std::path::Path::new(candidate).is_file())
        .unwrap_or_else(|| panic!("config.toml was not found"));
    let text = std::fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("could not read {path}: {err}")
    });
    let file: FileConfig = toml::from_str(&text).unwrap_or_else(|err| {
        panic!("could not parse {path}: {err}")
    });
    let mut shop = file.shop.checked().unwrap_or_else(|err| panic!("{err}"));
    shop.barcode_rules = if file.barcode.rules.is_empty() {
        chaska_core::BarcodeRule::defaults()
    } else {
        file.barcode
            .rules
            .into_iter()
            .map(|rule| rule.checked().unwrap_or_else(|err| panic!("{err}")))
            .collect()
    };
    shop
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::from_filename(".env");
    let _ = dotenvy::from_filename("../../.env");
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "chaska_api=info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://chaska:chaska@127.0.0.1:5433/chaska_pos".to_string()
    });
    let pool = chaska_core::connect(&database_url)
        .await
        .expect("database connection and migrations");
    let shop = load_shop_config();

    let app = Router::new()
        .route("/api/pos", get(get_pos))
        .route("/api/pos/focus", put(put_focus))
        .route("/api/pos/sections", put(put_sections))
        .route("/api/bills", axum::routing::post(post_bill_route))
        .route("/api/items/{id}/prompt", put(put_item_prompt))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(AppState { pool, shop });

    let addr: SocketAddr = std::env::var("API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".into())
        .parse()
        .expect("API_ADDR");
    tracing::info!("Chaska API listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("server");
}

async fn get_pos(State(state): State<AppState>) -> Result<Json<chaska_core::PosSnapshot>, ApiError> {
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
) -> Result<Json<chaska_core::PosSnapshot>, ApiError> {
    Ok(Json(set_pos_sections(&state.pool, &state.shop, &body.sections).await?))
}

async fn put_item_prompt(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<ItemPromptRequest>,
) -> Result<Json<chaska_core::PosSnapshot>, ApiError> {
    Ok(Json(set_item_prompt(&state.pool, &state.shop, id, &body).await?))
}

async fn post_bill_route(
    State(state): State<AppState>,
    Json(body): Json<PostBillRequest>,
) -> Result<Json<PostedBill>, ApiError> {
    Ok(Json(
        post_bill(&state.pool, &state.shop, body.section_id, &body.lines).await?,
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
            AppError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(ErrorBody {
            error: self.0.to_string(),
        });
        (status, body).into_response()
    }
}
