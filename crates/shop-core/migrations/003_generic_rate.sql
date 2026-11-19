ALTER TABLE items RENAME COLUMN rate_rupees TO rate;
ALTER TABLE bills RENAME COLUMN total_rupees TO total;
ALTER TABLE bill_lines RENAME COLUMN rate_rupees TO rate;
ALTER TABLE bill_lines RENAME COLUMN amount_rupees TO amount;
