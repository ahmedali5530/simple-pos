ALTER TABLE shop_settings
    ADD COLUMN bill_prefix TEXT NOT NULL DEFAULT 'BILL';

UPDATE shop_settings AS shop
SET bill_prefix = UPPER(prefix.bill_prefix)
FROM (
    SELECT split_part(bill_no, '-', 1) AS bill_prefix
    FROM bills
    WHERE bill_no LIKE '%-%'
    ORDER BY id DESC
    LIMIT 1
) AS prefix
WHERE shop.id = 1
  AND prefix.bill_prefix ~ '^[A-Za-z0-9]{1,8}$';

ALTER TABLE shop_settings
    DROP CONSTRAINT IF EXISTS shop_settings_bill_prefix_check;

ALTER TABLE shop_settings
    ADD CONSTRAINT shop_settings_bill_prefix_check
    CHECK (bill_prefix ~ '^[A-Z0-9]{1,8}$');
