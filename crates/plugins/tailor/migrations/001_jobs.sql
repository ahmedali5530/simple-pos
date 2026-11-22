CREATE TABLE IF NOT EXISTS tailor_jobs (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    customer_name TEXT NOT NULL,
    garment TEXT NOT NULL,
    due_on TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT ''
);

ALTER TABLE tailor_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE tailor_jobs FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON tailor_jobs;
DROP POLICY IF EXISTS tenant_isolation_permissive ON tailor_jobs;
CREATE POLICY tenant_isolation_restrictive ON tailor_jobs AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON tailor_jobs AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
