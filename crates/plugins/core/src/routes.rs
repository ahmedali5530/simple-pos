use crate::models::{ShopProfileForm, ShopSnapshot};
use crate::shop::current_shop_tx;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};

async fn snapshot(db: TenantDb) -> Result<Json<ShopSnapshot>, AppError> {
    let mut tx = db.begin(true).await?;
    let shop = current_shop_tx(tx.transaction()).await?;
    tx.commit().await?;
    Ok(Json(shop))
}

async fn settings(
    db: TenantDb,
    Json(body): Json<ShopProfileForm>,
) -> Result<Json<ShopSnapshot>, AppError> {
    let name = body.name.trim().to_string();
    let currency = body.currency.trim().to_ascii_uppercase();
    let locale = body.locale.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::BadRequest(
            "Enter a shop name up to 80 characters.".into(),
        ));
    }
    if currency.len() != 3 || !currency.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(AppError::BadRequest(
            "Currency is a 3-letter code, such as PKR.".into(),
        ));
    }
    if locale.is_empty() || locale.len() > 35 {
        return Err(AppError::BadRequest(
            "Enter a locale, such as en-PK.".into(),
        ));
    }

    let mut tx = db.begin(false).await?;
    sqlx::query(
        "UPDATE tenant_profile SET name = $1, currency = $2, locale = $3 WHERE tenant_id = current_setting('app.tenant_id')::uuid",
    )
    .bind(&name)
    .bind(&currency)
    .bind(&locale)
    .execute(tx.conn())
    .await?;
    let snapshot = current_shop_tx(tx.transaction()).await?;
    tx.commit().await?;
    Ok(Json(snapshot))
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("snapshot", snapshot, "core.read")?;
    routes.put("settings", settings, "settings.write")?;
    Ok(())
}
