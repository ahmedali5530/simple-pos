-- Phase 2: Inventory owns on-hand stock and average cost via inventory_balances.
-- The table was created in core migration 007; Catalog migration 005 syncs and drops
-- items.stock / items.avg_cost. This migration records inventory ownership.

DO $$
BEGIN
  IF to_regclass('public.inventory_balances') IS NULL
     OR to_regclass('public.stock_movements') IS NULL THEN
    RAISE EXCEPTION 'inventory tables are missing; core migration 007 must run first';
  END IF;
END $$;

ALTER TABLE inventory_balances ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory_balances FORCE ROW LEVEL SECURITY;
