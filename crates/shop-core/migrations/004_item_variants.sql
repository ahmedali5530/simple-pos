CREATE TABLE item_variants (
    id SERIAL PRIMARY KEY,
    item_id INT NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    rate INT NOT NULL CHECK (rate > 0),
    units INT NOT NULL DEFAULT 1 CHECK (units > 0),
    barcode TEXT UNIQUE,
    sort_order INT NOT NULL DEFAULT 0,
    UNIQUE (item_id, name)
);

CREATE INDEX item_variants_item_idx ON item_variants (item_id, sort_order);

ALTER TABLE bill_lines
    ADD COLUMN variant_id INT REFERENCES item_variants (id) ON DELETE SET NULL,
    ADD COLUMN units INT NOT NULL DEFAULT 1 CHECK (units > 0);

INSERT INTO items (id, section_id, category_id, barcode, name, rate, stock, sort_order)
VALUES
    (11, 1, 1, NULL, 'Cigarette', 10, 200, 4),
    (12, 2, 2, NULL, 'Chicken Karahi', 400, 24, 4)
ON CONFLICT (id) DO NOTHING;

INSERT INTO item_variants (item_id, name, rate, units, barcode, sort_order)
VALUES
    (11, 'Piece', 10, 1, '1005', 0),
    (11, 'Pack', 180, 20, '1006', 1),
    (12, 'Full', 400, 1, '2005', 0),
    (12, 'Half', 250, 1, '2006', 1)
ON CONFLICT (item_id, name) DO NOTHING;

SELECT setval(pg_get_serial_sequence('items', 'id'), (SELECT MAX(id) FROM items));
SELECT setval(pg_get_serial_sequence('item_variants', 'id'), (SELECT MAX(id) FROM item_variants));
