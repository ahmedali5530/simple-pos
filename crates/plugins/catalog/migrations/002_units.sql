CREATE TABLE IF NOT EXISTS catalog_units (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    base_code TEXT NOT NULL,
    factor INT NOT NULL DEFAULT 1 CHECK (factor > 0),
    sort_order INT NOT NULL DEFAULT 0,
    UNIQUE (tenant_id, code),
    UNIQUE (tenant_id, id)
);

ALTER TABLE items ADD COLUMN IF NOT EXISTS unit_id INT;
ALTER TABLE item_variants ADD COLUMN IF NOT EXISTS unit_id INT;

DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY['catalog_units'] LOOP
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

CREATE OR REPLACE FUNCTION catalog_seed_units(p_tenant uuid)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
  INSERT INTO catalog_units (tenant_id, code, name, base_code, factor, sort_order)
  VALUES
    (p_tenant, 'pc', 'Piece', 'pc', 1, 0),
    (p_tenant, 'pack', 'Pack', 'pc', 1, 1),
    (p_tenant, 'kg', 'Kilogram', 'g', 1000, 2),
    (p_tenant, 'g', 'Gram', 'g', 1, 3),
    (p_tenant, 'l', 'Litre', 'ml', 1000, 4),
    (p_tenant, 'ml', 'Millilitre', 'ml', 1, 5),
    (p_tenant, 'plate', 'Plate', 'plate', 1, 6),
    (p_tenant, 'portion', 'Portion', 'portion', 1, 7)
  ON CONFLICT (tenant_id, code) DO NOTHING;

  UPDATE items i
  SET unit_id = u.id
  FROM catalog_units u
  WHERE i.tenant_id = p_tenant
    AND u.tenant_id = p_tenant
    AND u.code = 'pc'
    AND i.unit_id IS NULL;

  UPDATE item_variants v
  SET unit_id = u.id
  FROM items i, catalog_units u
  WHERE v.item_id = i.id
    AND i.tenant_id = p_tenant
    AND u.tenant_id = p_tenant
    AND u.code = 'pc'
    AND v.unit_id IS NULL;
END;
$$;

SELECT catalog_seed_units(id) FROM tenants;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'items_unit_id_fkey'
  ) THEN
    ALTER TABLE items
      ADD CONSTRAINT items_unit_id_fkey
      FOREIGN KEY (tenant_id, unit_id) REFERENCES catalog_units (tenant_id, id);
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'item_variants_unit_id_fkey'
  ) THEN
    ALTER TABLE item_variants
      ADD CONSTRAINT item_variants_unit_id_fkey
      FOREIGN KEY (unit_id) REFERENCES catalog_units (id);
  END IF;
END $$;
