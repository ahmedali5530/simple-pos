-- Printer configuration is served and written by the Printing plugin. Its
-- values were copied to printing_settings by core migration 007.
ALTER TABLE shop_settings
    DROP COLUMN IF EXISTS print_enabled,
    DROP COLUMN IF EXISTS printer_kind,
    DROP COLUMN IF EXISTS printer_target,
    DROP COLUMN IF EXISTS printer_port;
