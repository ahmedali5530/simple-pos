use platform_kernel::error::AppError;
use platform_kernel::migrate::Migration;
use platform_kernel::plugin::Plugin;
use sqlx::PgPool;
use std::sync::Arc;

/// Ensure the platform_migrations tracking table exists.
pub async fn ensure_table(pool: &PgPool) -> Result<(), AppError> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS platform_migrations (
            owner TEXT NOT NULL,
            version BIGINT NOT NULL,
            name TEXT NOT NULL,
            checksum TEXT NOT NULL,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            PRIMARY KEY (owner, version)
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn applied(pool: &PgPool, owner: &str) -> Result<Vec<i64>, AppError> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT version FROM platform_migrations WHERE owner = $1 ORDER BY version")
            .bind(owner)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(v,)| v).collect())
}

pub async fn run_owner(pool: &PgPool, owner: &str, migrations: &[Migration]) -> Result<(), AppError> {
    // Session locks are tied to one connection. Hold that connection until unlock
    // so a pooled checkout cannot leave the lock behind.
    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock(87201401)")
        .execute(&mut *conn)
        .await?;
    let result = run_owner_inner(pool, owner, migrations).await;
    sqlx::query("SELECT pg_advisory_unlock(87201401)")
        .execute(&mut *conn)
        .await
        .ok();
    result
}

async fn run_owner_inner(pool: &PgPool, owner: &str, migrations: &[Migration]) -> Result<(), AppError> {
    ensure_table(pool).await?;
    let done = applied(pool, owner).await?;
    for migration in migrations {
        if done.contains(&migration.version) {
            let checksum: Option<String> = sqlx::query_scalar(
                "SELECT checksum FROM platform_migrations WHERE owner = $1 AND version = $2",
            )
            .bind(owner)
            .bind(migration.version)
            .fetch_optional(pool)
            .await?;
            if checksum.as_deref() != Some(migration.checksum().as_str()) {
                return Err(AppError::BadRequest(format!(
                    "Migration {owner}/{} checksum mismatch. Frozen migrations must not change.",
                    migration.version
                )));
            }
            continue;
        }
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(migration.sql).execute(&mut *tx).await?;
        sqlx::query(
            r#"
            INSERT INTO platform_migrations (owner, version, name, checksum)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(owner)
        .bind(migration.version)
        .bind(migration.name)
        .bind(migration.checksum())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(owner, version = migration.version, name = migration.name, "applied migration");
    }
    Ok(())
}

pub async fn run_plugins(pool: &PgPool, plugins: &[Arc<dyn Plugin>]) -> Result<(), AppError> {
    for plugin in plugins {
        let manifest = plugin.manifest();
        run_owner(pool, manifest.id.as_str(), plugin.migrations()).await?;
    }
    Ok(())
}

pub mod core;
