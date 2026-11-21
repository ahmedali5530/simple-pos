use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

use platform_kernel::error::AppError;

use super::roles;

/// Separate migration (owner/admin) and runtime (platform_app) pools.
#[derive(Clone)]
pub struct Pools {
    pub migration: PgPool,
    pub runtime: PgPool,
}

impl Pools {
    pub async fn connect(admin_url: &str, app_url: &str) -> Result<Self, AppError> {
        let migration = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(15))
            .connect(admin_url)
            .await?;

        let runtime_opts = PgConnectOptions::from_str(app_url)
            .map_err(|e| AppError::BadRequest(format!("DATABASE_URL is invalid: {e}")))?;

        let runtime = PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(15))
            .after_connect(|conn, _| {
                Box::pin(async move {
                    // Fail closed: refuse connections that still carry a tenant setting.
                    #[cfg(any(test, debug_assertions))]
                    {
                        let current: Option<String> = sqlx::query_scalar(
                            "SELECT nullif(current_setting('app.tenant_id', true), '')",
                        )
                        .fetch_one(&mut *conn)
                        .await?;
                        if current.is_some() {
                            return Err(sqlx::Error::Protocol(
                                "pooled connection leaked app.tenant_id".into(),
                            ));
                        }
                    }
                    let _ = conn;
                    Ok(())
                })
            })
            .connect_with(runtime_opts)
            .await?;

        Ok(Self { migration, runtime })
    }

    /// Single-URL mode used before roles are provisioned (M0/M1 and fresh installs).
    pub async fn connect_shared(url: &str) -> Result<Self, AppError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(15))
            .connect(url)
            .await?;
        Ok(Self {
            migration: pool.clone(),
            runtime: pool,
        })
    }

    pub async fn guard_runtime(&self) -> Result<(), AppError> {
        roles::startup_guard(&self.runtime).await
    }
}
