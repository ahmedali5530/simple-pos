# Frozen legacy migrations

Migrations `001_init.sql` through `016_item_cost.sql` are **frozen permanently**.

Do not edit, reorder, or renumber these files. Field installs (desktop and Docker)
already have them recorded in `_sqlx_migrations`. New schema changes belong in
`platform_migrations` (core) or a plugin's own migration set.
