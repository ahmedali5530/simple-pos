ALTER TABLE tailor_jobs
    ADD COLUMN IF NOT EXISTS phone TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS measurements JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS cloth_type TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS cloth_color TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS cloth_quantity TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS cloth_source TEXT NOT NULL DEFAULT 'customer',
    ADD COLUMN IF NOT EXISTS cloth_notes TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS sewing_details TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS total_amount BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'received',
    ADD COLUMN IF NOT EXISTS delivery_status TEXT NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS delivery_date DATE,
    ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE tailor_jobs DROP CONSTRAINT IF EXISTS tailor_jobs_status_check;
ALTER TABLE tailor_jobs
    ADD CONSTRAINT tailor_jobs_status_check
    CHECK (status IN ('received', 'cutting', 'sewing', 'finishing', 'ready', 'cancelled'));

ALTER TABLE tailor_jobs DROP CONSTRAINT IF EXISTS tailor_jobs_delivery_status_check;
ALTER TABLE tailor_jobs
    ADD CONSTRAINT tailor_jobs_delivery_status_check
    CHECK (delivery_status IN ('pending', 'ready', 'out', 'delivered'));

ALTER TABLE tailor_jobs DROP CONSTRAINT IF EXISTS tailor_jobs_cloth_source_check;
ALTER TABLE tailor_jobs
    ADD CONSTRAINT tailor_jobs_cloth_source_check
    CHECK (cloth_source IN ('customer', 'shop'));

CREATE TABLE IF NOT EXISTS tailor_payments (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    job_id INT NOT NULL REFERENCES tailor_jobs (id),
    amount BIGINT NOT NULL CHECK (amount > 0),
    method TEXT NOT NULL CHECK (method IN ('cash', 'card', 'transfer', 'credit')),
    note TEXT NOT NULL DEFAULT '',
    paid_on DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS tailor_payments_job_idx ON tailor_payments (job_id);

ALTER TABLE tailor_payments ENABLE ROW LEVEL SECURITY;
ALTER TABLE tailor_payments FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON tailor_payments;
DROP POLICY IF EXISTS tenant_isolation_permissive ON tailor_payments;
CREATE POLICY tenant_isolation_restrictive ON tailor_payments AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON tailor_payments AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
