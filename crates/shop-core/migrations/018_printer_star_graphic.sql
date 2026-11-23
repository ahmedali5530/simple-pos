ALTER TABLE shop_settings
    DROP CONSTRAINT IF EXISTS shop_settings_printer_command_set_check;

ALTER TABLE shop_settings
    ADD CONSTRAINT shop_settings_printer_command_set_check
    CHECK (printer_command_set IN ('escpos', 'star', 'star-graphic'));
