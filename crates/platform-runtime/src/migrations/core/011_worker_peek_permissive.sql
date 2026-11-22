-- Permissive policies are OR-combined, and Postgres still evaluates the cast.
-- An empty app.tenant_id aborted the worker peek even after the restrictive fix.
DROP POLICY IF EXISTS tenant_isolation_permissive ON platform_events;
CREATE POLICY tenant_isolation_permissive ON platform_events AS PERMISSIVE FOR ALL TO PUBLIC
  USING (
    CASE
      WHEN current_setting('app.worker', true) = '1' THEN true
      WHEN NULLIF(current_setting('app.tenant_id', true), '') IS NULL THEN false
      ELSE tenant_id = NULLIF(current_setting('app.tenant_id', true), '')::uuid
    END
  )
  WITH CHECK (
    CASE
      WHEN NULLIF(current_setting('app.tenant_id', true), '') IS NULL THEN false
      ELSE tenant_id = NULLIF(current_setting('app.tenant_id', true), '')::uuid
    END
  );
