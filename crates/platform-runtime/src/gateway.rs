use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityRegistry};
use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use serde_json::Value;
use sqlx::PgPool;

use crate::http::auth::AuthUser;

pub struct CapabilityGateway {
    pub registry: CapabilityRegistry,
    pool: PgPool,
}

impl CapabilityGateway {
    pub fn new(pool: PgPool, registry: CapabilityRegistry) -> Self {
        Self { pool, registry }
    }

    pub fn list_for(&self, user: &AuthUser, write_allowed: bool) -> Vec<&Capability> {
        self.registry
            .list()
            .into_iter()
            .filter(|cap| user.actor().has_permission(&cap.permission))
            .filter(|cap| {
                write_allowed
                    || matches!(cap.effect, CapabilityEffect::Read)
            })
            .collect()
    }

    pub async fn invoke(
        &self,
        user: &AuthUser,
        capability_id: &str,
        input: Value,
        idempotency_key: Option<&str>,
        allow_destructive: bool,
    ) -> Result<Value, AppError> {
        let (spec, handler) = self
            .registry
            .get(capability_id)
            .ok_or_else(|| AppError::NotFound(format!("Unknown capability `{capability_id}`.")))?;

        if !user.actor().has_permission(&spec.permission) {
            return Err(AppError::Forbidden(
                "You do not have permission for that capability.".into(),
            ));
        }
        if matches!(spec.effect, CapabilityEffect::Destructive) && !allow_destructive {
            return Err(AppError::Forbidden(
                "Destructive capabilities are disabled for this token.".into(),
            ));
        }
        if spec.requires_confirmation
            && input.get("confirm").and_then(|v| v.as_bool()) != Some(true)
        {
            return Err(AppError::BadRequest(
                "This capability requires confirm: true.".into(),
            ));
        }

        let mut tx =
            TenantTx::begin_with(&self.pool, user.tenant_id, user.actor(), false).await?;

        if let Some(key) = idempotency_key {
            let existing: Option<(Value,)> = sqlx::query_as(
                "SELECT response FROM platform_idempotency_keys WHERE tenant_id = $1 AND key = $2",
            )
            .bind(user.tenant_id.as_uuid())
            .bind(key)
            .fetch_optional(tx.conn())
            .await?;
            if let Some((response,)) = existing {
                tx.commit().await?;
                return Ok(response);
            }
        }

        let output = handler.invoke(&mut tx, input).await?;

        sqlx::query(
            r#"
            INSERT INTO platform_audit_log (tenant_id, actor_id, action, detail)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(user.tenant_id.as_uuid())
        .bind(user.user_id)
        .bind(format!("capability:{capability_id}"))
        .bind(serde_json::json!({ "ok": true }))
        .execute(tx.conn())
        .await?;

        if let Some(key) = idempotency_key {
            sqlx::query(
                r#"
                INSERT INTO platform_idempotency_keys (tenant_id, key, response)
                VALUES ($1, $2, $3)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(user.tenant_id.as_uuid())
            .bind(key)
            .bind(&output)
            .execute(tx.conn())
            .await?;
        }

        tx.commit().await?;
        Ok(output)
    }

    pub async fn invoke_as_tenant(
        &self,
        tenant_id: TenantId,
        actor: Actor,
        capability_id: &str,
        input: Value,
    ) -> Result<Value, AppError> {
        let user = AuthUser {
            tenant_id,
            user_id: actor.user_id.unwrap_or(0),
            name: actor.name.clone(),
            role: actor.role.clone(),
            permissions: actor.permissions.clone(),
        };
        self.invoke(&user, capability_id, input, None, true).await
    }
}
