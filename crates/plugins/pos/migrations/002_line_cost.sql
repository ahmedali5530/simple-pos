-- Bill lines snapshot what the item cost when it sold. Reports read this column
-- instead of the item's current average cost, so editing a price or cost later
-- never rewrites an older bill.
DO $$
BEGIN
  IF to_regclass('public.bill_lines') IS NULL THEN
    RETURN;
  END IF;
  ALTER TABLE bill_lines ADD COLUMN IF NOT EXISTS unit_cost BIGINT NOT NULL DEFAULT 0;
  UPDATE bill_lines
  SET unit_cost = CASE WHEN qty > 0 THEN cost_total / qty ELSE 0 END
  WHERE unit_cost = 0 AND cost_total <> 0;
END $$;
