use std::sync::Arc;
use std::time::Duration;

use std::collections::HashSet;

use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_kernel::event::{chrono_lite, EventEnvelope, EventHandler};
use platform_kernel::manifest::PluginId;
use sqlx::PgPool;
use tokio::sync::watch;
use uuid::Uuid;

use crate::plugins::EnabledPlugins;

/// In-process outbox dispatcher. Peels events per tenant and subscriber so each
/// handler advances its own cursor; failures retry on the next tick and are
/// dead-lettered after three attempts.
pub struct EventDispatcher {
    pool: PgPool,
    handlers: Vec<Arc<dyn EventHandler>>,
    enabled: Arc<EnabledPlugins>,
}

const BATCH: i64 = 200;
const MAX_ATTEMPTS: i32 = 3;

/// A handler with no owning plugin always runs. A handler whose plugin is disabled is skipped
/// and its delivery cursor is left where it is.
pub fn plugin_allows(owner: Option<&str>, enabled: &HashSet<PluginId>) -> bool {
    match owner {
        Some(owner) => PluginId::parse(owner)
            .map(|id| enabled.contains(&id))
            .unwrap_or(false),
        None => true,
    }
}

type Cursor = (chrono::DateTime<chrono::Utc>, Uuid);

impl EventDispatcher {
    pub fn new(pool: PgPool, handlers: Vec<Arc<dyn EventHandler>>, enabled: Arc<EnabledPlugins>) -> Self {
        Self {
            pool,
            handlers,
            enabled,
        }
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
        for tenant_uuid in self.tenant_ids().await? {
            let tenant_id = TenantId(tenant_uuid);
            for handler in &self.handlers {
                if !self.handler_enabled(tenant_id, handler).await? {
                    continue;
                }
                self.drain(tenant_id, handler.as_ref()).await?;
            }
        }
        Ok(())
    }

