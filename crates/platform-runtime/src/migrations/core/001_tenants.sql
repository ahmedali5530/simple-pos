CREATE TABLE IF NOT EXISTS tenants (
    id uuid PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    host TEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO tenants (id, code, host)
VALUES ('00000000-0000-4000-8000-000000000001', 'default', NULL)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS tenant_profile (
    tenant_id uuid PRIMARY KEY REFERENCES tenants (id),
    name TEXT NOT NULL,
    currency TEXT NOT NULL,
    locale TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'Asia/Karachi'
);

INSERT INTO tenant_profile (tenant_id, name, currency, locale, timezone)
SELECT
    '00000000-0000-4000-8000-000000000001',
    COALESCE((SELECT name FROM shop_settings WHERE id = 1), 'Shop'),
    COALESCE((SELECT currency FROM shop_settings WHERE id = 1), 'PKR'),
    COALESCE((SELECT locale FROM shop_settings WHERE id = 1), 'en-PK'),
    'Asia/Karachi'
ON CONFLICT (tenant_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS platform_permissions (
    name TEXT PRIMARY KEY,
    description TEXT NOT NULL DEFAULT ''
);

INSERT INTO platform_permissions (name, description) VALUES
    ('platform.admin', 'Full platform administration'),
    ('platform.plugins.manage', 'Enable and disable plugins'),
    ('platform.tokens.manage', 'Manage API tokens'),
    ('catalog.read', 'Read catalog'),
    ('catalog.write', 'Write catalog'),
    ('inventory.read', 'Read inventory'),
    ('inventory.write', 'Write inventory'),
    ('pos.bills.post', 'Post bills'),
    ('pos.read', 'Read POS data'),
    ('cash.read', 'Read cash desk'),
    ('cash.write', 'Open and close cash shifts'),
    ('reports.read', 'Read reports'),
    ('printing.use', 'Print documents'),
    ('settings.write', 'Change shop settings'),
    ('users.write', 'Manage users')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS tenant_plugins (
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    plugin_id TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    version TEXT NOT NULL DEFAULT '0.1.0',
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (tenant_id, plugin_id)
);

INSERT INTO tenant_plugins (tenant_id, plugin_id, enabled, version)
VALUES
    ('00000000-0000-4000-8000-000000000001', 'catalog', TRUE, '0.1.0'),
    ('00000000-0000-4000-8000-000000000001', 'inventory', TRUE, '0.1.0'),
    ('00000000-0000-4000-8000-000000000001', 'pos', TRUE, '0.1.0'),
    ('00000000-0000-4000-8000-000000000001', 'cash', TRUE, '0.1.0'),
    ('00000000-0000-4000-8000-000000000001', 'printing', TRUE, '0.1.0'),
    ('00000000-0000-4000-8000-000000000001', 'reports', TRUE, '0.1.0')
ON CONFLICT DO NOTHING;
