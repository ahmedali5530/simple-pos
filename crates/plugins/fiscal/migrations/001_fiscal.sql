CREATE TABLE IF NOT EXISTS fiscal_settings (
    tenant_id uuid PRIMARY KEY DEFAULT current_setting('app.tenant_id')::uuid,
    authority TEXT NOT NULL DEFAULT 'fbr' CHECK (authority IN ('fbr', 'pra')),
    mode TEXT NOT NULL DEFAULT 'sandbox' CHECK (mode IN ('sandbox', 'live')),
    ntn TEXT NOT NULL DEFAULT '',
    strn TEXT NOT NULL DEFAULT '',
    pos_id TEXT NOT NULL DEFAULT '',
    api_token TEXT NOT NULL DEFAULT '',
    api_base_url TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE fiscal_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE fiscal_settings FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON fiscal_settings;
DROP POLICY IF EXISTS tenant_isolation_permissive ON fiscal_settings;
CREATE POLICY tenant_isolation_restrictive ON fiscal_settings AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON fiscal_settings AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE TABLE IF NOT EXISTS fiscal_submissions (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    bill_id INT NOT NULL,
    bill_no TEXT NOT NULL,
    authority TEXT NOT NULL,
    invoice_no TEXT NOT NULL DEFAULT '',
    qr_payload TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('ok', 'failed')),
    error_message TEXT NOT NULL DEFAULT '',
    raw_response JSONB NOT NULL DEFAULT '{}'::jsonb,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, bill_no)
);

CREATE INDEX IF NOT EXISTS fiscal_submissions_submitted_idx
    ON fiscal_submissions (tenant_id, submitted_at DESC);
CREATE INDEX IF NOT EXISTS fiscal_submissions_status_idx
    ON fiscal_submissions (tenant_id, status);

ALTER TABLE fiscal_submissions ENABLE ROW LEVEL SECURITY;
ALTER TABLE fiscal_submissions FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON fiscal_submissions;
DROP POLICY IF EXISTS tenant_isolation_permissive ON fiscal_submissions;
CREATE POLICY tenant_isolation_restrictive ON fiscal_submissions AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON fiscal_submissions AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
