use platform_kernel::context::{Actor, TenantId};
use platform_kernel::error::AppError;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub tenant_id: TenantId,
    pub user_id: i32,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
}

impl AuthUser {
    pub fn actor(&self) -> Actor {
        Actor {
            user_id: Some(self.user_id),
            name: self.name.clone(),
            role: self.role.clone(),
            permissions: self.permissions.clone(),
        }
    }
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Resolve a bearer/cookie token via the SECURITY DEFINER function, then load the user inside a tenant tx.
pub async fn resolve_session(pool: &PgPool, token: &str) -> Result<AuthUser, AppError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::Unauthorized("Sign in before continuing.".into()));
    }
    let hash = hash_token(token);
    // Try hash first, then raw token for pre-migration sessions.
    let row: Option<(uuid::Uuid, i32, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as("SELECT tenant_id, user_id, expires_at FROM platform_resolve_session($1)")
            .bind(&hash)
            .fetch_optional(pool)
            .await?;
    let (tenant_uuid, user_id, expires_at) = match row {
        Some(r) => r,
        None => {
            let fallback: Option<(uuid::Uuid, i32, Option<chrono::DateTime<chrono::Utc>>)> =
                sqlx::query_as("SELECT tenant_id, user_id, expires_at FROM platform_resolve_session($1)")
                    .bind(token)
                    .fetch_optional(pool)
                    .await?;
            fallback.ok_or_else(|| AppError::Unauthorized("Sign in before continuing.".into()))?
        }
    };
    if let Some(expires) = expires_at {
        if expires < chrono::Utc::now() {
            return Err(AppError::Unauthorized("Session expired. Sign in again.".into()));
        }
    }
    let tenant_id = TenantId(tenant_uuid);
    let mut tx = platform_kernel::context::TenantTx::begin_with(
        pool,
        tenant_id,
        Actor::system(),
        true,
    )
    .await?;
    let user: Option<(i32, String, String, bool)> = sqlx::query_as(
        "SELECT id, name, role, active FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(tx.conn())
    .await?;
    let permissions: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT rp.permission
        FROM user_roles ur
        JOIN role_permissions rp ON rp.tenant_id = ur.tenant_id AND rp.role_id = ur.role_id
        WHERE ur.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(tx.conn())
    .await
    .unwrap_or_default();
    tx.commit().await?;

    let Some((id, name, role, active)) = user else {
        return Err(AppError::Unauthorized("Sign in before continuing.".into()));
    };
    if !active {
        return Err(AppError::Unauthorized("That account is turned off.".into()));
    }
    let permissions = if permissions.is_empty() {
        // Compatibility before role rows exist: grant all.
        vec!["*".into()]
    } else {
        permissions
    };
    Ok(AuthUser {
        tenant_id,
        user_id: id,
        name,
        role,
        permissions,
    })
}
