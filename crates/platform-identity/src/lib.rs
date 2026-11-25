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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserView {
    pub id: i32,
    pub name: String,
    pub role: String,
    pub active: bool,
}

pub async fn list_users(pool: &PgPool, tenant_id: TenantId) -> Result<Vec<UserView>, AppError> {
    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), true).await?;
    let users = sqlx::query_as::<_, UserView>(
        "SELECT id, name, role, active FROM users ORDER BY lower(name), id",
    )
    .fetch_all(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(users)
}

#[derive(Debug, Deserialize)]
pub struct UserForm {
    pub name: String,
    #[serde(default)]
    pub password: Option<String>,
    pub role: String,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

pub async fn save_user(
    pool: &PgPool,
    tenant_id: TenantId,
    id: Option<i32>,
    form: &UserForm,
) -> Result<i32, AppError> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a name.".into()));
    }
    if name.chars().count() > 40 {
        return Err(AppError::BadRequest(
            "Keep the name to 40 characters.".into(),
        ));
    }
    if form.role != "cashier" && form.role != "manager" {
        return Err(AppError::BadRequest("Choose cashier or manager.".into()));
    }
    let password = form
        .password
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if id.is_none() && password.is_none() {
        return Err(AppError::BadRequest("Enter a password.".into()));
    }
    if password.is_some_and(|value| value.chars().count() < 4) {
        return Err(AppError::BadRequest(
            "Use at least 4 characters for the password.".into(),
        ));
    }

    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
    let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE lower(name) = lower($1) AND id IS DISTINCT FROM $2)")
        .bind(name).bind(id).fetch_one(tx.conn()).await?;
    if taken {
        return Err(AppError::BadRequest("That name is already used.".into()));
    }
    let other_managers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE role = 'manager' AND active AND id IS DISTINCT FROM $1",
    )
    .bind(id)
    .fetch_one(tx.conn())
    .await?;
    if other_managers == 0 && !(form.role == "manager" && form.active) {
        return Err(AppError::BadRequest("Keep at least one manager.".into()));
    }

    let user_id = if let Some(id) = id {
        let result = if let Some(password) = password {
            let hash = bcrypt::hash(password, 10)
                .map_err(|_| AppError::BadRequest("Could not save that password.".into()))?;
            sqlx::query(
                "UPDATE users SET name=$1, password_hash=$2, role=$3, active=$4 WHERE id=$5",
            )
            .bind(name)
            .bind(hash)
            .bind(&form.role)
            .bind(form.active)
            .bind(id)
            .execute(tx.conn())
            .await?
        } else {
            sqlx::query("UPDATE users SET name=$1, role=$2, active=$3 WHERE id=$4")
                .bind(name)
                .bind(&form.role)
                .bind(form.active)
                .bind(id)
                .execute(tx.conn())
                .await?
        };
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("That user is no longer saved.".into()));
        }
        id
    } else {
        let hash = bcrypt::hash(password.unwrap_or_default(), 10)
            .map_err(|_| AppError::BadRequest("Could not save that password.".into()))?;
        sqlx::query_scalar(
            "INSERT INTO users (name,password_hash,role,active) VALUES ($1,$2,$3,$4) RETURNING id",
        )
        .bind(name)
        .bind(hash)
        .bind(&form.role)
        .bind(form.active)
        .fetch_one(tx.conn())
        .await?
    };
    sync_role(&mut tx, tenant_id, user_id, &form.role).await?;
    tx.commit().await?;
    Ok(user_id)
}

async fn sync_role(
    tx: &mut platform_kernel::context::TenantTx<'_>,
    tenant_id: TenantId,
    user_id: i32,
    role_name: &str,
) -> Result<(), AppError> {
    let role_id: i32 = sqlx::query_scalar("INSERT INTO roles (tenant_id,name,is_system) VALUES ($1,$2,TRUE) ON CONFLICT (tenant_id,name) DO UPDATE SET is_system=TRUE RETURNING id")
        .bind(tenant_id.as_uuid()).bind(role_name).fetch_one(tx.conn()).await?;
    if role_name == "manager" {
        sqlx::query("INSERT INTO role_permissions (tenant_id,role_id,permission) SELECT $1,$2,name FROM platform_permissions ON CONFLICT DO NOTHING")
            .bind(tenant_id.as_uuid()).bind(role_id).execute(tx.conn()).await?;
    } else {
        sqlx::query("INSERT INTO role_permissions (tenant_id,role_id,permission) SELECT $1,$2,name FROM platform_permissions WHERE default_for_cashier ON CONFLICT DO NOTHING")
            .bind(tenant_id.as_uuid()).bind(role_id).execute(tx.conn()).await?;
    }
    sqlx::query("DELETE FROM user_roles WHERE tenant_id=$1 AND user_id=$2")
        .bind(tenant_id.as_uuid())
        .bind(user_id)
        .execute(tx.conn())
        .await?;
    sqlx::query("INSERT INTO user_roles (tenant_id,user_id,role_id) VALUES ($1,$2,$3)")
        .bind(tenant_id.as_uuid())
        .bind(user_id)
        .bind(role_id)
        .execute(tx.conn())
        .await?;
    Ok(())
}

pub async fn disable_user(
    pool: &PgPool,
    tenant_id: TenantId,
    user_id: i32,
) -> Result<(), AppError> {
    let mut tx = TenantTx::begin_with(pool, tenant_id, Actor::system(), false).await?;
    let user: Option<(String, bool)> = sqlx::query_as("SELECT role, active FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_optional(tx.conn())
        .await?;
    let Some((role, active)) = user else {
        return Err(AppError::NotFound("That user is no longer saved.".into()));
    };
    if role == "manager" && active {
        let others: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE role='manager' AND active AND id<>$1",
        )
        .bind(user_id)
        .fetch_one(tx.conn())
        .await?;
        if others == 0 {
            return Err(AppError::BadRequest("Keep at least one manager.".into()));
        }
    }
    sqlx::query("UPDATE users SET active=FALSE WHERE id=$1")
        .bind(user_id)
        .execute(tx.conn())
        .await?;
    sqlx::query("DELETE FROM sessions WHERE tenant_id=$1 AND user_id=$2")
        .bind(tenant_id.as_uuid())
        .bind(user_id)
        .execute(tx.conn())
        .await?;
    tx.commit().await?;
    Ok(())
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
