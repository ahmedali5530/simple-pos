CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS platform_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL DEFAULT current_setting('app.tenant_id')::uuid REFERENCES tenants (id),
    event_type TEXT NOT NULL,
    version INT NOT NULL DEFAULT 1,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS platform_events_pending_idx
  ON platform_events (created_at, id);

CREATE TABLE IF NOT EXISTS platform_event_deliveries (
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    subscriber_id TEXT NOT NULL,
    last_event_id uuid,
    last_created_at TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, subscriber_id)
);

ALTER TABLE platform_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE platform_events FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON platform_events;
DROP POLICY IF EXISTS tenant_isolation_permissive ON platform_events;
CREATE POLICY tenant_isolation_restrictive ON platform_events AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON platform_events AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

ALTER TABLE platform_event_deliveries ENABLE ROW LEVEL SECURITY;
ALTER TABLE platform_event_deliveries FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenant_isolation_restrictive ON platform_event_deliveries;
DROP POLICY IF EXISTS tenant_isolation_permissive ON platform_event_deliveries;
CREATE POLICY tenant_isolation_restrictive ON platform_event_deliveries AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
CREATE POLICY tenant_isolation_permissive ON platform_event_deliveries AS PERMISSIVE FOR ALL TO PUBLIC
  USING (tenant_id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);

-- Worker can peek at event headers across tenants (no payload needed for routing).
DROP POLICY IF EXISTS worker_peek ON platform_events;
CREATE POLICY worker_peek ON platform_events AS PERMISSIVE FOR SELECT TO PUBLIC
  USING (current_setting('app.worker', true) = '1');
