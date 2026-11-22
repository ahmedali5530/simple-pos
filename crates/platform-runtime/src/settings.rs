//! Per-tenant plugin settings. The engine stores JSON. The plugin owns the schema.

use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

pub async fn load(tx: &mut TenantTx<'_>, plugin_id: &str) -> Result<Value, AppError> {
    let value: Option<Value> = sqlx::query_scalar(
        "SELECT value FROM plugin_settings WHERE plugin_id = $1",
    )
    .bind(plugin_id)
    .fetch_optional(tx.conn())
    .await?;
    Ok(value.unwrap_or_else(|| Value::Object(Default::default())))
}

pub async fn load_as<T: DeserializeOwned>(tx: &mut TenantTx<'_>, plugin_id: &str) -> Result<T, AppError> {
    let value = load(tx, plugin_id).await?;
    serde_json::from_value(value).map_err(|err| AppError::BadRequest(err.to_string()))
}

pub async fn save<T: Serialize>(tx: &mut TenantTx<'_>, plugin_id: &str, value: &T) -> Result<(), AppError> {
    let json = serde_json::to_value(value).map_err(|err| AppError::BadRequest(err.to_string()))?;
    sqlx::query(
        r#"
        INSERT INTO plugin_settings (tenant_id, plugin_id, value)
        VALUES (current_setting('app.tenant_id')::uuid, $1, $2)
        ON CONFLICT (tenant_id, plugin_id) DO UPDATE
        SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(plugin_id)
    .bind(json)
    .execute(tx.conn())
    .await?;
    Ok(())
}
