# Plugin platform implementation plan

This roadmap tracks work toward a composable, statically linked plugin platform. Native Rust
plugins are trusted code; install and enable are separate operations. Runtime upload of arbitrary
code is out of scope until an isolation boundary exists.

## Phase 0: make the current plugin workflow safe

- [x] Keep `xtask` in the workspace and verify composition.
- [x] Make install/remove transactional, validate backend and UI descriptors, and validate the
  composed catalog after removal.
- [x] Use explicit plugin defaults, enforce permissions, and keep capability exposure consistent
  with the enabled plugin set.
- [x] Define compatibility behavior for manifest and kernel API changes (manifest v2,
  `#[non_exhaustive]` on `PluginManifest`, kernel API range, and forward persisted version
  adoption; a downgrade is refused).

Acceptance: failed operations leave the working tree unchanged; `plugins check` catches broken
composition and mismatched plugin declarations; a fresh tenant only enables explicitly opted-in
plugins.

Verification: `cargo xtask plugins check` (composition, backend/UI parity, slot hosts),
`cargo check --workspace --all-targets`, shell-only/catalog-only/web feature builds, `tsc --noEmit`,
`node scripts/check-plugin-ui.mjs`, the static plugin-boundary guard, and `cargo test --workspace`
against a disposable database all pass. The full web binary serves `/` (SPA) and `/api/bootstrap` without the old fallback panic. The
workspace transaction helper has rollback, commit, and new-parent cleanup tests. CI now runs `cargo run -p shop-api -- --migrate-only` before
the test suite because the platform integration tests expect the legacy `shop-api` schema first.
Verification also covers empty-database bootstrap (`shop-api` `empty_bootstrap` test) and xtask
install rollback via `CHASKA_XTASK_FAIL_AT` fault injection. CI runs shell-only and catalog-only
`cargo check` plus `npm run typecheck`.

## Phase 1: make the host independent of business plugins

- [x] Move identity and platform shop profile ownership to platform crates.
- [x] Remove POS handler imports from the API host and retire POS routes from core; compile-time
  composition now selects plugins behind optional features.
- [x] Boot the shell with no business plugins and with POS. A catalog-only build also compiles.

User/session persistence is owned by `platform-identity`; the core profile is stored and served by
`platform-runtime`; POS layout, bill-prefix, and barcode settings are plugin-owned. The catalog-only
variant passed its composition test. Core exposes profile-only `ShopSnapshot`; POS owns
`PosSettings` / `TillShop` for till fields. Printing loads core shop profile data directly and
printer settings are no longer part of the POS snapshot. Characterization tests now build the real
platform-backed router instead of the legacy-only router.

## Phase 2: use contracts across plugin boundaries

- [x] Retain typed providers on `Platform` and resolve contracts through them.
- [x] Remove registered veto/print contracts that had no host.
- [x] Replace remaining direct plugin table/crate reads with query/ledger contracts and domain events.
- [x] Make declared dependencies match runtime requirements in every plugin.

Stock issuing and inventory reporting now use providers. POS is standalone: catalog and inventory are
optional runtime providers (`CatalogApi` / `StockLedger`), not hard `requires`, so another catalog
plugin can supply the till later. POS reads and writes catalog data only through `CatalogQuery`
(`CatalogSnapshot` for the till, the sale report, layout and focus); it no longer reads or writes
`items`/`sections`/`categories` directly. Inventory's stock summary,
product-mix and report paths use the same snapshot. Cash stores the operator names on the shift, so
its history no longer joins `users`. Reports is split: POS supplies
`SalesReporting` (sections, bills, profit) and Inventory supplies `StockReport` (valuation,
movements, product mix); the Reports plugin composes them and no longer reads either plugin's
tables. Mobile consumes richer bill-posted events and validates models through `CatalogQuery`
instead of reading `items`. Cash gets sales totals through `SalesQuery`. Printing requests receipt
data through `SalesReceiptQuery`, product mix through `InventoryReporting`, and cash closing through
`CashClosingQuery`; its POS/Inventory/Cash links are optional provider dependencies. Stock movements
now carry `bill_no`, so Inventory and the Catalog workbook exporter no longer join the POS `bills`
table. Shop name, currency, and locale come from the core-owned `ShopProfile`.

