//! The engine must not learn what a shop sells.
//!
//! Business nouns belong to plugins. This test fails when they appear in the
//! kernel, the runtime, or identity.

use std::fs;
use std::path::Path;

const NOUNS: &[&str] = &["item", "bill", "stock", "cash", "section", "customer"];

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn scan_file(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut hits = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        for noun in NOUNS {
            if contains_word(&lower, noun) {
                hits.push(format!("{}:{}: {noun}", path.display(), index + 1));
            }
        }
    }
    hits
}

fn contains_word(line: &str, word: &str) -> bool {
    let bytes = line.as_bytes();
    let word = word.as_bytes();
    let mut start = 0;
    while let Some(found) = line[start..].find(std::str::from_utf8(word).unwrap()) {
        let at = start + found;
        let before = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        let after = at + word.len() == bytes.len() || !bytes[at + word.len()].is_ascii_alphanumeric();
        if before && after {
            return true;
        }
        start = at + word.len();
    }
    false
}

fn scan_tree(root: &Path) -> Vec<String> {
    let mut hits = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name == "target" {
                    continue;
                }
                stack.push(path);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("rs" | "svelte" | "ts")
            ) {
                hits.extend(scan_file(&path));
            }
        }
    }
    hits
}

#[test]
fn engine_sources_have_no_business_nouns() {
    let root = repo_root();
    let mut hits = Vec::new();
    for rel in [
        "crates/platform-kernel/src",
        "crates/platform-runtime/src",
        "crates/platform-identity/src",
    ] {
        hits.extend(scan_tree(&root.join(rel)));
    }
    assert!(
        hits.is_empty(),
        "business nouns leaked into the engine:\n{}",
        hits.join("\n")
    );
}

#[test]
fn engine_crates_do_not_depend_on_business_code() {
    let root = repo_root();
    for rel in [
        "crates/platform-kernel/Cargo.toml",
        "crates/platform-runtime/Cargo.toml",
        "crates/platform-identity/Cargo.toml",
    ] {
        let text = fs::read_to_string(root.join(rel)).expect(rel);
        let deps = text.split("[dev-dependencies]").next().unwrap_or(&text);
        for banned in ["shop-core", "plugin-", "catalog-contract", "inventory-contract", "sales-contract"] {
            assert!(
                !deps.contains(banned),
                "{rel} depends on {banned}"
            );
        }
    }
}

#[test]
fn the_legacy_shop_core_crate_is_gone() {
    let root = repo_root();
    assert!(
        !root.join("crates/shop-core").exists(),
        "shop-core was removed; business code lives in plugins"
    );
    let host = fs::read_to_string(root.join("crates/shop-api/Cargo.toml")).unwrap();
    assert!(!host.contains("shop-core"), "shop-api must not depend on shop-core");
    let source = fs::read_to_string(root.join("crates/shop-api/src/lib.rs")).unwrap();
    for legacy in [
        "\"/api/items",
        "\"/api/sections",
        "\"/api/categories",
        "\"/api/settings",
        "\"/api/print",
        "\"/api/stock",
        "\"/api/pos\"",
    ] {
        assert!(!source.contains(legacy), "shop-api still serves {legacy}");
    }
}
