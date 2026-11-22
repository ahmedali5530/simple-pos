CREATE TABLE IF NOT EXISTS catalog_attributes (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    name TEXT NOT NULL,
    UNIQUE (tenant_id, name),
    UNIQUE (tenant_id, id)
);

CREATE TABLE IF NOT EXISTS catalog_variant_attributes (
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    variant_id INT NOT NULL REFERENCES item_variants (id) ON DELETE CASCADE,
    attribute_id INT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (variant_id, attribute_id),
    FOREIGN KEY (tenant_id, attribute_id) REFERENCES catalog_attributes (tenant_id, id)
);

DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['catalog_attributes', 'catalog_variant_attributes'] LOOP
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
