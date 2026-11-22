use axum::extract::Query;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ledger::{adjust_stock, recent_movements, stock_summary, StockMove};

#[derive(Deserialize)]
struct MovementQuery {
    item_id: Option<i32>,
}

async fn summary(db: TenantDb) -> Result<Json<Vec<crate::ledger::StockBalance>>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows = stock_summary(tx.transaction()).await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn movements(
    db: TenantDb,
    Query(query): Query<MovementQuery>,
) -> Result<Json<Vec<crate::ledger::StockMovement>>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows = recent_movements(tx.transaction(), query.item_id).await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn adjust(db: TenantDb, Json(body): Json<StockMove>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let movements = adjust_stock(tx.transaction(), &body).await?;
    tx.commit().await?;
    Ok(Json(json!({ "movements": movements })))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("summary", summary, "inventory.read")?;
    routes.get("movements", movements, "inventory.read")?;
    routes.post("adjust", adjust, "inventory.write")?;
    Ok(())
}
