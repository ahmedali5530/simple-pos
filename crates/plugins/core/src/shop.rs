use crate::models::ShopSnapshot;
use platform_kernel::error::AppError;

pub async fn current_shop_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<ShopSnapshot, AppError> {
    let row = sqlx::query_as::<_, ShopRow>(
        r#"
        SELECT name, currency, locale
        FROM tenant_profile
        WHERE tenant_id = current_setting('app.tenant_id')::uuid
        "#,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(ShopSnapshot {
            shop_name: "Shop".into(),
            currency: "PKR".into(),
            locale: "en-PK".into(),
        });
    };
    Ok(ShopSnapshot {
        shop_name: row.name,
        currency: row.currency,
        locale: row.locale,
    })
}

#[derive(sqlx::FromRow)]
struct ShopRow {
    name: String,
    currency: String,
    locale: String,
}
