# Plugin authoring

The engine knows tenants, identity, plugins, events, capabilities, settings, files, and the screen shell. It does not know what a shop sells. A plugin owns one business area: its tables, rules, HTTP routes, and screens.

Plugins are compiled into the binary. Adding one is a directory plus one command: the composition root and the UI registry discover it, so no core file needs hand-editing.

## Compatibility

| Surface | Rule |
|---|---|
| Kernel API | Manifests declare `kernel_api: "^1.0"`. The binary refuses plugins outside the supported range. |
| Manifest schema | Version `1` and `2` are accepted. Prefer `PluginManifest::v1` until you need v2 fields; the type is `#[non_exhaustive]`. |
| Plugin version | On boot, stored versions adopt **forward** to the compiled version. A stored version **newer** than the binary is treated as a downgrade and skipped (plugin stays off). |
| UI metadata | Rust `PluginManifest` is the source of truth. `cargo xtask plugins check` regenerates `src/plugins/<id>/manifest.generated.ts`. Hand-edit only page loaders and slot `component`s in `index.ts`. |
| Contracts | Shared shapes live in `crates/contracts/*-contract`. Prefer versioned provider traits over reading another plugin's tables. |
| Deprecation | Remove a permission, event, or slot only after a release that stops advertising it; keep reading old rows until a migration cleans them up. |

Authoring SDK surface today: `platform-kernel` (Plugin trait, manifests, routes, events) + contract crates. Integration harness: `platform-testkit` (probe plugin + boot helpers). TypeScript helpers: `src/platform/registry.ts` (`defineUiPlugin`).

## Installing a plugin

```sh
cargo xtask plugins install ../my-plugin
npm run app
```

A package is a crate plus, optionally, a `ui/` directory:

```
my-plugin/
  Cargo.toml          # package name: plugin-<id>
  src/lib.rs          # must expose `pub fn plugin() -> Arc<dyn Plugin>`
  migrations/         # plugin-owned SQL
  uninstall.sql       # optional, used by `plugins remove --purge`
  ui/                 # optional Svelte descriptor + screens, copied to src/plugins/<id>
```

`install` copies the crate to `crates/plugins/<id>`, adds it to `crates/shop-api/Cargo.toml`, regenerates
`crates/shop-api/src/compose.rs`, and runs `cargo check -p shop-api`. When the package has a `ui/`
directory it is copied to `src/plugins/<id>/` and the frontend is rebuilt.

- `crates/plugins/*` and `crates/contracts/*` are workspace glob members, so the crate joins the build
  automatically.
- `src/plugins/*/index.ts` is discovered with `import.meta.glob` and must export exactly one UI plugin.
  Pages and slot components stay lazy, so code splitting is unchanged.
- `cargo xtask plugins check` fails when `compose.rs` is stale, a Rust/UI manifest disagrees, a slot
  contribution has no host, or a wired plugin directory is missing; CI runs it. `cargo xtask plugins
  list` prints the compiled plugins in dependency order.
- `cargo xtask plugins new <id> [--ui]` scaffolds a crate that compiles immediately.

A plugin added to a shop that already has plugin rows starts **disabled**. A brand-new tenant only
enables plugins whose Rust manifest sets `default_enabled = true`; experimental plugins stay off
until an operator turns them on in Settings → Plugins. Migrations still run on boot while a plugin
is disabled, which is what makes enabling instant.

**Upgrading** — append a migration, bump `PluginManifest::version`, reinstall. Never edit a migration
that has already run: the runner freezes them and refuses to boot ("Frozen migrations must not change").
On boot the stored version is adopted forward to the compiled version; a stored version newer than the
binary is a downgrade and is skipped, so the plugin stays off rather than reading data it may not know.

**Removing** — `cargo xtask plugins remove <id>` stops composing the plugin and moves the crate to
`crates/plugins-disabled/`. Its tables, `plugin_settings` row and `tenant_plugins` row stay. `--purge`
runs the plugin's optional `uninstall.sql` and deletes its bookkeeping rows; add `--embedded` when the
purge should use the desktop till's bundled database.

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
- `pos.line-source` — `{ section, lines, addLine, currency, locale }`.
  `addLine({ item_id, source, reference?, details?, … })` adds a line tied to a catalog item. A non-empty `reference` makes the line key unique, so serialized items do not merge into one quantity, and it makes the line stock-neutral: the contributing plugin owns availability and POS/inventory do not consume the catalog quantity. `lines` is the section's current cart, so a source can hide or reject an entity already on the bill.
- `pos.line-detail` — `{ line, patch, currency, locale }`.
  `patch({ source?, reference?, details? })` updates one cart line's plugin metadata.
- `pos.item-options` — `{ item, section, lines, addLine, close, currency, locale }`.
  Rendered inside the POS variant sheet. A plugin adds its own option rows and calls `addLine` for each pick. A frontend plugin that needs this outlet registers a provider with `registerItemOptionsProvider({ plugin, prepare, hasOptions, reset })`; `hasOptions(itemId)` makes clicking that catalog item open the sheet instead of adding it directly. `prepare` loads the item ids for the current shop, and `reset` drops them when the session changes.

The shell draws the top bar, the page outlet, and the settings list. It does not switch on business paths.

## Sale line metadata

`bill_lines` carries three plugin-owned columns: `source` (default `catalog.product`), `reference`, and `details JSONB`. POS stores and totals them without reading their meaning. Each line also snapshots `rate`, `unit_cost`, and `cost_total` when the bill posts, so editing an item's price or average cost later never rewrites an old bill or its reported profit. The `sales.bill_posted` event is version 3 and includes the bill facts (`bill_id`, `bill_no`, `section_id`, `user_id`, `total`, `customer`, `posted_at`) plus `SaleLine` entries (`source`, `reference`, `label`, `quantity`, `unit_amount`, `amount`, `item_id`, `stock_quantity`, `details`), so a plugin can react to its own lines without reading POS tables. `Mobile` reads this event to mark devices sold and start warranties.

## Mobile shop example

`plugin-mobile` shows the full pattern. It owns `mobile_models`, `mobile_devices`, `mobile_warranties`, and `mobile_claims`; contributes model fields to `catalog.item-form` and an IMEI chooser to `pos.item-options` (selected through the normal catalog card or search, not a separate till panel); and consumes `sales.bill_posted` v2. A phone model is a catalog item, while each physical unit is a device row with an IMEI. The device registry owns phone availability: device lines are referenced and stock-neutral, so a phone sale never changes the catalog quantity, and the picker works even when that quantity is zero. The sale handler reads `bill_lines` by `bill_id` (the event does not carry `item_id`), marks the device sold, and inserts the warranty idempotently.

A device reference identifies one physical unit, so `plugin-mobile` adds a trigger on `bill_lines` that rejects a second bill reusing `source='mobile.device', reference='device:<id>'`. The device list also derives `sold` from those lines, so a just-posted unit stops appearing in the picker even before the deferred warranty handler updates its status row.

## Settings

`settings_schema` is JSON Schema. Values are stored per tenant in `plugin_settings`. Disabling the plugin hides the values and does not delete them.

## Proof a change belongs in a plugin

A new trade (tailor jobs, a butcher's weighed sale) ships as a plugin. The pull request does not modify `platform-kernel` or `platform-runtime`.
