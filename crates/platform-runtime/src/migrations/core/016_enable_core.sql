UPDATE tenant_plugins
SET enabled = TRUE, updated_at = now()
WHERE plugin_id = 'core' AND enabled = FALSE;
