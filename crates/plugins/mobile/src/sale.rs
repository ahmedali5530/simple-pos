//! Warranty start from a posted bill. The event says which bill; the database says which units.

use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::{EventEnvelope, EventHandler};
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
        if envelope.version < 2 {
            return Ok(());
        }
        let Some(bill_id) = bill_id_for(tx, &envelope.payload).await? else {
            return Ok(());
        };
        apply_bill(tx, bill_id).await
    }
}

async fn bill_id_for(
    tx: &mut TenantTx<'_>,
    payload: &Value,
) -> Result<Option<i32>, AppError> {
    let by_id = payload
        .get("bill_id")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .filter(|id| *id > 0);
    if by_id.is_some() {
        return Ok(by_id);
    }
    let Some(bill_no) = payload.get("bill_no").and_then(Value::as_str) else {
        return Ok(None);
    };
    let id: Option<i32> = sqlx::query_scalar("SELECT id FROM bills WHERE bill_no = $1")
        .bind(bill_no)
        .fetch_optional(tx.conn())
        .await?;
    Ok(id)
}

/// Mark the devices on this bill sold and start their warranties. Safe to run more than once.
pub async fn apply_bill(tx: &mut TenantTx<'_>, bill_id: i32) -> Result<(), AppError> {
    let lines: Vec<(i32, i32, i32, String, Value)> = sqlx::query_as(
        r#"
        SELECT id, item_id, qty, reference, details
        FROM bill_lines
        WHERE bill_id = $1 AND source = 'mobile.device'
        ORDER BY id
        "#,
    )
    .bind(bill_id)
    .fetch_all(tx.conn())
    .await?;
    if lines.is_empty() {
        return Ok(());
    }

    let timezone: String = sqlx::query_scalar("SELECT timezone FROM tenant_profile LIMIT 1")
        .fetch_optional(tx.conn())
        .await?
        .unwrap_or_else(|| "UTC".into());

    let mut seen = std::collections::HashSet::new();
    for (line_id, item_id, qty, reference, details) in lines {
        if qty != 1 {
            warn!(bill_id, line_id, qty, "mobile device line has quantity other than one");
            continue;
        }
        let Some(device_id) = device_id_from_line(&reference, &details) else {
            warn!(bill_id, line_id, "mobile device line has no device reference");
            continue;
        };
        if !seen.insert(device_id) {
            warn!(bill_id, line_id, device_id, "mobile device repeats on the same bill");
            continue;
        }

        let updated = sqlx::query(
            r#"
            UPDATE mobile_devices d
            SET status = 'sold',
                sold_bill_id = b.id,
                sold_bill_no = b.bill_no,
                sold_at = b.created_at,
                customer_name = COALESCE(b.customer, ''),
                updated_at = now()
            FROM bills b
            WHERE d.id = $1
              AND b.id = $2
              AND d.item_id = $3
              AND d.status = 'in_stock'
            "#,
        )
        .bind(device_id)
        .bind(bill_id)
        .bind(item_id)
        .execute(tx.conn())
        .await?;

        if updated.rows_affected() == 0 {
            let sold_here: Option<bool> = sqlx::query_scalar(
                "SELECT sold_bill_id = $1 FROM mobile_devices WHERE id = $2",
            )
            .bind(bill_id)
            .bind(device_id)
            .fetch_optional(tx.conn())
            .await?;
            if sold_here != Some(true) {
                warn!(bill_id, device_id, "mobile device is not available for this sale");
                continue;
            }
        }

        sqlx::query(
            r#"
            INSERT INTO mobile_warranties
                (device_id, bill_id, bill_no, months, sold_at, starts_on, ends_on, customer_name)
            SELECT d.id, b.id, b.bill_no, d.warranty_months, b.created_at,
                   (b.created_at AT TIME ZONE $3::text)::date,
                   ((b.created_at AT TIME ZONE $3::text)
                     + make_interval(months => d.warranty_months))::date,
                   COALESCE(b.customer, '')
            FROM mobile_devices d
            JOIN bills b ON b.id = $2
            WHERE d.id = $1 AND d.sold_bill_id = $2
            ON CONFLICT (tenant_id, bill_id, device_id) DO NOTHING
            "#,
        )
        .bind(device_id)
        .bind(bill_id)
        .bind(&timezone)
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
