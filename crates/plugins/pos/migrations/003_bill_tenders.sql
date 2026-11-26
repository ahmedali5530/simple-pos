DO $$
BEGIN
  IF to_regclass('public.bills') IS NULL THEN
    RETURN;
  END IF;

  CREATE TABLE IF NOT EXISTS bill_tenders (
      id SERIAL PRIMARY KEY,
      tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
      bill_id INT NOT NULL,
      method TEXT NOT NULL,
      amount INT NOT NULL CHECK (amount > 0),
      reference TEXT NOT NULL DEFAULT '',
      details JSONB NOT NULL DEFAULT '{}'::jsonb,
      UNIQUE (tenant_id, id),
      CONSTRAINT bill_tenders_bill_fk FOREIGN KEY (tenant_id, bill_id)
          REFERENCES bills (tenant_id, id) ON DELETE CASCADE
  );

  CREATE INDEX IF NOT EXISTS bill_tenders_bill_idx ON bill_tenders (tenant_id, bill_id);

  ALTER TABLE bill_tenders ENABLE ROW LEVEL SECURITY;
  ALTER TABLE bill_tenders FORCE ROW LEVEL SECURITY;
  DROP POLICY IF EXISTS tenant_isolation_restrictive ON bill_tenders;
  DROP POLICY IF EXISTS tenant_isolation_permissive ON bill_tenders;
  CREATE POLICY tenant_isolation_restrictive ON bill_tenders AS RESTRICTIVE FOR ALL TO PUBLIC
    USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
  CREATE POLICY tenant_isolation_permissive ON bill_tenders AS PERMISSIVE FOR ALL TO PUBLIC
    USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
    WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
END $$;
