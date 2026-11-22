ALTER TABLE items
    ADD COLUMN IF NOT EXISTS item_type TEXT NOT NULL DEFAULT 'product';

ALTER TABLE items DROP CONSTRAINT IF EXISTS items_item_type_check;
ALTER TABLE items
    ADD CONSTRAINT items_item_type_check CHECK (item_type IN ('product', 'service'));

UPDATE items SET item_type = 'product' WHERE item_type IS NULL OR item_type = '';
