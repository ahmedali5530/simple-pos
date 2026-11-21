# Simple POS

One till for a shop. Each section is its own sale stream, with its own bill. The cashier works one section at a time.

## Run

Postgres must already be running. Copy `.env.example` to `.env` and set `DATABASE_URL` to that database.

```sh
cargo run -p shop-api
npm install
npm run dev
```

The till is at http://localhost:1420. The API is at http://127.0.0.1:8787.

```
DATABASE_URL=postgres://shop:shop@127.0.0.1:5433/shop
```

The shop name, currency, and locale start in `config.toml`. After the first save they live in Settings. Item rates are whole units of that currency. Changing the currency does not convert prices already stored.

`npm run app` builds the till and opens it in a desktop window with its own database.

## What it does

- Billing for every section, with barcode scan, options such as piece and pack, and weight or price labels.
- Settings for sections, categories, items, pictures, users, and the receipt.
- Stock in and out. A purchase updates the item's average cost.
- Reports for sales and profit, the stock ledger, and the product mix. Reports open on today.
- Cash closing for the open shift.
- Item and stock workbooks.

An item belongs to one section and one category. Stock value and profit use the average cost. A missing cost counts as zero.

## Platform plugins

The till is evolving into a business-neutral core with statically linked Rust plugins
(`catalog`, `inventory`, `pos`, `cash`, `printing`, `reports`). Core enforces tenancy
with Postgres row-level security, permissions, and plugin lifecycle.

Useful endpoints:

- `GET /api/v1/platform/manifest` — enabled plugins, nav and permissions
- `GET /api/v1/capabilities` — AI-callable capabilities
- `POST /api/v1/capabilities/{id}/invoke` — permission-checked capability gateway
- `GET /api/v1/mcp` — MCP tool descriptor over the same registry
- `POST /api/v1/platform/tokens` — API tokens for agents

Legacy `/api/*` routes remain for the existing Svelte UI.

Set `DATABASE_ADMIN_URL` for migrations when the runtime connects as `platform_app`.
See `.env.example`.

## License

MIT. See [LICENSE](LICENSE).
