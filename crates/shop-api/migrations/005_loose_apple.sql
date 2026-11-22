INSERT INTO items (id, section_id, category_id, barcode, name, rate, stock, sort_order)
VALUES (13, 1, 1, '00042', 'Loose Apple', 200, 50000, 5)
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('items', 'id'), (SELECT MAX(id) FROM items));