The stock-movement and item workbooks are now provider-backed: Inventory supplies movement rows,
applies workbook imports, and applies item-workbook stock deltas; Catalog only formats/parses Excel.
Catalog declares POS optional for the delete-section provider and Inventory optional for the
workbook contract.

A static boundary guard (`crates/shop-api/tests/plugin_boundaries.rs`) now fails if a plugin
production crate depends on another plugin or queries a table it does not own; it explicitly allows
Inventory's stock/avg_cost access as the documented exception.

Stock data ownership is done: `stock` and `avg_cost` live on Inventory's `inventory_balances`.
Catalog migration 005 syncs and drops those columns from `items`; Catalog may LEFT JOIN balances
when enriching snapshots and sheet exports, and writes cost through `StockLedger::ensure_balance`.
Everything else is contract/event/provider-bound.

The dormant `DeletionVeto` and `Printable` hooks were removed instead of being advertised as
extension points; the unused `printing-contract` crate was removed. Receipt rendering and dispatch
remain an internal Printing implementation, not an extension API. The kernel purity test now passes:
the default-enabled baseline lives on each plugin manifest, and cashier defaults live as a
permission catalog flag rather than engine code.

## Phase 3: stabilize plugin authoring

- [x] Generate backend composition and frontend registration from one manifest source
  (Rust `PluginManifest` → `manifest.generated.ts`; page loaders stay hand-authored).
- [x] Document compatibility ranges / deprecation rules; ship `platform-testkit` probe harness
  and keep `defineUiPlugin` as the TypeScript helper surface.
- [x] Verify an external clean-repo plugin can install, build, and remove
  (`fixtures/external-hello` + `scripts/prove-external-plugin.sh` in CI).

## Phase 4: prove meaningful extension

- Support filters, tenders, receipt blocks, reporting widgets, settings sections, and generic line
  policies through host-owned contracts.
- Complete Installments or a gift-card example without editing POS or core.

## Phase 5: lifecycle and trust

- Add package metadata, checksums, upgrade/rollback, and migration backups.
- Decide whether installation is a deployment pipeline or runtime admin operation.
- Document native trusted plugin privileges; require sandboxing for untrusted marketplace code.
- Persist per-tenant install/enable/config and role permission assignments.

## Decisions and deferred scope

- Keep native plugins statically linked for now; this is a deployment model, not runtime isolation.
- Prefer typed domain contracts over generic table access or a universal hook bus.
- Keep install/upgrade in the operator deployment pipeline until package signing, migrations,
  rollback, and trust policy are implemented.
- Treat package marketplace, WASM/process isolation, and multi-tenant plugin administration as
  later phases; they depend on the contract and lifecycle foundations above.

## Decisions for project review

The existing direction is sound: statically linked trusted plugins, explicit dependencies,
tenant-scoped data, and typed provider/event contracts are a reasonable first platform. The
following choices should be decided before implementation reaches those phases:

- Is the supported plugin install path a deployment pipeline that rebuilds the app, or runtime
  installation? The Rust composition currently implies deployment-time installation.
- Should experimental plugins default off universally, or can an operator mark a plugin on for
  fresh tenants? The current implementation uses a core baseline allowlist.
- Is third-party code trusted native Rust, or must the platform eventually accept sandboxed code?
  Native code must not be described as tenant-isolated.
- Is a generic filter/interceptor API needed now? Prefer specific contracts until real plugins
  demonstrate a repeated cross-cutting need.
- Which plugin-facing contracts are public and versioned? The API and event payloads need an
  explicit compatibility policy before an external SDK is published.
