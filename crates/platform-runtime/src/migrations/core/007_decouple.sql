-- M5 data decoupling: inventory balances, movement sources, POS layout, settings.

CREATE TABLE IF NOT EXISTS inventory_balances (
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    item_id INT NOT NULL,
    stock INT NOT NULL DEFAULT 0 CHECK (stock >= 0),
    avg_cost BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, item_id),
    FOREIGN KEY (tenant_id, item_id) REFERENCES items (tenant_id, id) ON DELETE CASCADE
);

INSERT INTO inventory_balances (tenant_id, item_id, stock, avg_cost)
SELECT tenant_id, id, stock, avg_cost FROM items
ON CONFLICT (tenant_id, item_id) DO NOTHING;

ALTER TABLE stock_movements ADD COLUMN IF NOT EXISTS source_type TEXT;
ALTER TABLE stock_movements ADD COLUMN IF NOT EXISTS source_id INT;
UPDATE stock_movements
SET source_type = CASE WHEN bill_id IS NOT NULL THEN 'bill' ELSE 'manual' END,
    source_id = bill_id
WHERE source_type IS NULL;

CREATE TABLE IF NOT EXISTS pos_section_layout (
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    section_id INT NOT NULL,
    show_on_pos BOOLEAN NOT NULL DEFAULT TRUE,
    pos_order INT NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, section_id),
    FOREIGN KEY (tenant_id, section_id) REFERENCES sections (tenant_id, id) ON DELETE CASCADE
);

INSERT INTO pos_section_layout (tenant_id, section_id, show_on_pos, pos_order)
SELECT tenant_id, id, show_on_pos, pos_order FROM sections
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS pos_settings (
    tenant_id uuid PRIMARY KEY REFERENCES tenants (id),
    bill_prefix TEXT NOT NULL DEFAULT 'BILL',
    last_section_id INT
);

INSERT INTO pos_settings (tenant_id, bill_prefix, last_section_id)
SELECT
    s.tenant_id,
    COALESCE(s.bill_prefix, 'BILL'),
    (SELECT last_section_id FROM pos_state p WHERE p.tenant_id = s.tenant_id LIMIT 1)
FROM shop_settings s
ON CONFLICT (tenant_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS pos_bill_counters (
    tenant_id uuid PRIMARY KEY REFERENCES tenants (id),
    next_id BIGINT NOT NULL DEFAULT 1
);

INSERT INTO pos_bill_counters (tenant_id, next_id)
SELECT
    '00000000-0000-4000-8000-000000000001',
    COALESCE((SELECT last_value + 1 FROM bills_id_seq), 1)
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS printing_settings (
    tenant_id uuid PRIMARY KEY REFERENCES tenants (id),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    kind TEXT NOT NULL DEFAULT 'network',
    target TEXT NOT NULL DEFAULT '',
    port INT NOT NULL DEFAULT 9100
);

INSERT INTO printing_settings (tenant_id, enabled, kind, target, port)
SELECT
    tenant_id,
    print_enabled,
    printer_kind,
    printer_target,
    printer_port
FROM shop_settings
ON CONFLICT (tenant_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS barcode_rules_v2 (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    prefix TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('weight', 'price')),
    scale INT NOT NULL CHECK (scale >= 1),
    sort_order INT NOT NULL DEFAULT 0,
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, prefix)
);

INSERT INTO barcode_rules_v2 (tenant_id, prefix, mode, scale, sort_order)
SELECT tenant_id, prefix, mode, scale, sort_order FROM barcode_rules
ON CONFLICT DO NOTHING;

-- RLS for new tables
DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY[
    'inventory_balances','pos_section_layout','pos_settings','pos_bill_counters',
    'printing_settings','barcode_rules_v2'
  ]
  LOOP
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
