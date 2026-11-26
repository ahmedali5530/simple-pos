//! Static guard for the Phase 2 plugin boundary.
//!
//! Stock and average cost live on Inventory's `inventory_balances`. Catalog may
//! LEFT JOIN that table when enriching snapshots and sheet exports. Inventory
//! may still read Catalog's `items` for names via CatalogQuery, not for stock.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn plugin_ids(root: &Path) -> Vec<String> {
    let mut ids: Vec<_> = fs::read_dir(root.join("crates/plugins"))
        .expect("plugins directory")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids
}

#[test]
fn production_plugins_do_not_depend_on_other_plugins() {
    let root = repo_root();
    for id in plugin_ids(&root) {
        let manifest = fs::read_to_string(root.join(format!("crates/plugins/{id}/Cargo.toml")))
            .expect("plugin manifest");
        let production = manifest
            .split("[dev-dependencies]")
            .next()
            .unwrap_or(&manifest);
        for line in production.lines() {
            if let Some(name) = line.trim().strip_prefix("plugin-") {
                assert!(
                    !name.starts_with(|character: char| character.is_ascii_alphanumeric()),
                    "crates/plugins/{id} depends on another plugin: {}",
                    line.trim()
                );
            }
        }
    }
}

fn allowed_tables(plugin: &str, table: &str) -> bool {
    match table {
        "bills" | "bill_lines" | "bill_tenders" => plugin == "pos",
        "stock_movements" => plugin == "inventory",
        // Catalog reads balances for snapshot/sheet enrichment; only Inventory writes them.
        "inventory_balances" => plugin == "inventory" || plugin == "catalog",
        "items" | "sections" | "categories" | "item_variants" | "catalog_units" => {
            plugin == "catalog"
        }
        // Core-owned tables: identity, settings, tenancy.
        "users"
        | "sessions"
        | "tenant_profile"
        | "tenants"
        | "role_permissions"
        | "roles"
        | "platform_permissions"
        | "platform_settings"
        | "plugin_settings" => true,
        _ => true,
    }
}

#[test]
fn plugins_only_query_tables_they_own() {
    let root = repo_root();
    let statements = ["FROM ", "JOIN ", "UPDATE ", "INSERT INTO "];
    for id in plugin_ids(&root) {
        let source_root = root.join(format!("crates/plugins/{id}/src"));
        let mut stack = vec![source_root];
        while let Some(directory) = stack.pop() {
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }
                let text = fs::read_to_string(&path).unwrap_or_default();
                for (line_number, line) in text.lines().enumerate() {
                    let upper = line.to_ascii_uppercase();
                    for statement in statements {
                        let Some(position) = upper.find(statement) else {
                            continue;
                        };
                        let rest = &line[position + statement.len()..];
                        let table: String = rest
                            .chars()
                            .skip_while(|character| character.is_whitespace())
                            .take_while(|character| {
                                character.is_ascii_alphanumeric() || *character == '_'
                            })
                            .collect();
                        if table.is_empty() {
                            continue;
                        }
                        // SQL can be split across lines; only check lines with a concrete table.
                        if !table
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_alphabetic())
                        {
                            continue;
                        }
                        assert!(
                            allowed_tables(&id, &table),
                            "{}:{}: plugin `{id}` queries `{table}`",
                            path.display(),
                            line_number + 1
                        );
                    }
                }
            }
        }
    }
}
