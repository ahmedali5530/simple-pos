-- Cashier role defaults live with the permission catalog, not in engine code.
ALTER TABLE platform_permissions
    ADD COLUMN IF NOT EXISTS default_for_cashier BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE platform_permissions
SET default_for_cashier = TRUE
WHERE name IN ('catalog.read', 'inventory.read', 'pos.read', 'pos.bills.post', 'cash.read');
