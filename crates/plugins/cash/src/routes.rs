use axum::Json;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use sales_contract::SalesQueryProvider;

use crate::desk::{
    cash_desk, lock_open_since, open_shift, open_since, save_shift, CashDesk, OpenShift, ShiftForm,
};

async fn current_sales(
    provider: &SalesQueryProvider,
    tx: &mut TenantTx<'_>,
    lock: bool,
) -> Result<i64, AppError> {
    let since = if lock {
        lock_open_since(tx.transaction()).await?
    } else {
        open_since(tx.transaction()).await?
    };
    match since {
        Some(since) => Ok(provider.0.total_since(tx, &since).await?.total),
        None => Ok(0),
    }
}

async fn desk(db: TenantDb) -> Result<Json<CashDesk>, AppError> {
    let provider = db.provider::<SalesQueryProvider>().ok_or_else(|| {
        AppError::Conflict("Point of Sale is not enabled.".into())
    })?;
    let mut tx = db.begin(true).await?;
    let sales = current_sales(&provider, &mut tx, false).await?;
    let desk = cash_desk(tx.transaction(), sales).await?;
    tx.commit().await?;
    Ok(Json(desk))
}

async fn open(db: TenantDb, Json(body): Json<OpenShift>) -> Result<Json<CashDesk>, AppError> {
    let user_id = db
        .actor()
        .user_id
        .ok_or_else(|| AppError::Unauthorized("Sign in before opening the till.".into()))?;
    let user_name = db.actor().name.clone();
    let mut tx = db.begin(false).await?;
    let desk = open_shift(tx.transaction(), user_id, &user_name, &body).await?;
    tx.commit().await?;
    Ok(Json(desk))
}

async fn draft(db: TenantDb, Json(body): Json<ShiftForm>) -> Result<Json<CashDesk>, AppError> {
    save(db, body, false).await
}

async fn close(db: TenantDb, Json(body): Json<ShiftForm>) -> Result<Json<CashDesk>, AppError> {
    save(db, body, true).await
}

async fn save(
    db: TenantDb,
    body: ShiftForm,
    close_shift: bool,
) -> Result<Json<CashDesk>, AppError> {
    let user_id = db
        .actor()
        .user_id
        .ok_or_else(|| AppError::Unauthorized("Sign in before closing the till.".into()))?;
    let provider = db.provider::<SalesQueryProvider>().ok_or_else(|| {
        AppError::Conflict("Point of Sale is not enabled.".into())
    })?;
    let user_name = db.actor().name.clone();
    let mut tx = db.begin(false).await?;
    let sales = current_sales(&provider, &mut tx, true).await?;
    let desk = save_shift(
        tx.transaction(),
        user_id,
        &user_name,
        &body,
        close_shift,
        sales,
    )
    .await?;
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
