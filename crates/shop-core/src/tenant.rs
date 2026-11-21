//! Transaction-local tenant context for RLS.
//! Every shop-core entry point must run inside `begin()`.

use sqlx::{Postgres, Transaction};

use crate::error::AppError;

/// Default tenant UUID used for single-shop and desktop installs.
pub const DEFAULT_TENANT: &str = "00000000-0000-4000-8000-000000000001";

/// Begin a transaction and set `app.tenant_id` with transaction-local scope.
pub async fn begin(pool: &sqlx::PgPool) -> Result<Transaction<'_, Postgres>, AppError> {
    begin_for(pool, DEFAULT_TENANT).await
}

pub async fn begin_for<'a>(
    pool: &'a sqlx::PgPool,
    tenant_id: &str,
) -> Result<Transaction<'a, Postgres>, AppError> {
    let mut tx = pool.begin().await?;
    // If tenants/RLS are not installed yet, set_config still succeeds.
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// True when the `tenants` table exists (platform migrations applied).
pub async fn platform_ready(pool: &sqlx::PgPool) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM information_schema.tables
          WHERE table_schema = 'public' AND table_name = 'tenants'
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}
