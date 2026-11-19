ALTER TABLE bills
    ADD COLUMN customer TEXT;

ALTER TABLE bills
    DROP CONSTRAINT IF EXISTS bills_customer_check;

ALTER TABLE bills
    ADD CONSTRAINT bills_customer_check
    CHECK (customer IS NULL OR char_length(customer) BETWEEN 1 AND 80);
