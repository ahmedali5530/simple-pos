use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use platform_kernel::event::DomainEvent;
use serde_json::Value;
use uuid::Uuid;

pub struct Outbox;

impl Outbox {
    pub async fn publish<E: DomainEvent>(
        tx: &mut TenantTx<'_>,
        event: &E,
    ) -> Result<Uuid, AppError> {
        let id = Uuid::new_v4();
        let payload = serde_json::to_value(event)
            .map_err(|e| AppError::BadRequest(format!("Could not encode event: {e}")))?;
        sqlx::query(
            r#"
            INSERT INTO platform_events (id, tenant_id, event_type, version, payload)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(id)
        .bind(tx.tenant_id.as_uuid())
        .bind(E::TYPE)
        .bind(E::VERSION as i32)
        .bind(payload)
        .execute(tx.conn())
        .await?;
        Ok(id)
    }

    pub async fn publish_raw(
        tx: &mut TenantTx<'_>,
        event_type: &str,
        version: u16,
        payload: Value,
    ) -> Result<Uuid, AppError> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO platform_events (id, tenant_id, event_type, version, payload)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(id)
        .bind(tx.tenant_id.as_uuid())
        .bind(event_type)
        .bind(version as i32)
        .bind(payload)
        .execute(tx.conn())
        .await?;
        Ok(id)
    }
}
