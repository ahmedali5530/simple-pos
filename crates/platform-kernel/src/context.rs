use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;

/// Stable tenant identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TenantId(pub Uuid);

impl TenantId {
    pub const DEFAULT: TenantId = TenantId(Uuid::from_bytes([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ]));

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Who is performing the action inside a tenant.
#[derive(Debug, Clone)]
pub struct Actor {
    pub user_id: Option<i32>,
    pub name: String,
    pub role: String,
    pub permissions: Vec<String>,
}

impl Actor {
    pub fn system() -> Self {
        Self {
            user_id: None,
            name: "system".into(),
            role: "system".into(),
            permissions: vec!["*".into()],
        }
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == "*" || p == permission)
    }
}

/// The only database handle plugins may hold.
///
/// Created by platform-runtime. Sets `app.tenant_id` with transaction-local
/// `set_config(..., true)` immediately after `BEGIN`. There is no public
/// constructor for plugins.
pub struct TenantTx<'a> {
    pub(crate) inner: Transaction<'a, Postgres>,
    pub tenant_id: TenantId,
    pub actor: Actor,
    pub read_only: bool,
}

impl<'a> TenantTx<'a> {
    /// Runtime-only constructor. Plugins must not call this.
    pub async fn begin_with(
        pool: &sqlx::PgPool,
        tenant_id: TenantId,
        actor: Actor,
        read_only: bool,
    ) -> Result<TenantTx<'a>, AppError> {
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant_id.to_string())
            .execute(&mut *tx)
            .await?;
        if let Some(user_id) = actor.user_id {
            sqlx::query("SELECT set_config('app.actor_id', $1, true)")
                .bind(user_id.to_string())
                .execute(&mut *tx)
                .await?;
        }
        let _ = read_only;
        Ok(TenantTx {
            inner: tx,
            tenant_id,
            actor,
            read_only,
        })
    }

    pub fn as_mut(&mut self) -> &mut Transaction<'a, Postgres> {
        &mut self.inner
    }

    pub fn transaction(&mut self) -> &mut Transaction<'a, Postgres> {
        &mut self.inner
    }

    /// Borrow the underlying Postgres connection for sqlx queries.
    pub fn conn(&mut self) -> &mut sqlx::PgConnection {
        &mut *self.inner
    }

    pub async fn commit(self) -> Result<(), AppError> {
        self.inner.commit().await?;
        Ok(())
    }

    pub async fn rollback(self) -> Result<(), AppError> {
        self.inner.rollback().await?;
        Ok(())
    }
}

impl<'a> std::ops::Deref for TenantTx<'a> {
    type Target = Transaction<'a, Postgres>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> std::ops::DerefMut for TenantTx<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
