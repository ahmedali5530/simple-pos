INSERT INTO sections (id, name, short_name, kind, accent, show_on_pos, pos_order)
VALUES
    (1, 'Chaska Corner Pan & Tuck Shop', 'Pan & Tuck', 'retail', '#0f766e', TRUE, 0),
    (2, 'Chaska Corner Foodies', 'Foodies', 'food', '#c2410c', TRUE, 1),
    (3, 'Cold Corner', 'Cold Corner', 'retail', '#0369a1', FALSE, 2)
ON CONFLICT (id) DO NOTHING;

INSERT INTO categories (id, name, sort_order)
VALUES
    (1, 'Shop', 0),
    (2, 'Meals', 1),
    (3, 'Sides', 2),
    (4, 'Cold Drinks', 3)
ON CONFLICT (id) DO NOTHING;

INSERT INTO items (
    id, section_id, category_id, barcode, name, rate_rupees, stock,
    portion, group_name, signature, sort_order
)
VALUES
    (1, 1, 1, '1001', 'Pan', 50, 100, 'regular', NULL, FALSE, 0),
    (2, 1, 1, '1002', 'Regular Bottle', 80, 72, 'regular', NULL, FALSE, 1),
    (3, 1, 1, '1003', 'Catrage Marome', 200, 40, 'regular', NULL, FALSE, 2),
    (4, 1, 1, '1004', 'Lighter', 60, 45, 'regular', NULL, FALSE, 3),
    (5, 2, 2, '2001', 'Daal Chawal (Full)', 250, 62, 'full', 'Daal Chawal', TRUE, 0),
    (6, 2, 2, '2002', 'Daal Chawal (Half)', 150, 68, 'half', 'Daal Chawal', TRUE, 1),
    (7, 2, 2, '2003', 'Chicken Biryani (Full)', 320, 45, 'full', NULL, FALSE, 2),
    (8, 2, 3, '2004', 'Raita / Salad', 50, 80, 'regular', NULL, FALSE, 3),
    (9, 3, 4, '3001', 'Cold Drink 500ml', 100, 40, 'regular', NULL, FALSE, 0),
    (10, 3, 4, '3002', 'Water 1.5L', 80, 30, 'regular', NULL, FALSE, 1)
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('sections', 'id'), (SELECT MAX(id) FROM sections));
SELECT setval(pg_get_serial_sequence('categories', 'id'), (SELECT MAX(id) FROM categories));
SELECT setval(pg_get_serial_sequence('items', 'id'), (SELECT MAX(id) FROM items));
