//! Core-owned shop identity and localization settings.

use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ShopProfile {
    pub name: String,
    pub currency: String,
    pub locale: String,
    pub timezone: String,
}

#[derive(Debug, Deserialize)]
pub struct ShopProfileForm {
    pub name: String,
    pub currency: String,
    pub locale: String,
}

pub async fn load(pool: &PgPool, tenant_id: TenantId) -> Result<ShopProfile, AppError> {
    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), true).await?;
    let profile = sqlx::query_as::<_, ShopProfile>(
        "SELECT name, currency, locale, timezone FROM tenant_profile WHERE tenant_id = $1",
    )
    .bind(tenant_id.as_uuid())
    .fetch_optional(tx.conn())
    .await?
    .ok_or_else(|| AppError::NotFound("Shop profile is not set up.".into()))?;
    tx.commit().await?;
    Ok(profile)
}

pub async fn load_tx(tx: &mut TenantTx<'_>) -> Result<ShopProfile, AppError> {
    sqlx::query_as::<_, ShopProfile>(
        "SELECT name, currency, locale, timezone FROM tenant_profile WHERE tenant_id = $1",
    )
    .bind(tx.tenant_id.as_uuid())
    .fetch_optional(tx.conn())
    .await?
    .ok_or_else(|| AppError::NotFound("Shop profile is not set up.".into()))
}

pub async fn save(
    pool: &PgPool,
    tenant_id: TenantId,
    form: &ShopProfileForm,
) -> Result<ShopProfile, AppError> {
    let name = form.name.trim();
    let currency = form.currency.trim().to_ascii_uppercase();
    let locale = form.locale.trim();
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

    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
    sqlx::query(
        "UPDATE tenant_profile SET name = $2, currency = $3, locale = $4 WHERE tenant_id = $1",
    )
    .bind(tenant_id.as_uuid())
    .bind(name)
    .bind(currency)
    .bind(locale)
    .execute(tx.conn())
    .await?;
    let profile = sqlx::query_as::<_, ShopProfile>(
        "SELECT name, currency, locale, timezone FROM tenant_profile WHERE tenant_id = $1",
    )
    .bind(tenant_id.as_uuid())
    .fetch_one(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(profile)
}
