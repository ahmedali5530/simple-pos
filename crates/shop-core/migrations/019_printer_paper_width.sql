ALTER TABLE shop_settings
    ADD COLUMN printer_paper_width INT NOT NULL DEFAULT 80;

ALTER TABLE shop_settings
    DROP CONSTRAINT IF EXISTS shop_settings_printer_paper_width_check;

ALTER TABLE shop_settings
    ADD CONSTRAINT shop_settings_printer_paper_width_check
    CHECK (printer_paper_width IN (58, 72, 80));
