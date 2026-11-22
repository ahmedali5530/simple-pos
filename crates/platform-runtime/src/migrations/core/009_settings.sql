CREATE TABLE IF NOT EXISTS plugin_settings (
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    plugin_id TEXT NOT NULL,
    value JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, plugin_id)
);

ALTER TABLE plugin_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE plugin_settings FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON plugin_settings;
DROP POLICY IF EXISTS tenant_isolation_permissive ON plugin_settings;
CREATE POLICY tenant_isolation_restrictive ON plugin_settings AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON plugin_settings AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE TABLE IF NOT EXISTS platform_event_failures (
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    subscriber_id TEXT NOT NULL,
    event_id uuid NOT NULL,
    error TEXT NOT NULL,
    attempts INT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, subscriber_id, event_id)
);

ALTER TABLE platform_event_failures ENABLE ROW LEVEL SECURITY;
ALTER TABLE platform_event_failures FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON platform_event_failures;
DROP POLICY IF EXISTS tenant_isolation_permissive ON platform_event_failures;
CREATE POLICY tenant_isolation_restrictive ON platform_event_failures AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON platform_event_failures AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
DROP POLICY IF EXISTS worker_peek ON platform_event_failures;
CREATE POLICY worker_peek ON platform_event_failures AS PERMISSIVE FOR ALL TO PUBLIC
  USING (current_setting('app.worker', true) = '1')
  WITH CHECK (current_setting('app.worker', true) = '1');
