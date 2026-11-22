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

A UI plugin in `src/plugins/{id}` exports pages, the outlets it provides, and the components it contributes to other plugins' outlets:

- A page is a path, a lazy component, a permission, and an optional density (`compact` or `touch`).
- `slotsProvided` names the outlets on this plugin's screens, e.g. `catalog.item-form`.
- `contributions` lists components this plugin renders inside another plugin's outlet. Each entry has a stable `id`, the `slot` name, an `order`, and a lazy `component`. A contribution without a component is metadata only and does not render.

Contribution components are lazy and only mount when their owning plugin is both compiled into the backend and enabled for the shop. The host passes a typed context; extra props are ignored. Current outlets:

- `catalog.item-form` — `{ item, itemId, editing, registerExtraSave }`.
  `registerExtraSave(id, save)` registers plugin-owned item data. The callback receives the saved item id, runs after the catalog save (including for a new item), and may throw an `Error` with a user-facing message. Keep details JSON-serializable.
- `pos.line-source` — `{ section, addLine, currency, locale }`.
  `addLine({ item_id, source, reference?, details?, … })` adds a line tied to a catalog item. A non-empty `reference` makes the line key unique, so serialized items do not merge into one quantity.
- `pos.line-detail` — `{ line, patch, currency, locale }`.
  `patch({ source?, reference?, details? })` updates one cart line's plugin metadata.

The shell draws the top bar, the page outlet, and the settings list. It does not switch on business paths.

## Sale line metadata

`bill_lines` carries three plugin-owned columns: `source` (default `catalog.product`), `reference`, and `details JSONB`. POS stores and totals them without reading their meaning. The `sales.bill_posted` event is version 2 and includes `SaleLine` entries (`source`, `reference`, `label`, `quantity`, `unit_amount`, `amount`, `details`), so a plugin can react to its own lines. Consumers that only need the bill can keep reading `bill_no` from the payload or query the tables.

## Mobile shop example

`plugin-mobile` shows the full pattern. It owns `mobile_models`, `mobile_devices`, `mobile_warranties`, and `mobile_claims`; contributes model fields to `catalog.item-form` and a device picker to `pos.line-source`; and consumes `sales.bill_posted` v2. A phone model is a catalog item, while each physical unit is a device row with an IMEI. The sale handler reads `bill_lines` by `bill_id` (the event does not carry `item_id`), marks the device sold, and inserts the warranty idempotently. POS inventory still decrements the catalog item's stock.

## Settings

`settings_schema` is JSON Schema. Values are stored per tenant in `plugin_settings`. Disabling the plugin hides the values and does not delete them.

## Proof a change belongs in a plugin

A new trade (tailor jobs, installments, a butcher's weighed sale) ships as a plugin. The pull request does not modify `platform-kernel` or `platform-runtime`.
