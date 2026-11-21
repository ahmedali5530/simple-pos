use platform_kernel::migrate::Migration;

pub fn migrations() -> &'static [Migration] {
    static MIGRATIONS: &[Migration] = &[
        Migration::new(1, "tenants_and_profile", include_str!("001_tenants.sql")),
        Migration::new(2, "identity_roles", include_str!("002_identity.sql")),
        Migration::new(3, "tenant_id_backfill", include_str!("003_tenant_id.sql")),
        Migration::new(4, "rls_policies", include_str!("004_rls.sql")),
        Migration::new(5, "session_hardening", include_str!("005_sessions.sql")),
        Migration::new(6, "events_and_audit", include_str!("006_events.sql")),
        Migration::new(7, "plugin_data_decoupling", include_str!("007_decouple.sql")),
    ];
    MIGRATIONS
}
