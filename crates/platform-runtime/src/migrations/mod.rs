use platform_kernel::error::AppError;
use platform_kernel::migrate::Migration;
use platform_kernel::plugin::Plugin;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
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
    let pending: Vec<&Migration> = migrations
        .iter()
        .filter(|migration| !done.contains(&migration.version))
        .collect();
    if !pending.is_empty() {
        backup_plugin_tables(pool, owner, migrations).await?;
    }
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

/// Directory for JSON table dumps taken before plugin schema upgrades.
pub fn migration_backup_root() -> PathBuf {
    if let Ok(path) = std::env::var("CHASKA_MIGRATION_BACKUP_DIR") {
        return PathBuf::from(path);
    }
    PathBuf::from("data/migration-backups")
}

fn tables_named_in_sql(sql: &str) -> BTreeSet<String> {
    let mut tables = BTreeSet::new();
    let upper = sql.to_ascii_uppercase();
    for needle in ["CREATE TABLE", "ALTER TABLE"] {
        let mut from = 0usize;
        while let Some(found) = upper[from..].find(needle) {
            let start = from + found + needle.len();
            let rest = sql.get(start..).unwrap_or("").trim_start();
            let rest = if rest.len() >= 13 && rest[..13].eq_ignore_ascii_case("IF NOT EXISTS") {
                rest[13..].trim_start()
            } else {
                rest
            };
            if let Some(name) = rest
                .split(|c: char| c.is_whitespace() || c == '(' || c == ';')
                .next()
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                let name = name.trim_matches('"').trim_start_matches("public.");
                if name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    tables.insert(name.to_ascii_lowercase());
                }
            }
            from = start;
        }
    }
    tables
}

async fn table_exists(pool: &PgPool, table: &str) -> Result<bool, AppError> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = $1
        )
        "#,
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn backup_plugin_tables(
    pool: &PgPool,
    owner: &str,
    migrations: &[Migration],
) -> Result<(), AppError> {
    let mut tables = BTreeSet::new();
    for migration in migrations {
        tables.extend(tables_named_in_sql(migration.sql));
    }
    let mut existing = Vec::new();
    for table in tables {
        if table_exists(pool, &table).await? {
            existing.push(table);
        }
    }
    if existing.is_empty() {
        return Ok(());
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let dir = migration_backup_root().join(owner).join(&stamp);
    std::fs::create_dir_all(&dir).map_err(|err| {
        AppError::BadRequest(format!(
            "Could not create migration backup directory {}: {err}",
            dir.display()
        ))
    })?;
    for table in existing {
        dump_table_json(pool, &table, &dir.join(format!("{table}.json"))).await?;
    }
    tracing::info!(
        owner,
        path = %dir.display(),
        "backed up plugin tables before migration"
    );
    Ok(())
}

async fn dump_table_json(pool: &PgPool, table: &str, path: &Path) -> Result<(), AppError> {
    // Table names were filtered to [a-z0-9_]; still quote them.
    let sql = format!("SELECT row_to_json(t) FROM (SELECT * FROM \"{table}\") t");
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(&sql).fetch_all(pool).await?;
    let values: Vec<serde_json::Value> = rows.into_iter().map(|(value,)| value).collect();
    let text = serde_json::to_string_pretty(&values).map_err(|err| {
        AppError::BadRequest(format!("Could not serialize backup for {table}: {err}"))
    })?;
    std::fs::write(path, text + "\n").map_err(|err| {
        AppError::BadRequest(format!(
            "Could not write migration backup {}: {err}",
            path.display()
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod backup_tests {
    use super::*;

    #[test]
    fn extracts_create_and_alter_tables() {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS gift_cards (id int);
            ALTER TABLE gift_ledger ADD COLUMN note text;
            CREATE TABLE bill_tenders (id int);
        "#;
        let tables = tables_named_in_sql(sql);
        assert!(tables.contains("gift_cards"));
        assert!(tables.contains("gift_ledger"));
        assert!(tables.contains("bill_tenders"));
    }
}

pub mod core;
