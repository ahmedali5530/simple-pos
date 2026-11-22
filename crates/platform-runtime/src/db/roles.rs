use platform_kernel::error::AppError;
use sqlx::PgPool;

const APP_ROLE: &str = "platform_app";
const OWNER_ROLE: &str = "platform_owner";
const WORKER_ROLE: &str = "platform_worker";

/// Provision roles from an admin/superuser connection.
pub async fn provision(admin: &PgPool, app_password: &str) -> Result<(), AppError> {
    let password = app_password.replace('\'', "''");
    sqlx::query(&format!(
        r#"
        DO $$
        BEGIN
          IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{OWNER_ROLE}') THEN
            CREATE ROLE {OWNER_ROLE} NOLOGIN;
          END IF;
          IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{APP_ROLE}') THEN
            CREATE ROLE {APP_ROLE} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEROLE PASSWORD '{password}';
          ELSE
            ALTER ROLE {APP_ROLE} WITH LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEROLE PASSWORD '{password}';
          END IF;
          IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{WORKER_ROLE}') THEN
            CREATE ROLE {WORKER_ROLE} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEROLE PASSWORD '{password}';
          END IF;
        END $$;
        "#
    ))
    .execute(admin)
    .await?;

    sqlx::query(&format!(
        "GRANT USAGE ON SCHEMA public TO {APP_ROLE}, {WORKER_ROLE}"
    ))
    .execute(admin)
    .await?;

    // One statement per query — sqlx does not run multi-statement strings reliably.
    sqlx::query(&format!(
        "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {APP_ROLE}"
    ))
    .execute(admin)
    .await?;
    sqlx::query(&format!(
        "GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO {APP_ROLE}"
    ))
    .execute(admin)
    .await?;
    sqlx::query(&format!(
        "GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO {APP_ROLE}"
    ))
    .execute(admin)
    .await?;

    Ok(())
}

/// Grant the app role access to tables created after the first provision.
pub async fn grant_existing(admin: &PgPool) -> Result<(), AppError> {
    sqlx::query(&format!(
        "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {APP_ROLE}"
    ))
    .execute(admin)
    .await?;
    sqlx::query(&format!(
        "GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO {APP_ROLE}"
    ))
    .execute(admin)
    .await?;
    Ok(())
}

/// Refuse to start if the runtime role can bypass RLS.
pub async fn startup_guard(runtime: &PgPool) -> Result<(), AppError> {
    let row: Option<(bool, bool, String)> = sqlx::query_as(
        r#"
        SELECT r.rolsuper, r.rolbypassrls, r.rolname
        FROM pg_roles r
        WHERE r.rolname = current_user
        "#,
    )
    .fetch_optional(runtime)
    .await?;

    let Some((is_super, bypass, name)) = row else {
        return Err(AppError::BadRequest(
            "Could not verify the database role.".into(),
        ));
    };

    let app_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname = 'platform_app')")
            .fetch_one(runtime)
            .await
            .unwrap_or(false);

    if app_exists && name == APP_ROLE && (is_super || bypass) {
        return Err(AppError::BadRequest(format!(
            "Runtime role `{name}` must not be superuser or BYPASSRLS."
        )));
    }

    if name == APP_ROLE {
        let owned: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            JOIN pg_roles r ON r.oid = c.relowner
            WHERE n.nspname = 'public'
              AND c.relkind = 'r'
              AND r.rolname = current_user
            "#,
        )
        .fetch_one(runtime)
        .await?;
        if owned > 0 {
            return Err(AppError::BadRequest(
                "Runtime role must not own tenant tables.".into(),
            ));
        }
    }

    let _ = (is_super, bypass, app_exists);
    Ok(())
}

pub fn app_role() -> &'static str {
    APP_ROLE
}

pub fn owner_role() -> &'static str {
    OWNER_ROLE
}

pub fn worker_role() -> &'static str {
    WORKER_ROLE
}