    /// Tenants with events. The worker policy on platform_events is the only
    /// cross-tenant read the dispatcher needs.
    async fn tenant_ids(&self) -> Result<Vec<Uuid>, AppError> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("SELECT set_config('app.worker', '1', false)")
            .execute(&mut *conn)
            .await?;
        let rows = sqlx::query_scalar("SELECT DISTINCT tenant_id FROM platform_events")
            .fetch_all(&mut *conn)
            .await;
        let reset = sqlx::query("SELECT set_config('app.worker', '', false)")
            .execute(&mut *conn)
            .await;
        reset?;
        Ok(rows?)
    }

    async fn handler_enabled(
        &self,
        tenant_id: TenantId,
        handler: &Arc<dyn EventHandler>,
    ) -> Result<bool, AppError> {
        let Some(owner) = handler.owner_plugin() else {
            return Ok(true);
        };
        let Ok(id) = PluginId::parse(owner) else {
            return Ok(false);
        };
        self.enabled.is_enabled(tenant_id, &id).await
    }

    /// Deliver every pending event for one handler, oldest first.
    async fn drain(&self, tenant_id: TenantId, handler: &dyn EventHandler) -> Result<(), AppError> {
        loop {
            let cursor = self.cursor(tenant_id, handler.subscriber_id()).await?;
            let mut read = TenantTx::begin_with(&self.pool, tenant_id, Actor::system(), true).await?;
            let rows: Vec<(Uuid, i32, serde_json::Value, chrono::DateTime<chrono::Utc>)> =
                sqlx::query_as(
                    r#"
                    SELECT id, version, payload, created_at
                    FROM platform_events
                    WHERE tenant_id = $1
                      AND event_type = $2
                      AND ($3::timestamptz IS NULL
                           OR (created_at, id) > ($3::timestamptz, $4::uuid))
                    ORDER BY created_at, id
                    LIMIT $5
                    "#,
                )
                .bind(tenant_id.as_uuid())
                .bind(handler.event_type())
                .bind(cursor.map(|(at, _)| at))
                .bind(cursor.map(|(_, id)| id))
                .bind(BATCH)
                .fetch_all(read.conn())
                .await?;
            read.commit().await?;
            if rows.is_empty() {
                return Ok(());
            }
            let fetched = rows.len() as i64;
            for (id, version, payload, created_at) in rows {
                let envelope = EventEnvelope {
                    id,
                    tenant_id,
                    event_type: handler.event_type().to_string(),
                    version: version as u16,
                    payload,
                    created_at: chrono_lite::DateTime(created_at.to_rfc3339()),
                };
                let mut tx =
                    TenantTx::begin_with(&self.pool, tenant_id, Actor::system(), false).await?;
                match handler.handle(&mut tx, &envelope).await {
                    Ok(()) => {
                        self.advance(tx.conn(), tenant_id, handler.subscriber_id(), id, created_at)
                            .await?;
                        tx.commit().await?;
                    }
                    Err(err) => {
                        let message = err.to_string();
                        tx.rollback().await.ok();
                        let mut fail_tx =
                            TenantTx::begin_with(&self.pool, tenant_id, Actor::system(), false)
                                .await?;
                        let attempts: i32 = sqlx::query_scalar(
                            r#"
                            INSERT INTO platform_event_failures (tenant_id, subscriber_id, event_id, error, attempts)
                            VALUES ($1, $2, $3, $4, 1)
                            ON CONFLICT (tenant_id, subscriber_id, event_id) DO UPDATE
                            SET error = EXCLUDED.error, attempts = platform_event_failures.attempts + 1, updated_at = now()
                            RETURNING attempts
                            "#,
                        )
                        .bind(tenant_id.as_uuid())
                        .bind(handler.subscriber_id())
                        .bind(id)
                        .bind(&message)
                        .fetch_one(fail_tx.conn())
                        .await?;
                        if attempts < MAX_ATTEMPTS {
                            fail_tx.commit().await?;
                            tracing::warn!(error = %message, subscriber = handler.subscriber_id(), "event handler will retry");
                            // Keep the cursor here and let the next tick retry;
                            // processing the rest of the batch now would spin.
                            return Ok(());
                        }
                        self.advance(
                            fail_tx.conn(),
                            tenant_id,
                            handler.subscriber_id(),
                            id,
                            created_at,
                        )
                        .await?;
                        fail_tx.commit().await?;
                        tracing::warn!(error = %message, subscriber = handler.subscriber_id(), "event handler moved to the dead letter");
                    }
                }
            }
            if fetched < BATCH {
                return Ok(());
            }
        }
    }

    async fn cursor(
        &self,
        tenant_id: TenantId,
        subscriber_id: &str,
    ) -> Result<Option<Cursor>, AppError> {
        let mut tx = TenantTx::begin_with(&self.pool, tenant_id, Actor::system(), true).await?;
        let row: Option<(Uuid, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
            r#"
            SELECT last_event_id, last_created_at
            FROM platform_event_deliveries
            WHERE tenant_id = $1 AND subscriber_id = $2
            "#,
        )
        .bind(tenant_id.as_uuid())
        .bind(subscriber_id)
        .fetch_optional(tx.conn())
        .await?;
        tx.commit().await?;
        Ok(row.and_then(|(id, at)| at.map(|at| (at, id))))
    }

    async fn advance(
        &self,
        conn: &mut sqlx::PgConnection,
        tenant_id: TenantId,
        subscriber_id: &str,
        id: Uuid,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO platform_event_deliveries (tenant_id, subscriber_id, last_event_id, last_created_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (tenant_id, subscriber_id) DO UPDATE
            SET last_event_id = EXCLUDED.last_event_id,
                last_created_at = EXCLUDED.last_created_at
            "#,
        )
        .bind(tenant_id.as_uuid())
        .bind(subscriber_id)
        .bind(id)
        .bind(created_at)
        .execute(conn)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_plugin_is_not_delivered_and_unowned_handlers_are() {
        let mut enabled = HashSet::new();
        enabled.insert(PluginId::must("inventory"));
        assert!(plugin_allows(None, &enabled));
        assert!(plugin_allows(Some("inventory"), &enabled));
        assert!(!plugin_allows(Some("catalog"), &enabled));
    }
}
