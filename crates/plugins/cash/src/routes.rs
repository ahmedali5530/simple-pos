use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};

use crate::desk::{cash_desk, open_shift, save_shift, CashDesk, OpenShift, ShiftForm};

async fn desk(db: TenantDb) -> Result<Json<CashDesk>, AppError> {
    let mut tx = db.begin(true).await?;
    let desk = cash_desk(tx.transaction()).await?;
    tx.commit().await?;
    Ok(Json(desk))
}

async fn open(db: TenantDb, Json(body): Json<OpenShift>) -> Result<Json<CashDesk>, AppError> {
    let user_id = db
        .actor()
        .user_id
        .ok_or_else(|| AppError::Unauthorized("Sign in before opening the till.".into()))?;
    let mut tx = db.begin(false).await?;
    let desk = open_shift(tx.transaction(), user_id, &body).await?;
    tx.commit().await?;
    Ok(Json(desk))
}

async fn draft(db: TenantDb, Json(body): Json<ShiftForm>) -> Result<Json<CashDesk>, AppError> {
    save(db, body, false).await
}

async fn close(db: TenantDb, Json(body): Json<ShiftForm>) -> Result<Json<CashDesk>, AppError> {
    save(db, body, true).await
}

async fn save(db: TenantDb, body: ShiftForm, close_shift: bool) -> Result<Json<CashDesk>, AppError> {
    let user_id = db
        .actor()
        .user_id
        .ok_or_else(|| AppError::Unauthorized("Sign in before closing the till.".into()))?;
    let mut tx = db.begin(false).await?;
    let desk = save_shift(tx.transaction(), user_id, &body, close_shift).await?;
    tx.commit().await?;
    Ok(Json(desk))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("desk", desk, "cash.read")?;
    routes.post("open", open, "cash.write")?;
    routes.post("draft", draft, "cash.write")?;
    routes.post("close", close, "cash.write")?;
    Ok(())
}
