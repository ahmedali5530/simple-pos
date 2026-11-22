CREATE TABLE IF NOT EXISTS installment_agreements (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    customer_name TEXT NOT NULL,
    total_amount BIGINT NOT NULL,
    paid_amount BIGINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'open'
);

ALTER TABLE installment_agreements ENABLE ROW LEVEL SECURITY;
ALTER TABLE installment_agreements FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON installment_agreements;
DROP POLICY IF EXISTS tenant_isolation_permissive ON installment_agreements;
CREATE POLICY tenant_isolation_restrictive ON installment_agreements AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON installment_agreements AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
