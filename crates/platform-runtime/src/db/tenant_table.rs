//! Helpers that emit SQL for tenant-owned tables with RLS.

/// SQL fragment: add tenant_id column with current_setting default.
pub fn add_tenant_id_column(table: &str) -> String {
    format!(
        r#"
ALTER TABLE {table}
  ADD COLUMN IF NOT EXISTS tenant_id uuid;
UPDATE {table}
SET tenant_id = '00000000-0000-4000-8000-000000000001'
WHERE tenant_id IS NULL;
ALTER TABLE {table}
  ALTER COLUMN tenant_id SET DEFAULT current_setting('app.tenant_id')::uuid,
  ALTER COLUMN tenant_id SET NOT NULL;
"#
    )
}

/// ENABLE + FORCE RLS with restrictive and permissive policies for platform_app.
pub fn enable_rls(table: &str) -> String {
    format!(
        r#"
ALTER TABLE {table} ENABLE ROW LEVEL SECURITY;
ALTER TABLE {table} FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON {table};
DROP POLICY IF EXISTS tenant_isolation_permissive ON {table};
CREATE POLICY tenant_isolation_restrictive ON {table}
  AS RESTRICTIVE FOR ALL TO platform_app
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON {table}
  AS PERMISSIVE FOR ALL TO platform_app
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
-- During transition, also allow the database owner (tests / single-URL mode).
DO $$
BEGIN
  IF current_user <> 'platform_app' THEN
    EXECUTE format(
      'CREATE POLICY IF NOT EXISTS tenant_isolation_owner ON %I AS PERMISSIVE FOR ALL TO PUBLIC USING (true) WITH CHECK (true)',
      '{table}'
    );
  END IF;
EXCEPTION WHEN duplicate_object THEN
  NULL;
END $$;
"#
    )
}

/// Unique (tenant_id, id) on a parent table so children can use composite FKs.
pub fn unique_tenant_id(table: &str) -> String {
    format!(
        r#"
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = '{table}_tenant_id_id_key'
  ) THEN
    ALTER TABLE {table} ADD CONSTRAINT {table}_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
END $$;
"#
    )
}

/// Full helper for a brand-new plugin table body (columns without tenant_id/id).
pub fn create_tenant_table(table: &str, body_sql: &str) -> String {
    format!(
        r#"
CREATE TABLE IF NOT EXISTS {table} (
  id SERIAL PRIMARY KEY,
  tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants(id),
  {body_sql}
);
{unique}
{rls}
"#,
        unique = unique_tenant_id(table),
        rls = enable_rls(table),
    )
}
