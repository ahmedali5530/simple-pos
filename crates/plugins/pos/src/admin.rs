use crate::models::{BarcodeRule, PosSnapshot, TillShop, PosSettingsForm};
use platform_kernel::error::AppError;

fn friendly_unique(err: sqlx::Error) -> AppError {
    let constraint = err
        .as_database_error()
        .and_then(|db| db.constraint().map(str::to_string));
    match constraint.as_deref() {
        Some("sections_short_name_lower_idx") => {
            AppError::BadRequest("That short name is already used.".into())
        }
        Some("categories_name_key") | Some("categories_name_lower_idx") => {
            AppError::BadRequest("That category name is already used.".into())
        }
        Some("barcode_rules_prefix_key") => {
            AppError::BadRequest("Each barcode prefix is used once.".into())
        }
        _ => AppError::Db(err),
    }
}

pub async fn save_shop_profile(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    catalog: Option<&catalog_contract::CatalogApi>,
    fallback: &TillShop,
    form: &PosSettingsForm,
) -> Result<PosSnapshot, AppError> {
    let bill_prefix = form.bill_prefix.trim().to_ascii_uppercase();
    if bill_prefix.is_empty()
        || bill_prefix.len() > 8
        || !bill_prefix.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return Err(AppError::BadRequest(
            "The bill prefix is 1 to 8 letters or digits.".into(),
        ));
    }
    if form.barcode_rules.len() > 20 {
        return Err(AppError::BadRequest("Keep barcode rules to 20.".into()));
    }
    let mut seen = std::collections::HashSet::new();
    for rule in &form.barcode_rules {
        check_rule(rule)?;
        if !seen.insert(rule.prefix.clone()) {
            return Err(AppError::BadRequest(
                "Each barcode prefix is used once.".into(),
            ));
        }
    }

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
    for (index, rule) in form.barcode_rules.iter().enumerate() {
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
    match catalog {
        Some(catalog) => crate::billing::load_pos_catalog_tx(tx, fallback, catalog).await,
        None => crate::billing::load_pos_shop_tx(tx, fallback).await,
    }
}

fn check_rule(rule: &BarcodeRule) -> Result<(), AppError> {
    match rule.clone().checked() {
        Ok(_) => Ok(()),
        Err(message) if message.contains("prefix must") => Err(AppError::BadRequest(
            "A barcode prefix is 1 to 4 digits.".into(),
        )),
        Err(message) if message.contains("mode") => Err(AppError::BadRequest(
            "Each barcode rule is weight or price.".into(),
        )),
        Err(message) if message.contains("scale") => Err(AppError::BadRequest(
            "Barcode scale must be at least 1.".into(),
        )),
        Err(message) if message.contains("room") => Err(AppError::BadRequest(
            "That prefix leaves no room for an item code.".into(),
        )),
        Err(message) => Err(AppError::BadRequest(message)),
    }
}
