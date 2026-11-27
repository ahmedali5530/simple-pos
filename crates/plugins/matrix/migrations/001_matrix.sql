CREATE TABLE IF NOT EXISTS matrix_items (
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    item_id INT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    barcode_prefix TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, item_id),
    UNIQUE (tenant_id, item_id)
);

CREATE TABLE IF NOT EXISTS matrix_groups (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    item_id INT NOT NULL,
    name TEXT NOT NULL,
    sort_order INT NOT NULL DEFAULT 0,
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, item_id, name),
    FOREIGN KEY (tenant_id, item_id) REFERENCES matrix_items (tenant_id, item_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS matrix_group_values (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    group_id INT NOT NULL,
    value TEXT NOT NULL,
    sort_order INT NOT NULL DEFAULT 0,
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, group_id, value),
    FOREIGN KEY (tenant_id, group_id) REFERENCES matrix_groups (tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS matrix_skus (
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    variant_id INT NOT NULL REFERENCES item_variants (id) ON DELETE CASCADE,
    item_id INT NOT NULL,
    fingerprint TEXT NOT NULL,
    stock INT NOT NULL DEFAULT 0 CHECK (stock >= 0),
    avg_cost BIGINT NOT NULL DEFAULT 0 CHECK (avg_cost >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, variant_id),
    UNIQUE (tenant_id, item_id, fingerprint),
    FOREIGN KEY (tenant_id, item_id) REFERENCES matrix_items (tenant_id, item_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS matrix_skus_item_idx ON matrix_skus (tenant_id, item_id);

CREATE TABLE IF NOT EXISTS matrix_sku_movements (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    variant_id INT NOT NULL,
    item_id INT NOT NULL,
    qty INT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, variant_id) REFERENCES matrix_skus (tenant_id, variant_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS matrix_sku_movements_variant_idx
    ON matrix_sku_movements (tenant_id, variant_id, created_at DESC);

DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY[
    'matrix_items',
    'matrix_groups',
    'matrix_group_values',
    'matrix_skus',
    'matrix_sku_movements'
  ] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_restrictive ON %I', t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_permissive ON %I', t);
    EXECUTE format(
      'CREATE POLICY tenant_isolation_restrictive ON %I AS RESTRICTIVE FOR ALL TO PUBLIC
       USING (tenant_id = current_setting(''app.tenant_id'', true)::uuid)
       WITH CHECK (tenant_id = current_setting(''app.tenant_id'', true)::uuid)', t);
    EXECUTE format(
      'CREATE POLICY tenant_isolation_permissive ON %I AS PERMISSIVE FOR ALL TO PUBLIC
       USING (tenant_id = current_setting(''app.tenant_id'', true)::uuid)
       WITH CHECK (tenant_id = current_setting(''app.tenant_id'', true)::uuid)', t);
  END LOOP;
END $$;
