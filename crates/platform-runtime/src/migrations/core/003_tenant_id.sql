-- Add tenant_id to every legacy business table and rebuild uniqueness / FKs.

ALTER TABLE sections ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE categories ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE items ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE item_variants ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE bills ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE bill_lines ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE stock_movements ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE pos_state ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE shop_settings ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE barcode_rules ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE users ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS tenant_id uuid;
ALTER TABLE cash_expenses ADD COLUMN IF NOT EXISTS tenant_id uuid;

UPDATE sections SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE categories SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE items SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE item_variants SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE bills SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE bill_lines SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE stock_movements SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE pos_state SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE shop_settings SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE barcode_rules SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE users SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE sessions SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE cash_shifts SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;
UPDATE cash_expenses SET tenant_id = '00000000-0000-4000-8000-000000000001' WHERE tenant_id IS NULL;

DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY[
    'sections','categories','items','item_variants','bills','bill_lines',
    'stock_movements','pos_state','shop_settings','barcode_rules','users',
    'sessions','cash_shifts','cash_expenses'
  ]
  LOOP
    EXECUTE format(
      'ALTER TABLE %I ALTER COLUMN tenant_id SET DEFAULT current_setting(''app.tenant_id'')::uuid',
      t
    );
    EXECUTE format('ALTER TABLE %I ALTER COLUMN tenant_id SET NOT NULL', t);
  END LOOP;
END $$;

-- Parent uniqueness for composite FKs
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'sections_tenant_id_id_key') THEN
    ALTER TABLE sections ADD CONSTRAINT sections_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'categories_tenant_id_id_key') THEN
    ALTER TABLE categories ADD CONSTRAINT categories_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'items_tenant_id_id_key') THEN
    ALTER TABLE items ADD CONSTRAINT items_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'item_variants_tenant_id_id_key') THEN
    ALTER TABLE item_variants ADD CONSTRAINT item_variants_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'bills_tenant_id_id_key') THEN
    ALTER TABLE bills ADD CONSTRAINT bills_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_tenant_id_id_key') THEN
    ALTER TABLE users ADD CONSTRAINT users_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'cash_shifts_tenant_id_id_key') THEN
    ALTER TABLE cash_shifts ADD CONSTRAINT cash_shifts_tenant_id_id_key UNIQUE (tenant_id, id);
  END IF;
END $$;

-- Rebuild unique indexes per tenant
DROP INDEX IF EXISTS sections_short_name_lower_idx;
CREATE UNIQUE INDEX sections_short_name_lower_idx ON sections (tenant_id, lower(short_name));

DROP INDEX IF EXISTS categories_name_lower_idx;
ALTER TABLE categories DROP CONSTRAINT IF EXISTS categories_name_key;
CREATE UNIQUE INDEX categories_name_lower_idx ON categories (tenant_id, lower(name));

ALTER TABLE items DROP CONSTRAINT IF EXISTS items_barcode_key;
CREATE UNIQUE INDEX items_barcode_key ON items (tenant_id, barcode) WHERE barcode IS NOT NULL;

ALTER TABLE item_variants DROP CONSTRAINT IF EXISTS item_variants_barcode_key;
CREATE UNIQUE INDEX item_variants_barcode_key ON item_variants (tenant_id, barcode) WHERE barcode IS NOT NULL;

ALTER TABLE bills DROP CONSTRAINT IF EXISTS bills_bill_no_key;
CREATE UNIQUE INDEX bills_bill_no_key ON bills (tenant_id, bill_no);

DROP INDEX IF EXISTS users_name_lower_idx;
CREATE UNIQUE INDEX users_name_lower_idx ON users (tenant_id, lower(name));

ALTER TABLE barcode_rules DROP CONSTRAINT IF EXISTS barcode_rules_prefix_key;
CREATE UNIQUE INDEX barcode_rules_prefix_key ON barcode_rules (tenant_id, prefix);

DROP INDEX IF EXISTS cash_shifts_open_idx;
CREATE UNIQUE INDEX cash_shifts_open_idx ON cash_shifts (tenant_id) WHERE closed_at IS NULL;

-- user_roles composite FK now that users have (tenant_id, id)
ALTER TABLE user_roles DROP CONSTRAINT IF EXISTS user_roles_user_fk;
ALTER TABLE user_roles
  ADD CONSTRAINT user_roles_user_fk
  FOREIGN KEY (tenant_id, user_id) REFERENCES users (tenant_id, id) ON DELETE CASCADE;
ALTER TABLE user_roles DROP CONSTRAINT IF EXISTS user_roles_role_fk;
ALTER TABLE user_roles
  ADD CONSTRAINT user_roles_role_fk
  FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON DELETE CASCADE;
