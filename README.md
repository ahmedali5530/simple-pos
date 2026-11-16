# Chaska Corner

One till for a shop that sells retail and food. Each section has its own bill. The cashier chooses how many sections this screen shows.

## Run

```sh
docker start chaska-pos-db
cargo run -p chaska-api
npm install
npm run dev
```

The till is at http://localhost:1420. The API is at http://127.0.0.1:8787.

Postgres for this app listens on port 5433 (`postgres://chaska:chaska@127.0.0.1:5433/chaska_pos`).

Shop name, currency, and locale are in `config.toml`. Item rates are whole units of that currency. Changing the currency does not convert prices already stored.

## What is built

Billing is the working screen. Sections, categories, and items live in Postgres. A section is a column on the till. A category is a filter inside that column. An item belongs to one section and one category.

Still to build: the control panel (create and edit sections, categories, and items), stock in/out, reports, and the Tauri window for the Windows touch PC.
