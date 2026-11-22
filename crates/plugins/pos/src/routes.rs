use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::permission::builtins;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde_json::json;
use shop_core::{
    load_pos_tx, post_bill_tx, set_pos_focus_tx, set_pos_sections_tx, AppError as ShopError, BarcodeRule,
    PostBillRequest, SetFocusRequest, SetSectionsRequest, ShopConfig,
};

struct Fail(AppError);

impl From<AppError> for Fail {
    fn from(value: AppError) -> Self {
        Fail(value)
    }
}

impl From<ShopError> for Fail {
    fn from(value: ShopError) -> Self {
        Fail(match value {
            ShopError::BadRequest(message) => AppError::BadRequest(message),
            ShopError::NotFound(message) => AppError::NotFound(message),
            ShopError::Unauthorized(message) => AppError::Unauthorized(message),
            ShopError::Db(error) => AppError::Db(error),
        })
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}

fn till_fallback() -> ShopConfig {
    ShopConfig {
        name: "Shop".into(),
        currency: "PKR".into(),
        locale: "en-PK".into(),
        bill_prefix: "BILL".into(),
        barcode_rules: BarcodeRule::defaults(),
    }
}

async fn snapshot(db: TenantDb) -> Result<Json<shop_core::PosSnapshot>, Fail> {
    let mut tx = db.begin(true).await?;
    let pos = load_pos_tx(tx.transaction(), &till_fallback()).await?;
    tx.commit().await?;
    Ok(Json(pos))
}

async fn focus(db: TenantDb, Json(body): Json<SetFocusRequest>) -> Result<Json<shop_core::PosFocus>, Fail> {
    let mut tx = db.begin(false).await?;
    let last_section_id = set_pos_focus_tx(tx.transaction(), body.section_id).await?;
    tx.commit().await?;
    Ok(Json(shop_core::PosFocus { last_section_id }))
}

async fn sections(
    db: TenantDb,
    Json(body): Json<SetSectionsRequest>,
) -> Result<Json<shop_core::PosSnapshot>, Fail> {
    let mut tx = db.begin(false).await?;
    let pos = set_pos_sections_tx(tx.transaction(), &till_fallback(), &body.sections).await?;
    tx.commit().await?;
    Ok(Json(pos))
}

async fn post_bill(db: TenantDb, Json(body): Json<PostBillRequest>) -> Result<Json<shop_core::PostedBill>, Fail> {
    let user_id = db
        .actor()
        .user_id
        .ok_or_else(|| AppError::Unauthorized("Sign in before posting a bill.".into()))?;
    let mut tx = db.begin(false).await?;
    let posted = post_bill_tx(
        tx.transaction(),
        &till_fallback(),
        user_id,
        body.section_id,
        &body.lines,
        body.customer.as_deref(),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(posted))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("snapshot", snapshot, builtins::POS_READ)?;
    routes.put("focus", focus, builtins::POS_BILL)?;
    routes.put("sections", sections, builtins::POS_BILL)?;
    routes.post("bills", post_bill, builtins::POS_BILL)?;
    Ok(())
}
