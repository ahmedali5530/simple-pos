use crate::models::{BarcodeRule, ShopConfig};
use platform_kernel::error::AppError;

pub async fn current_shop_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    fallback: &ShopConfig,
) -> Result<ShopConfig, AppError> {
    let row = sqlx::query_as::<_, ShopRow>(
        r#"
        SELECT profile.name, profile.currency, profile.locale,
               COALESCE(settings.bill_prefix, 'BILL') AS bill_prefix
        FROM tenant_profile profile
        LEFT JOIN pos_settings settings ON settings.tenant_id = profile.tenant_id
        WHERE profile.tenant_id = current_setting('app.tenant_id')::uuid
        "#,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(fallback.clone());
    };
    let rules = sqlx::query_as::<_, RuleRow>(
        "SELECT prefix, mode, scale FROM barcode_rules_v2 ORDER BY sort_order, id",
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(ShopConfig {
        name: row.name,
        currency: row.currency,
        locale: row.locale,
        bill_prefix: row.bill_prefix,
        barcode_rules: rules
            .into_iter()
            .map(|rule| BarcodeRule {
                prefix: rule.prefix,
                mode: rule.mode,
                scale: rule.scale,
            })
            .collect(),
    })
}

#[derive(sqlx::FromRow)]
struct ShopRow {
    name: String,
    currency: String,
    locale: String,
    bill_prefix: String,
}

#[derive(sqlx::FromRow)]
struct RuleRow {
    prefix: String,
    mode: String,
    scale: i32,
}
