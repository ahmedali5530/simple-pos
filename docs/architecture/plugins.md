# Plugin authoring

The engine knows tenants, identity, plugins, events, capabilities, settings, files, and the screen shell. It does not know what a shop sells. A plugin owns one business area: its tables, rules, HTTP routes, and screens.

Plugins are compiled into the binary. Adding one changes the composition root (`shop-api` and `src/plugins/registry.ts`), not the kernel.

## Backend

Implement `platform_kernel::plugin::Plugin`.

- `manifest` — id, version, description, dependencies, permissions, capabilities, events, pages, slots, and an optional JSON Schema for settings. `consents.enable` and `consents.disable` are optional. A tax-authority plugin can require an acknowledgement before it is turned on, and a different confirmation before it is turned off. A plugin with no consent switches immediately.
- `migrations` — SQL owned by the plugin. Existing tables are adopted, never dropped when the plugin is disabled. New tables use `FORCE ROW LEVEL SECURITY`.
- `routes` — relative paths. The runtime mounts them at `/api/v1/{plugin_id}/`. A disabled plugin has no routes for that tenant.
- `event_handlers` — subscribe only to event types listed on the manifest. Handlers for a disabled plugin are skipped.
- `register` — provide capabilities and other contracts through `Registrar`.

Talk to other plugins in only three ways:

1. Events, for facts that already happened (`sales.bill_posted`).
2. Capabilities, for a typed request (`catalog.find`).
3. UI slots, for a screen that contributes to another screen.

Do not read another plugin's tables. Shared shapes live in `crates/contracts`, which plugins own. The kernel does not import those crates.

Database access is a `TenantDb` on the request. Open a transaction with `begin`. There is no pool getter.

Permissions use the plugin id as a prefix (`catalog.read`). `users.write` and `settings.write` are the shared exceptions.

## Frontend

A UI plugin in `src/plugins/{id}` exports pages and slots:

- A page is a path, a lazy component, a permission, and an optional density (`compact` or `touch`).
- A slot is a name, a component, and an order. The shell renders every enabled contribution for that name.

The shell draws the top bar, the page outlet, and the settings list. It does not switch on business paths.

## Settings

`settings_schema` is JSON Schema. Values are stored per tenant in `plugin_settings`. Disabling the plugin hides the values and does not delete them.

## Proof a change belongs in a plugin

A new trade (tailor jobs, installments, a butcher's weighed sale) ships as a plugin. The pull request does not modify `platform-kernel` or `platform-runtime`.
