INSERT INTO categories (id, name, sort_order)
VALUES
    (5, 'Tobacco', 0),
    (6, 'Snacks', 4),
    (7, 'Grocery', 5),
    (8, 'Toiletries', 6)
ON CONFLICT (id) DO NOTHING;

UPDATE categories SET sort_order = 20 WHERE id = 1;

UPDATE items SET category_id = 5, sort_order = 100 WHERE id = 1;
UPDATE items SET category_id = 4, sort_order = 200 WHERE id = 2;
UPDATE items SET category_id = 7, sort_order = 411 WHERE id = 3;
UPDATE items SET category_id = 5, sort_order = 107 WHERE id = 4;
UPDATE items SET category_id = 5, sort_order = 106 WHERE id = 11;
UPDATE items SET category_id = 7, sort_order = 412 WHERE id = 13;

INSERT INTO items (id, section_id, category_id, barcode, name, rate, stock, sort_order)
VALUES
    (14, 1, 5, '1010', 'Meetha Pan', 40, 80, 101),
    (15, 1, 5, '1011', 'Gutka', 20, 120, 102),
    (16, 1, 5, '1012', 'Paan Masala', 15, 100, 103),
    (17, 1, 5, '1013', 'Supari', 10, 150, 104),
    (18, 1, 5, '1014', 'Matchbox', 10, 80, 105),
    (19, 1, 4, '1015', 'Sting 250ml', 80, 48, 201),
    (20, 1, 4, '1016', 'Pepsi 500ml', 100, 36, 202),
    (21, 1, 4, '1017', '7UP 500ml', 100, 36, 203),
    (22, 1, 4, '1018', 'Mirinda 500ml', 100, 24, 204),
    (23, 1, 4, '1019', 'Nestle Water 500ml', 50, 48, 205),
    (24, 1, 4, '1020', 'Nestle Water 1.5L', 80, 24, 206),
    (25, 1, 4, '1021', 'Mango Juice 250ml', 60, 40, 207),
    (26, 1, 4, '1022', 'Red Bull 250ml', 250, 18, 208),
    (27, 1, 4, '1023', 'Milk Pack 250ml', 70, 36, 209),
    (28, 1, 6, '1024', 'Lays Classic', 50, 40, 301),
    (29, 1, 6, '1025', 'Lays Masala', 50, 40, 302),
    (30, 1, 6, '1026', 'Kurkure', 40, 36, 303),
    (31, 1, 6, '1027', 'Slanty', 30, 48, 304),
    (32, 1, 6, '1028', 'Cheetos', 50, 24, 305),
    (33, 1, 6, '1029', 'Prince Biscuit', 40, 30, 306),
    (34, 1, 6, '1030', 'Sooper Biscuit', 50, 30, 307),
    (35, 1, 6, '1031', 'Oreo', 80, 20, 308),
    (36, 1, 6, '1032', 'Gala Biscuit', 30, 36, 309),
    (37, 1, 6, '1033', 'Nimco Mix', 60, 25, 310),
    (38, 1, 6, '1034', 'Cake Rusk', 40, 20, 311),
    (39, 1, 6, '1035', 'Chocolate Bar', 60, 30, 312),
    (40, 1, 6, '1036', 'Bubble Gum', 10, 100, 313),
    (41, 1, 6, '1037', 'Salted Peanuts', 40, 25, 314),
    (42, 1, 7, '1038', 'White Bread', 120, 15, 401),
    (43, 1, 7, '1039', 'Eggs Half Dozen', 150, 20, 402),
    (44, 1, 7, '1040', 'Milk 1L', 220, 18, 403),
    (45, 1, 7, '1041', 'Yogurt 500g', 140, 12, 404),
    (46, 1, 7, '1042', 'Sugar 1kg', 180, 20, 405),
    (47, 1, 7, '1043', 'Tea 190g', 280, 16, 406),
    (48, 1, 7, '1044', 'Cooking Oil 1L', 550, 10, 407),
    (49, 1, 7, '1045', 'Iodised Salt', 40, 30, 408),
    (50, 1, 7, '1046', 'Rice 1kg', 250, 15, 409),
    (51, 1, 7, '1047', 'Daal Mash 1kg', 320, 12, 410),
    (52, 1, 8, '1048', 'Bath Soap', 80, 24, 501),
    (53, 1, 8, '1049', 'Surf 90g', 40, 30, 502),
    (54, 1, 8, '1050', 'Shampoo Sachet', 20, 50, 503),
    (55, 1, 8, '1051', 'Toothpaste', 180, 16, 504),
    (56, 1, 8, '1052', 'Tissue Pack', 120, 20, 505),
    (57, 1, 8, '1053', 'Disposable Razor', 40, 18, 506)
ON CONFLICT (id) DO NOTHING;

SELECT setval(pg_get_serial_sequence('categories', 'id'), (SELECT MAX(id) FROM categories));
SELECT setval(pg_get_serial_sequence('items', 'id'), (SELECT MAX(id) FROM items));
