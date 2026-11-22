-- Catalog plugin baseline. The legacy tables already exist; this records that
-- the catalog plugin owns them from here on. No data is moved.

DO $$
BEGIN
  IF to_regclass('public.sections') IS NULL
     OR to_regclass('public.categories') IS NULL
     OR to_regclass('public.items') IS NULL
     OR to_regclass('public.item_variants') IS NULL THEN
    RAISE EXCEPTION 'catalog tables are missing; shop migrations 001-016 must run first';
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS items_category_idx ON items (category_id);
CREATE INDEX IF NOT EXISTS items_active_idx ON items (active, sort_order);
