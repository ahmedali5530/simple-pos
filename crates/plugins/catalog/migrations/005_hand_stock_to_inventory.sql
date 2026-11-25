-- Phase 2: Catalog hands stock/avg_cost to Inventory's inventory_balances, then drops the columns.

INSERT INTO inventory_balances (tenant_id, item_id, stock, avg_cost)
SELECT tenant_id, id, stock, avg_cost
FROM items
ON CONFLICT (tenant_id, item_id) DO UPDATE
SET stock = EXCLUDED.stock,
    avg_cost = EXCLUDED.avg_cost;

ALTER TABLE items DROP COLUMN IF EXISTS stock;
ALTER TABLE items DROP COLUMN IF EXISTS avg_cost;
