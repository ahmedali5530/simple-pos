-- The worker peeks events before a tenant is selected. The restrictive policy
-- cast an empty app.tenant_id and aborted the whole tick. Worker reads skip
-- that cast. Writes still require a real tenant id.
DROP POLICY IF EXISTS tenant_isolation_restrictive ON platform_events;
CREATE POLICY tenant_isolation_restrictive ON platform_events AS RESTRICTIVE FOR ALL TO PUBLIC
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
