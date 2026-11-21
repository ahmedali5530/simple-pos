use std::sync::Arc;
use std::time::Duration;

use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_kernel::event::{chrono_lite, EventEnvelope, EventHandler};
use sqlx::PgPool;
use tokio::sync::watch;
use uuid::Uuid;

/// In-process outbox dispatcher. Peeks events as worker, handles inside TenantTx.
pub struct EventDispatcher {
    pool: PgPool,
    handlers: Vec<Arc<dyn EventHandler>>,
}

impl EventDispatcher {
    pub fn new(pool: PgPool, handlers: Vec<Arc<dyn EventHandler>>) -> Self {
        Self { pool, handlers }
    }

    pub fn spawn(self) -> watch::Sender<bool> {
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(err) = self.tick().await {
                            tracing::warn!(error = %err, "event dispatcher tick failed");
                        }
                    }
                    changed = rx.changed() => {
                        if changed.is_err() || *rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
        tx
    }

    async fn tick(&self) -> Result<(), AppError> {
        // Mark worker mode so worker_peek policy applies.
        let mut conn = self.pool.acquire().await?;
        sqlx::query("SELECT set_config('app.worker', '1', false)")
            .execute(&mut *conn)
            .await?;
        let rows: Vec<(Uuid, Uuid, String, i32, serde_json::Value, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(
                r#"
                SELECT e.id, e.tenant_id, e.event_type, e.version, e.payload, e.created_at
                FROM platform_events e
                ORDER BY e.created_at, e.id
                LIMIT 50
                "#,
            )
            .fetch_all(&mut *conn)
            .await?;
        sqlx::query("SELECT set_config('app.worker', '', false)")
            .execute(&mut *conn)
            .await?;
        drop(conn);

        for (id, tenant_uuid, event_type, version, payload, created_at) in rows {
            let tenant_id = TenantId(tenant_uuid);
            for handler in &self.handlers {
                if handler.event_type() != event_type {
                    continue;
                }
                let mut tx =
                    TenantTx::begin_with(&self.pool, tenant_id, Actor::system(), false).await?;
                let delivered: Option<(Uuid,)> = sqlx::query_as(
                    r#"
                    SELECT last_event_id FROM platform_event_deliveries
                    WHERE tenant_id = $1 AND subscriber_id = $2
                    "#,
                )
                .bind(tenant_uuid)
                .bind(handler.subscriber_id())
                .fetch_optional(tx.conn())
                .await?;
                if delivered.map(|(e,)| e) == Some(id) {
                    tx.commit().await?;
                    continue;
                }
                let envelope = EventEnvelope {
                    id,
                    tenant_id,
                    event_type: event_type.clone(),
                    version: version as u16,
                    payload: payload.clone(),
                    created_at: chrono_lite::DateTime(created_at.to_rfc3339()),
                };
                handler.handle(&mut tx, &envelope).await?;
                sqlx::query(
                    r#"
                    INSERT INTO platform_event_deliveries (tenant_id, subscriber_id, last_event_id, last_created_at)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (tenant_id, subscriber_id) DO UPDATE
                    SET last_event_id = EXCLUDED.last_event_id,
                        last_created_at = EXCLUDED.last_created_at
                    "#,
                )
                .bind(tenant_uuid)
                .bind(handler.subscriber_id())
                .bind(id)
                .bind(created_at)
                .execute(tx.conn())
                .await?;
                tx.commit().await?;
            }
        }
        Ok(())
    }
}
