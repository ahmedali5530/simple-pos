use platform_kernel::migrate::Migration;

pub fn migrations() -> &'static [Migration] {
    static MIGRATIONS: &[Migration] = &[
        Migration::new(1, "tenants_and_profile", include_str!("001_tenants.sql")),
        Migration::new(2, "identity_roles", include_str!("002_identity.sql")),
        Migration::new(3, "tenant_id_backfill", include_str!("003_tenant_id.sql")),
        Migration::new(4, "rls_policies", include_str!("004_rls.sql")),
        Migration::new(5, "session_hardening", include_str!("005_sessions.sql")),
        Migration::new(6, "events_and_audit", include_str!("006_events.sql")),
        Migration::new(
            7,
            "plugin_data_decoupling",
            include_str!("007_decouple.sql"),
        ),
        Migration::new(
            8,
            "plugin_lifecycle",
            include_str!("008_plugin_lifecycle.sql"),
        ),
        Migration::new(9, "plugin_settings", include_str!("009_settings.sql")),
        Migration::new(10, "worker_event_peek", include_str!("010_worker_peek.sql")),
        Migration::new(
            11,
            "worker_event_peek_permissive",
            include_str!("011_worker_peek_permissive.sql"),
        ),
        Migration::new(
            12,
            "cashier_permissions",
            include_str!("012_cashier_permissions.sql"),
        ),
        Migration::new(
            13,
            "remove_pos_printer_columns",
            include_str!("013_remove_pos_printer_columns.sql"),
        ),
        Migration::new(
            14,
            "permission_defaults",
            include_str!("014_permission_defaults.sql"),
        ),
        Migration::new(
            15,
            "movement_reference",
            include_str!("015_movement_reference.sql"),
        ),
        Migration::new(16, "enable_core", include_str!("016_enable_core.sql")),
    ];
    MIGRATIONS
}
