use platform_kernel::error::AppError;
use sqlx::PgPool;

const GLOBAL_ALLOWLIST: &[&str] = &[
    "_sqlx_migrations",
    "platform_migrations",
    "platform_permissions",
];

const CORE_SECURITY_DEFINER: &[&str] = &[
    "platform_resolve_session",
    "platform_resolve_tenant",
    "platform_provision_tenant",
];

/// Fail if any public table outside the allowlist lacks proper RLS.
pub async fn validate(pool: &PgPool) -> Result<(), AppError> {
    let tables: Vec<(String, bool, bool)> = sqlx::query_as(
        r#"
        SELECT c.relname, c.relrowsecurity, c.relforcerowsecurity
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relkind = 'r'
        ORDER BY c.relname
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Only enforce once tenants table exists (post-M2).
    let has_tenants = tables.iter().any(|(name, _, _)| name == "tenants");
    if !has_tenants {
        return Ok(());
    }

    let mut errors = Vec::new();
    for (name, rls, force) in &tables {
        if GLOBAL_ALLOWLIST.contains(&name.as_str()) {
            continue;
        }
        if !rls || !force {
            errors.push(format!(
                "table `{name}` must ENABLE and FORCE ROW LEVEL SECURITY"
            ));
        }

        let has_tenant: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM information_schema.columns
              WHERE table_schema = 'public'
                AND table_name = $1
                AND column_name = 'tenant_id'
                AND is_nullable = 'NO'
            )
            "#,
        )
        .bind(name)
        .fetch_one(pool)
        .await?;
        if name != "tenants" && !has_tenant {
            errors.push(format!("table `{name}` needs NOT NULL tenant_id"));
        }

        let policies: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM pg_policy p
            JOIN pg_class c ON c.oid = p.polrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = 'public' AND c.relname = $1
            "#,
        )
        .bind(name)
        .fetch_one(pool)
        .await?;
        if policies == 0 {
            errors.push(format!("table `{name}` has no RLS policies"));
        }
    }

    // SECURITY DEFINER functions must be on the allowlist.
    let funcs: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT p.proname
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = 'public'
          AND p.prosecdef
        "#,
    )
    .fetch_all(pool)
    .await?;
    for (name,) in funcs {
        if !CORE_SECURITY_DEFINER.contains(&name.as_str()) {
            errors.push(format!(
                "SECURITY DEFINER function `{name}` is not on the core allowlist"
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "RLS validation failed:\n- {}",
            errors.join("\n- ")
        )))
    }
}

pub fn global_allowlist() -> &'static [&'static str] {
    GLOBAL_ALLOWLIST
}
