CREATE TABLE IF NOT EXISTS mobile_models (
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    item_id INT NOT NULL,
    brand TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    warranty_months INT NOT NULL DEFAULT 12 CHECK (warranty_months BETWEEN 1 AND 120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, item_id),
    CONSTRAINT mobile_models_item_fk FOREIGN KEY (tenant_id, item_id)
        REFERENCES items (tenant_id, id) ON DELETE RESTRICT
);

ALTER TABLE mobile_models ENABLE ROW LEVEL SECURITY;
ALTER TABLE mobile_models FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON mobile_models;
DROP POLICY IF EXISTS tenant_isolation_permissive ON mobile_models;
CREATE POLICY tenant_isolation_restrictive ON mobile_models AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON mobile_models AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE TABLE IF NOT EXISTS mobile_devices (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    item_id INT NOT NULL,
    imei TEXT NOT NULL,
    serial TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'in_stock' CHECK (status IN ('in_stock', 'sold')),
    received_on DATE NOT NULL DEFAULT CURRENT_DATE,
    warranty_months INT NOT NULL DEFAULT 12 CHECK (warranty_months BETWEEN 1 AND 120),
    sold_bill_id INT,
    sold_bill_no TEXT NOT NULL DEFAULT '',
    sold_at TIMESTAMPTZ,
    customer_name TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, imei),
    CONSTRAINT mobile_devices_item_fk FOREIGN KEY (tenant_id, item_id)
        REFERENCES items (tenant_id, id) ON DELETE RESTRICT,
    CONSTRAINT mobile_devices_bill_fk FOREIGN KEY (tenant_id, sold_bill_id)
        REFERENCES bills (tenant_id, id)
);

CREATE INDEX IF NOT EXISTS mobile_devices_status_idx ON mobile_devices (tenant_id, status);

ALTER TABLE mobile_devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE mobile_devices FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON mobile_devices;
DROP POLICY IF EXISTS tenant_isolation_permissive ON mobile_devices;
CREATE POLICY tenant_isolation_restrictive ON mobile_devices AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON mobile_devices AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE TABLE IF NOT EXISTS mobile_warranties (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    device_id INT NOT NULL,
    bill_id INT NOT NULL,
    bill_no TEXT NOT NULL,
    months INT NOT NULL CHECK (months BETWEEN 1 AND 120),
    sold_at TIMESTAMPTZ NOT NULL,
    starts_on DATE NOT NULL,
    ends_on DATE NOT NULL,
    customer_name TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    CONSTRAINT mobile_warranties_bill_device_key UNIQUE (tenant_id, bill_id, device_id),
    CONSTRAINT mobile_warranties_device_fk FOREIGN KEY (tenant_id, device_id)
        REFERENCES mobile_devices (tenant_id, id) ON DELETE CASCADE,
    CONSTRAINT mobile_warranties_bill_fk FOREIGN KEY (tenant_id, bill_id)
        REFERENCES bills (tenant_id, id)
);

CREATE INDEX IF NOT EXISTS mobile_warranties_device_idx ON mobile_warranties (tenant_id, device_id);

ALTER TABLE mobile_warranties ENABLE ROW LEVEL SECURITY;
ALTER TABLE mobile_warranties FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON mobile_warranties;
DROP POLICY IF EXISTS tenant_isolation_permissive ON mobile_warranties;
CREATE POLICY tenant_isolation_restrictive ON mobile_warranties AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON mobile_warranties AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE TABLE IF NOT EXISTS mobile_claims (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid,
    device_id INT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'repair' CHECK (kind IN ('repair', 'replacement')),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'in_progress', 'resolved', 'rejected')),
    problem TEXT NOT NULL DEFAULT '',
    resolution TEXT NOT NULL DEFAULT '',
    cost BIGINT NOT NULL DEFAULT 0 CHECK (cost >= 0),
    opened_on DATE NOT NULL DEFAULT CURRENT_DATE,
    closed_on DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    CONSTRAINT mobile_claims_device_fk FOREIGN KEY (tenant_id, device_id)
        REFERENCES mobile_devices (tenant_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS mobile_claims_status_idx ON mobile_claims (tenant_id, status);
CREATE INDEX IF NOT EXISTS mobile_claims_device_idx ON mobile_claims (tenant_id, device_id);

ALTER TABLE mobile_claims ENABLE ROW LEVEL SECURITY;
ALTER TABLE mobile_claims FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON mobile_claims;
DROP POLICY IF EXISTS tenant_isolation_permissive ON mobile_claims;
CREATE POLICY tenant_isolation_restrictive ON mobile_claims AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON mobile_claims AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
