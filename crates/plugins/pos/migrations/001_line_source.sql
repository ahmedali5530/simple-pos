DO $$
BEGIN
  IF to_regclass('public.bill_lines') IS NULL THEN
    RETURN;
  END IF;
  ALTER TABLE bill_lines ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'catalog.product';
  ALTER TABLE bill_lines ADD COLUMN IF NOT EXISTS reference TEXT NOT NULL DEFAULT '';
  ALTER TABLE bill_lines ADD COLUMN IF NOT EXISTS details JSONB NOT NULL DEFAULT '{}'::jsonb;
END $$;
