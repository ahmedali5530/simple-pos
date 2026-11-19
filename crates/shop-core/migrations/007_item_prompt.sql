ALTER TABLE items
    ADD COLUMN prompt TEXT NOT NULL DEFAULT 'none' CHECK (prompt IN ('none', 'quantity', 'price')),
    ADD COLUMN base_rate INT CHECK (base_rate IS NULL OR base_rate > 0),
    ADD COLUMN scale INT NOT NULL DEFAULT 1 CHECK (scale > 0);

UPDATE items
SET prompt = 'quantity', base_rate = rate, scale = 1000
WHERE id = 13;
