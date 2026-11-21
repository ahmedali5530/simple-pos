//! Core identity: users, sessions, roles, API tokens.

use chrono::{Duration, Utc};
use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use platform_runtime::http::auth::hash_token;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub name: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginSession {
    pub id: i32,
    pub name: String,
    pub role: String,
    pub active: bool,
    pub token: String,
    pub tenant_id: uuid::Uuid,
}

#[derive(Debug, Serialize)]
pub struct UserView {
    pub id: i32,
    pub name: String,
    pub role: String,
    pub active: bool,
}

pub async fn needs_first_user(pool: &PgPool, tenant_id: TenantId) -> Result<bool, AppError> {
    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), true).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(tx.conn())
        .await?;
    tx.commit().await?;
    Ok(count == 0)
}

pub async fn login(
    pool: &PgPool,
    tenant_id: TenantId,
    form: &LoginForm,
) -> Result<LoginSession, AppError> {
    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
    let name = form.name.trim();
    let row: Option<(i32, String, String, String, bool)> = sqlx::query_as(
        "SELECT id, name, password_hash, role, active FROM users WHERE lower(name) = lower($1)",
    )
    .bind(name)
    .fetch_optional(tx.conn())
    .await?;
    let Some((id, stored_name, hash, role, active)) = row else {
        return Err(AppError::BadRequest(
            "That name or password does not match.".into(),
        ));
    };
    if !bcrypt::verify(form.password.as_str(), &hash).unwrap_or(false) {
        return Err(AppError::BadRequest(
            "That name or password does not match.".into(),
        ));
    }
    if !active {
        return Err(AppError::BadRequest("That account is turned off.".into()));
    }
    let raw = new_token()?;
    let token_hash = hash_token(&raw);
    let expires = Utc::now() + Duration::days(30);
    sqlx::query(
        r#"
        INSERT INTO sessions (token, token_hash, user_id, tenant_id, expires_at, last_seen_at)
        VALUES ($1, $2, $3, $4, $5, now())
        "#,
    )
    .bind(&raw)
    .bind(&token_hash)
    .bind(id)
    .bind(tenant_id.as_uuid())
    .bind(expires)
    .execute(tx.conn())
    .await?;
    sqlx::query(
        r#"
        INSERT INTO platform_audit_log (tenant_id, actor_id, action, detail)
        VALUES ($1, $2, 'auth.login', '{}'::jsonb)
        "#,
    )
    .bind(tenant_id.as_uuid())
    .bind(id)
    .execute(tx.conn())
    .await
    .ok();
    tx.commit().await?;
    Ok(LoginSession {
        id,
        name: stored_name,
        role,
        active,
        token: raw,
        tenant_id: tenant_id.as_uuid(),
    })
}

pub async fn logout(pool: &PgPool, tenant_id: TenantId, token: &str) -> Result<(), AppError> {
    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
    let hash = hash_token(token.trim());
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1 OR token = $2")
        .bind(&hash)
        .bind(token.trim())
        .execute(tx.conn())
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn create_api_token(
    pool: &PgPool,
    tenant_id: TenantId,
    actor: Actor,
    name: &str,
    permissions: &[String],
    allow_destructive: bool,
) -> Result<(String, i32), AppError> {
    if !actor.has_permission(platform_kernel::permission::builtins::PLATFORM_TOKENS) {
        return Err(AppError::Forbidden("You cannot manage API tokens.".into()));
    }
    let raw = new_token()?;
    let token_hash = hash_token(&raw);
    let mut tx = TenantTx::begin_with(pool, tenant_id, actor, false).await?;
    let id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO api_tokens (tenant_id, name, token_hash, permissions, allow_destructive)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(tenant_id.as_uuid())
    .bind(name.trim())
    .bind(&token_hash)
    .bind(permissions)
    .bind(allow_destructive)
    .fetch_one(tx.conn())
    .await?;
    tx.commit().await?;
    Ok((raw, id))
}

fn new_token() -> Result<String, AppError> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| AppError::BadRequest("Could not start the session.".into()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
