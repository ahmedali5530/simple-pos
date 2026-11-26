CREATE TABLE IF NOT EXISTS payment_entries (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    bill_id INT NOT NULL,
    bill_no TEXT NOT NULL DEFAULT '',
    method TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    amount INT NOT NULL CHECK (amount > 0),
    reference TEXT NOT NULL DEFAULT '',
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    posted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id)
);

CREATE INDEX IF NOT EXISTS payment_entries_posted_idx
    ON payment_entries (tenant_id, posted_at DESC);
CREATE INDEX IF NOT EXISTS payment_entries_method_idx
    ON payment_entries (tenant_id, method);

ALTER TABLE payment_entries ENABLE ROW LEVEL SECURITY;
ALTER TABLE payment_entries FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON payment_entries;
DROP POLICY IF EXISTS tenant_isolation_permissive ON payment_entries;
CREATE POLICY tenant_isolation_restrictive ON payment_entries AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON payment_entries AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
