use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde_json::json;
use crate::billing::{load_pos_tx, post_bill_tx, set_pos_focus_tx, set_pos_sections_tx};
use crate::models::{BarcodeRule, PostBillRequest, SetFocusRequest, SetSectionsRequest, ShopConfig};

struct Fail(AppError);

impl From<AppError> for Fail {
    fn from(value: AppError) -> Self {
        Fail(value)
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

async fn snapshot(db: TenantDb) -> Result<Json<crate::models::PosSnapshot>, Fail> {
    let mut tx = db.begin(true).await?;
    let pos = load_pos_tx(tx.transaction(), &till_fallback()).await?;
    tx.commit().await?;
    Ok(Json(pos))
}

async fn focus(db: TenantDb, Json(body): Json<SetFocusRequest>) -> Result<Json<crate::models::PosFocus>, Fail> {
    let mut tx = db.begin(false).await?;
    let last_section_id = set_pos_focus_tx(tx.transaction(), body.section_id).await?;
    tx.commit().await?;
    Ok(Json(crate::models::PosFocus { last_section_id }))
}

async fn sections(
    db: TenantDb,
    Json(body): Json<SetSectionsRequest>,
) -> Result<Json<crate::models::PosSnapshot>, Fail> {
    let mut tx = db.begin(false).await?;
    let pos = set_pos_sections_tx(tx.transaction(), &till_fallback(), &body.sections).await?;
    tx.commit().await?;
    Ok(Json(pos))
}

async fn post_bill(db: TenantDb, Json(body): Json<PostBillRequest>) -> Result<Json<crate::models::PostedBill>, Fail> {
    let user_id = db
        .actor()
        .user_id
        .ok_or_else(|| AppError::Unauthorized("Sign in before posting a bill.".into()))?;
    let mut tx = db.begin(false).await?;
    let mut posted = post_bill_tx(
        tx.transaction(),
        &till_fallback(),
        user_id,
        body.section_id,
        &body.lines,
        body.customer.as_deref(),
    )
    .await?;
    plugin_inventory::sale::apply_bill(tx.transaction(), &posted.bill_no).await?;
    posted.pos = load_pos_tx(tx.transaction(), &till_fallback()).await?;
    sqlx::query(
        r#"
        INSERT INTO platform_events (tenant_id, event_type, version, payload)
        VALUES (current_setting('app.tenant_id')::uuid, 'sales.bill_posted', 1, $1)
        "#,
    )
    .bind(json!({ "bill_no": posted.bill_no, "total": posted.total }))
    .execute(tx.conn())
    .await
    .map_err(AppError::from)?;
    tx.commit().await?;
    Ok(Json(posted))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("snapshot", snapshot, "pos.read")?;
    routes.put("focus", focus, "pos.bills.post")?;
    routes.put("sections", sections, "pos.bills.post")?;
    routes.post("bills", post_bill, "pos.bills.post")?;
    Ok(())
}
