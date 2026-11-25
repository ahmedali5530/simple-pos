use crate::models::{ShopConfig, ShopProfileForm, ShopSnapshot};
use crate::shop::current_shop_tx;
use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};

async fn snapshot(db: TenantDb) -> Result<Json<ShopSnapshot>, AppError> {
    let mut tx = db.begin(true).await?;
    let shop = load_core_snapshot_tx(&mut tx).await?;
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
    let bill_prefix = body.bill_prefix.trim().to_ascii_uppercase();
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
    if bill_prefix.is_empty()
        || bill_prefix.len() > 8
        || !bill_prefix.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return Err(AppError::BadRequest(
            "The bill prefix is 1 to 8 letters or digits.".into(),
        ));
    }
    if body.barcode_rules.len() > 20 {
        return Err(AppError::BadRequest("Keep barcode rules to 20.".into()));
    }
    let mut rules = Vec::with_capacity(body.barcode_rules.len());
    let mut seen = std::collections::HashSet::new();
    for rule in body.barcode_rules {
        let rule = rule.checked()?;
        if !seen.insert(rule.prefix.clone()) {
            return Err(AppError::BadRequest(
                "Each barcode prefix is used once.".into(),
            ));
        }
        rules.push(rule);
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
    sqlx::query(
        r#"
        INSERT INTO pos_settings (tenant_id, bill_prefix)
        VALUES (current_setting('app.tenant_id')::uuid, $1)
        ON CONFLICT (tenant_id) DO UPDATE SET bill_prefix = EXCLUDED.bill_prefix
        "#,
    )
    .bind(&bill_prefix)
    .execute(tx.conn())
    .await?;
    sqlx::query("DELETE FROM barcode_rules_v2")
        .execute(tx.conn())
        .await?;
    for (index, rule) in rules.iter().enumerate() {
        sqlx::query(
            "INSERT INTO barcode_rules_v2 (prefix, mode, scale, sort_order) VALUES ($1, $2, $3, $4)",
        )
        .bind(&rule.prefix)
        .bind(&rule.mode)
        .bind(rule.scale)
        .bind(index as i32)
        .execute(tx.conn())
        .await
        .map_err(friendly_unique)?;
    }
    let snapshot = load_core_snapshot_tx(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(snapshot))
}

fn friendly_unique(err: sqlx::Error) -> AppError {
    let constraint = err
        .as_database_error()
        .and_then(|db| db.constraint().map(str::to_string));
    match constraint.as_deref() {
        Some("barcode_rules_prefix_key") => {
            AppError::BadRequest("Each barcode prefix is used once.".into())
        }
        _ => AppError::Db(err),
    }
}

fn till_fallback() -> ShopConfig {
    ShopConfig {
        name: "Shop".into(),
        currency: "PKR".into(),
        locale: "en-PK".into(),
        bill_prefix: "BILL".into(),
        barcode_rules: Vec::new(),
    }
}

async fn load_core_snapshot_tx(
    tx: &mut platform_kernel::context::TenantTx<'_>,
) -> Result<ShopSnapshot, AppError> {
    let shop = current_shop_tx(tx.transaction(), &till_fallback()).await?;
    Ok(ShopSnapshot {
        shop_name: shop.name,
        bill_prefix: shop.bill_prefix,
        currency: shop.currency,
        locale: shop.locale,
        barcode_rules: shop.barcode_rules,
    })
}

pub fn mount(routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
    routes.get("snapshot", snapshot, "core.read")?;
    routes.put("settings", settings, "settings.write")?;
    Ok(())
}
