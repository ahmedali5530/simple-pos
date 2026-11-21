-- Additive plugin lifecycle columns. Disabling a plugin never deletes its row.

ALTER TABLE tenant_plugins
    ADD COLUMN IF NOT EXISTS installed_version TEXT;

ALTER TABLE tenant_plugins
    ADD COLUMN IF NOT EXISTS installed_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE tenant_plugins
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE tenant_plugins
    ADD COLUMN IF NOT EXISTS updated_by INT;

UPDATE tenant_plugins
SET installed_version = version
WHERE installed_version IS NULL;

ALTER TABLE platform_permissions
    ADD COLUMN IF NOT EXISTS plugin_id TEXT;
