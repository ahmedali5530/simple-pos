-- Enable FORCE RLS and tenant isolation policies on every tenant-owned table.

DO $$
DECLARE
  t text;
  tables text[] := ARRAY[
    'tenant_profile','roles','role_permissions','user_roles','api_tokens',
    'tenant_plugins','platform_audit_log','platform_idempotency_keys',
    'sections','categories','items','item_variants','bills','bill_lines',
    'stock_movements','pos_state','shop_settings','barcode_rules','users',
    'sessions','cash_shifts','cash_expenses'
  ];
BEGIN
  FOREACH t IN ARRAY tables LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_restrictive ON %I', t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_permissive ON %I', t);
    EXECUTE format('DROP POLICY IF EXISTS tenant_isolation_owner ON %I', t);
    EXECUTE format(
      'CREATE POLICY tenant_isolation_restrictive ON %I AS RESTRICTIVE FOR ALL TO PUBLIC
       USING (tenant_id = current_setting(''app.tenant_id'', true)::uuid)
       WITH CHECK (tenant_id = current_setting(''app.tenant_id'', true)::uuid)',
      t
    );
    EXECUTE format(
      'CREATE POLICY tenant_isolation_permissive ON %I AS PERMISSIVE FOR ALL TO PUBLIC
       USING (tenant_id = current_setting(''app.tenant_id'', true)::uuid)
       WITH CHECK (tenant_id = current_setting(''app.tenant_id'', true)::uuid)',
      t
    );
  END LOOP;
END $$;

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tenants_self ON tenants;
CREATE POLICY tenants_self ON tenants AS RESTRICTIVE FOR ALL TO PUBLIC
  USING (id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (id = current_setting('app.tenant_id', true)::uuid);
DROP POLICY IF EXISTS tenants_self_perm ON tenants;
CREATE POLICY tenants_self_perm ON tenants AS PERMISSIVE FOR ALL TO PUBLIC
  USING (id = current_setting('app.tenant_id', true)::uuid)
  WITH CHECK (id = current_setting('app.tenant_id', true)::uuid);

-- Cross-tenant session resolution (returns only tenant_id, user_id, expires_at).
CREATE OR REPLACE FUNCTION platform_resolve_session(p_token_hash text)
RETURNS TABLE (tenant_id uuid, user_id int, expires_at timestamptz)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
  RETURN QUERY
  SELECT s.tenant_id, s.user_id, s.expires_at
  FROM sessions s
  WHERE s.token_hash = p_token_hash
     OR (s.token_hash IS NULL AND s.token = p_token_hash);
END;
$$;

CREATE OR REPLACE FUNCTION platform_resolve_tenant(p_host_or_code text)
RETURNS TABLE (tenant_id uuid)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
  RETURN QUERY
  SELECT t.id
  FROM tenants t
  WHERE t.code = p_host_or_code OR t.host = p_host_or_code
  LIMIT 1;
  IF NOT FOUND THEN
    RETURN QUERY SELECT '00000000-0000-4000-8000-000000000001'::uuid;
  END IF;
END;
$$;

CREATE OR REPLACE FUNCTION platform_provision_tenant(p_id uuid, p_code text, p_name text)
RETURNS uuid
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
  INSERT INTO tenants (id, code) VALUES (p_id, p_code)
  ON CONFLICT (id) DO NOTHING;
  INSERT INTO tenant_profile (tenant_id, name, currency, locale, timezone)
  VALUES (p_id, p_name, 'PKR', 'en-PK', 'Asia/Karachi')
  ON CONFLICT (tenant_id) DO NOTHING;
  RETURN p_id;
END;
$$;

REVOKE ALL ON FUNCTION platform_resolve_session(text) FROM PUBLIC;
REVOKE ALL ON FUNCTION platform_resolve_tenant(text) FROM PUBLIC;
REVOKE ALL ON FUNCTION platform_provision_tenant(uuid, text, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION platform_resolve_session(text) TO PUBLIC;
GRANT EXECUTE ON FUNCTION platform_resolve_tenant(text) TO PUBLIC;
GRANT EXECUTE ON FUNCTION platform_provision_tenant(uuid, text, text) TO PUBLIC;
