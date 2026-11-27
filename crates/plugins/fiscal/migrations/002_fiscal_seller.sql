ALTER TABLE fiscal_settings
    ADD COLUMN IF NOT EXISTS business_name TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS province TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS address TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS scenario_id TEXT NOT NULL DEFAULT 'SN001',
    ADD COLUMN IF NOT EXISTS hs_code TEXT NOT NULL DEFAULT '0101.2100',
    ADD COLUMN IF NOT EXISTS uom TEXT NOT NULL DEFAULT 'Numbers, pieces, units',
    ADD COLUMN IF NOT EXISTS sale_type TEXT NOT NULL DEFAULT 'Goods at standard rate (default)',
    ADD COLUMN IF NOT EXISTS st_rate TEXT NOT NULL DEFAULT '18%',
    ADD COLUMN IF NOT EXISTS tax_inclusive BOOLEAN NOT NULL DEFAULT true;

ALTER TABLE fiscal_settings DROP CONSTRAINT IF EXISTS fiscal_settings_mode_check;
ALTER TABLE fiscal_settings
    ADD CONSTRAINT fiscal_settings_mode_check
    CHECK (mode IN ('demo', 'sandbox', 'live'));

-- Previous "sandbox" only faked a QR locally. Keep that behaviour as demo;
-- real FBR sandbox posting uses mode = sandbox with an API token.
UPDATE fiscal_settings SET mode = 'demo' WHERE mode = 'sandbox';
