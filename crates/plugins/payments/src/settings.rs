use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PLUGIN_ID: &str = "payments";
pub const METHOD_PREFIX: &str = "payments.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentType {
    /// Stable slug used in tender method `payments.{id}`.
    pub id: String,
    pub label: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub requires_reference: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentsSettings {
    pub types: Vec<PaymentType>,
}

impl Default for PaymentsSettings {
    fn default() -> Self {
        Self {
            types: vec![
                PaymentType {
                    id: "card".into(),
                    label: "Card".into(),
                    enabled: true,
                    requires_reference: false,
                },
                PaymentType {
                    id: "bank".into(),
                    label: "Bank transfer".into(),
                    enabled: true,
                    requires_reference: true,
                },
                PaymentType {
                    id: "other".into(),
                    label: "Other".into(),
                    enabled: true,
                    requires_reference: true,
                },
            ],
        }
    }
}

pub fn method_for(type_id: &str) -> String {
    format!("{METHOD_PREFIX}{type_id}")
}

pub fn type_id_from_method(method: &str) -> Option<&str> {
    method.strip_prefix(METHOD_PREFIX)
}

pub async fn load(tx: &mut TenantTx<'_>) -> Result<PaymentsSettings, AppError> {
    let value: Option<Value> = sqlx::query_scalar(
        "SELECT value FROM plugin_settings WHERE plugin_id = $1",
    )
    .bind(PLUGIN_ID)
    .fetch_optional(tx.conn())
    .await?;
    let Some(value) = value else {
        return Ok(PaymentsSettings::default());
    };
    if value.as_object().is_some_and(|object| object.is_empty()) {
        return Ok(PaymentsSettings::default());
    }
    serde_json::from_value(value).map_err(|err| AppError::BadRequest(err.to_string()))
}

pub async fn save(tx: &mut TenantTx<'_>, settings: &PaymentsSettings) -> Result<(), AppError> {
    validate(settings)?;
    let normalized = PaymentsSettings {
        types: settings
            .types
            .iter()
            .map(|entry| PaymentType {
                id: entry.id.trim().to_ascii_lowercase(),
                label: entry.label.trim().to_string(),
                enabled: entry.enabled,
                requires_reference: entry.requires_reference,
            })
            .collect(),
    };
    let json =
        serde_json::to_value(&normalized).map_err(|err| AppError::BadRequest(err.to_string()))?;
    sqlx::query(
        r#"
        INSERT INTO plugin_settings (tenant_id, plugin_id, value)
        VALUES (current_setting('app.tenant_id')::uuid, $1, $2)
        ON CONFLICT (tenant_id, plugin_id) DO UPDATE
        SET value = EXCLUDED.value, updated_at = now()
        "#,
    )
    .bind(PLUGIN_ID)
    .bind(json)
    .execute(tx.conn())
    .await?;
    Ok(())
}

pub fn validate(settings: &PaymentsSettings) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    for entry in &settings.types {
        let id = entry.id.trim().to_ascii_lowercase();
        if id.is_empty() {
            return Err(AppError::BadRequest("Payment type id is required.".into()));
        }
        if id == "cash" {
            return Err(AppError::BadRequest(
                "Cash is built into the till — use another id for card/bank/wallets.".into(),
            ));
        }
        if !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(AppError::BadRequest(format!(
                "Payment type id `{id}` may only use letters, numbers, - and _."
            )));
        }
        if entry.label.trim().is_empty() {
            return Err(AppError::BadRequest(format!(
                "Payment type `{id}` needs a label."
            )));
        }
        if !seen.insert(id) {
            return Err(AppError::BadRequest(
                "Payment type ids must be unique.".into(),
            ));
        }
    }
    Ok(())
}

pub fn find_enabled<'a>(settings: &'a PaymentsSettings, type_id: &str) -> Option<&'a PaymentType> {
    settings
        .types
        .iter()
        .find(|entry| entry.enabled && entry.id.eq_ignore_ascii_case(type_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_are_namespaced() {
        assert_eq!(method_for("card"), "payments.card");
        assert_eq!(type_id_from_method("payments.bank"), Some("bank"));
        assert_eq!(type_id_from_method("gift.card"), None);
    }
}
