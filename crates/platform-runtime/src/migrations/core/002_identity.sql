CREATE TABLE IF NOT EXISTS roles (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    name TEXT NOT NULL,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, name)
);

CREATE TABLE IF NOT EXISTS role_permissions (
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    role_id INT NOT NULL,
    permission TEXT NOT NULL REFERENCES platform_permissions (name),
    PRIMARY KEY (tenant_id, role_id, permission),
    FOREIGN KEY (tenant_id, role_id) REFERENCES roles (tenant_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_roles (
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    user_id INT NOT NULL,
    role_id INT NOT NULL,
    PRIMARY KEY (tenant_id, user_id, role_id)
);

CREATE TABLE IF NOT EXISTS api_tokens (
    id SERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    permissions TEXT[] NOT NULL DEFAULT '{}',
    allow_destructive BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, token_hash)
);

CREATE TABLE IF NOT EXISTS platform_audit_log (
    id BIGSERIAL PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    actor_id INT,
    action TEXT NOT NULL,
    detail JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS platform_idempotency_keys (
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    key TEXT NOT NULL,
    response JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, key)
);

-- Seed manager and cashier roles for the default tenant with full compatibility permissions.
INSERT INTO roles (id, tenant_id, name, is_system)
VALUES
    (1, '00000000-0000-4000-8000-000000000001', 'manager', TRUE),
    (2, '00000000-0000-4000-8000-000000000001', 'cashier', TRUE)
ON CONFLICT DO NOTHING;

SELECT setval(pg_get_serial_sequence('roles', 'id'), (SELECT COALESCE(MAX(id), 1) FROM roles));

INSERT INTO role_permissions (tenant_id, role_id, permission)
SELECT '00000000-0000-4000-8000-000000000001', 1, name FROM platform_permissions
ON CONFLICT DO NOTHING;

-- Compatibility: cashiers keep all current permissions so behavior does not break.
INSERT INTO role_permissions (tenant_id, role_id, permission)
SELECT '00000000-0000-4000-8000-000000000001', 2, name FROM platform_permissions
ON CONFLICT DO NOTHING;

INSERT INTO user_roles (tenant_id, user_id, role_id)
SELECT
    '00000000-0000-4000-8000-000000000001',
    u.id,
    CASE WHEN u.role = 'manager' THEN 1 ELSE 2 END
FROM users u
ON CONFLICT DO NOTHING;
