CREATE TABLE IF NOT EXISTS customers (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    name TEXT NOT NULL,
    phone TEXT NOT NULL DEFAULT ''
);

ALTER TABLE customers ENABLE ROW LEVEL SECURITY;
ALTER TABLE customers FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON customers;
DROP POLICY IF EXISTS tenant_isolation_permissive ON customers;
CREATE POLICY tenant_isolation_restrictive ON customers AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON customers AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
