//! Warranty start from a posted bill. The event says which bill; the database says which units.

use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
use sales_contract::BillPostedV3;
use serde_json::Value;
use tracing::warn;

pub struct StartWarranties;

#[async_trait]
impl EventHandler for StartWarranties {
    fn event_type(&self) -> &str {
        "sales.bill_posted"
    }

    fn subscriber_id(&self) -> &str {
        "start_warranties"
    }

    async fn handle(
        &self,
        tx: &mut TenantTx<'_>,
        envelope: &EventEnvelope,
    ) -> Result<(), AppError> {
        if envelope.version < 3 {
            return Ok(());
        }
        let event: BillPostedV3 = serde_json::from_value(envelope.payload.clone())
            .map_err(|error| AppError::BadRequest(format!("invalid sales event: {error}")))?;
        apply_bill(tx, &event).await
    }
}

/// Mark the devices from the sales event sold and start their warranties.
/// The event contains the sale facts, so this plugin does not query POS tables.
async fn apply_bill(tx: &mut TenantTx<'_>, event: &BillPostedV3) -> Result<(), AppError> {
    let lines: Vec<_> = event.lines.iter().filter(|line| line.source == "mobile.device").collect();
    if lines.is_empty() {
        return Ok(());
    }

    let timezone: String = sqlx::query_scalar("SELECT timezone FROM tenant_profile LIMIT 1")
        .fetch_optional(tx.conn())
        .await?
        .unwrap_or_else(|| "UTC".into());

    let mut seen = std::collections::HashSet::new();
    for (line_index, line) in lines.into_iter().enumerate() {
        let qty = line.quantity.parse::<i32>().unwrap_or_default();
        let Some(item_id) = line.item_id else {
            warn!(bill_id = event.bill_id, line_index, "mobile sale line has no catalog item id");
            continue;
        };
        if qty != 1 {
            warn!(bill_id = event.bill_id, line_index, qty, "mobile device line has quantity other than one");
            continue;
        }
        let Some(device_id) = device_id_from_line(&line.reference, &line.details) else {
            warn!(bill_id = event.bill_id, line_index, "mobile device line has no device reference");
            continue;
        };
        if !seen.insert(device_id) {
            warn!(bill_id = event.bill_id, line_index, device_id, "mobile device repeats on the same bill");
            continue;
        }

        let updated = sqlx::query(
            r#"
            UPDATE mobile_devices d
            SET status = 'sold',
                sold_bill_id = $2,
                sold_bill_no = $3,
                sold_at = $4::timestamptz,
                customer_name = COALESCE($5, ''),
                updated_at = now()
            WHERE d.id = $1
              AND d.item_id = $6
              AND d.status = 'in_stock'
            "#,
        )
        .bind(device_id)
        .bind(event.bill_id)
        .bind(&event.bill_no)
        .bind(&event.posted_at)
        .bind(event.customer.as_deref())
        .bind(item_id)
        .execute(tx.conn())
        .await?;

        if updated.rows_affected() == 0 {
            let sold_here: Option<bool> = sqlx::query_scalar(
                "SELECT sold_bill_id = $1 FROM mobile_devices WHERE id = $2",
            )
            .bind(event.bill_id)
            .bind(device_id)
            .fetch_optional(tx.conn())
            .await?;
            if sold_here != Some(true) {
                warn!(bill_id = event.bill_id, device_id, "mobile device is not available for this sale");
                continue;
            }
        }

        sqlx::query(
            r#"
            INSERT INTO mobile_warranties
                (device_id, bill_id, bill_no, months, sold_at, starts_on, ends_on, customer_name)
            SELECT d.id, $2, $3, d.warranty_months, $4::timestamptz,
                   (($4::timestamptz) AT TIME ZONE $5::text)::date,
                   ((($4::timestamptz) AT TIME ZONE $5::text)
                     + make_interval(months => d.warranty_months))::date,
                   COALESCE($6, '')
            FROM mobile_devices d
            WHERE d.id = $1 AND d.sold_bill_id = $2
            ON CONFLICT (tenant_id, bill_id, device_id) DO NOTHING
            "#,
        )
        .bind(device_id)
        .bind(event.bill_id)
        .bind(&event.bill_no)
        .bind(&event.posted_at)
        .bind(&timezone)
        .bind(event.customer.as_deref())
        .execute(tx.conn())
        .await?;
    }
    Ok(())
}

fn device_id_from_line(reference: &str, details: &Value) -> Option<i32> {
    details
        .get("device_id")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .filter(|id| *id > 0)
        .or_else(|| {
            reference
                .strip_prefix("device:")
                .and_then(|raw| raw.parse::<i32>().ok())
                .filter(|id| *id > 0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_device_from_details_or_reference() {
        assert_eq!(device_id_from_line("", &json!({"device_id": 7})), Some(7));
        assert_eq!(device_id_from_line("device:12", &json!({})), Some(12));
        assert_eq!(device_id_from_line("job:4", &json!({})), None);
        assert_eq!(device_id_from_line("", &json!({"device_id": "7"})), None);
        assert_eq!(device_id_from_line("", &json!({"device_id": 0})), None);
    }
}
