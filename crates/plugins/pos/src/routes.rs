use crate::billing::{
    load_pos_catalog_tx, load_pos_shop_tx, post_bill_tx, set_pos_focus_tx, set_pos_sections_tx,
    PostBillOutcome,
};
use crate::models::{PostBillRequest, SetFocusRequest, SetSectionsRequest, TillShop};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use catalog_contract::CatalogApi;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use platform_runtime::events::Outbox;
use serde_json::json;

struct Fail(AppError);

impl From<AppError> for Fail {
    fn from(value: AppError) -> Self {
        Fail(value)
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> axum::response::Response {
        let status =
            StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}

fn till_fallback() -> TillShop {
    TillShop::fallback()
}

fn catalog_needed(action: &str) -> AppError {
    AppError::Conflict(format!(
        "A catalog provider must be enabled {action}."
    ))
}

async fn snapshot(db: TenantDb) -> Result<Json<crate::models::PosSnapshot>, Fail> {
    let mut tx = db.begin(true).await?;
    let pos = match db.provider::<CatalogApi>() {
        Some(catalog) => load_pos_catalog_tx(&mut tx, &till_fallback(), &catalog).await?,
        None => load_pos_shop_tx(&mut tx, &till_fallback()).await?,
    };
    tx.commit().await?;
    Ok(Json(pos))
}

async fn settings(
    db: TenantDb,
    Json(body): Json<crate::models::PosSettingsForm>,
) -> Result<Json<crate::models::PosSnapshot>, Fail> {
    let catalog = db.provider::<CatalogApi>();
    let mut tx = db.begin(false).await?;
    let pos = crate::admin::save_shop_profile(
        &mut tx,
        catalog.as_deref(),
        &till_fallback(),
        &body,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(pos))
}

async fn focus(
    db: TenantDb,
    Json(body): Json<SetFocusRequest>,
) -> Result<Json<crate::models::PosFocus>, Fail> {
    let catalog = db
        .provider::<CatalogApi>()
        .ok_or_else(|| catalog_needed("to set the till focus"))?;
    let mut tx = db.begin(false).await?;
    let last_section_id = set_pos_focus_tx(&mut tx, &catalog, body.section_id).await?;
    tx.commit().await?;
    Ok(Json(crate::models::PosFocus { last_section_id }))
}

async fn sections(
    db: TenantDb,
    Json(body): Json<SetSectionsRequest>,
) -> Result<Json<crate::models::PosSnapshot>, Fail> {
    let catalog = db
        .provider::<CatalogApi>()
        .ok_or_else(|| catalog_needed("to change the till layout"))?;
    let mut tx = db.begin(false).await?;
    let pos = set_pos_sections_tx(&mut tx, &till_fallback(), &catalog, &body.sections).await?;
    tx.commit().await?;
    Ok(Json(pos))
}

async fn post_bill(
    db: TenantDb,
    Json(body): Json<PostBillRequest>,
) -> Result<Json<crate::models::PostedBill>, Fail> {
    let user_id = db
        .actor()
        .user_id
        .ok_or_else(|| AppError::Unauthorized("Sign in before posting a bill.".into()))?;
    let catalog = db
        .provider::<CatalogApi>()
        .ok_or_else(|| catalog_needed("to post a bill"))?;
    let mut tx = db.begin(false).await?;
    let PostBillOutcome { mut posted, event } = post_bill_tx(
        &mut tx,
        &till_fallback(),
        &catalog,
        user_id,
        body.section_id,
        &body.lines,
        body.customer.as_deref(),
    )
    .await?;
    if let Some(ledger) = db.provider::<inventory_contract::StockLedgerProvider>() {
        let issues: Vec<_> = event
            .lines
            .iter()
            .filter_map(|line| Some((line.item_id?, line.stock_quantity)))
            .filter(|(_, qty)| *qty > 0)
            .map(|(item_id, qty)| inventory_contract::StockIssue {
                item_id,
                qty,
                reason: format!("POS bill {}", event.bill_no),
                source_type: "sale".into(),
                source_id: Some(event.bill_id),
                bill_id: Some(event.bill_id),
                bill_no: Some(event.bill_no.clone()),
            })
            .collect();
        ledger.0.issue(&mut tx, &issues).await?;
    }
    posted.pos = load_pos_catalog_tx(&mut tx, &till_fallback(), &catalog).await?;
    Outbox::publish(&mut tx, &event).await?;
    tx.commit().await?;
    Ok(Json(posted))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("snapshot", snapshot, "pos.read")?;
    routes.put("settings", settings, "settings.write")?;
    routes.put("focus", focus, "pos.bills.post")?;
    routes.put("sections", sections, "pos.bills.post")?;
    routes.post("bills", post_bill, "pos.bills.post")?;
    Ok(())
}
