-- Keep the receipt number on the stock movement. This lets inventory and the
-- workbook exporter read movement history without joining the POS bills table.
ALTER TABLE stock_movements
    ADD COLUMN IF NOT EXISTS bill_no TEXT;

UPDATE stock_movements m
SET bill_no = b.bill_no
FROM bills b
WHERE m.bill_id = b.id
  AND (m.bill_no IS NULL OR m.bill_no = '');
