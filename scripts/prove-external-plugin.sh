#!/usr/bin/env bash
# Install an out-of-tree sample plugin, validate composition, then remove it.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

fixture="$root/fixtures/external-hello"
plugin_dir="$root/crates/plugins/hello"
ui_dir="$root/src/plugins/hello"

cleanup() {
  unset CHASKA_XTASK_FAIL_AT || true
  if [[ -d "$plugin_dir" || -d "$ui_dir" || -d "$root/crates/plugins-disabled/hello" ]]; then
    cargo xtask plugins remove hello --keep-source >/dev/null 2>&1 || true
    rm -rf "$plugin_dir" "$ui_dir" "$root/crates/plugins-disabled/hello"
    cargo xtask plugins sync >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

echo "== external plugin install =="
cargo xtask plugins install "$fixture" --force

echo "== composition check =="
cargo xtask plugins check

echo "== smoke list =="
cargo xtask plugins list | tee /tmp/chaska-plugin-list.txt
grep -q '^hello ' /tmp/chaska-plugin-list.txt

echo "== remove =="
cargo xtask plugins remove hello
test ! -d "$plugin_dir"
test ! -d "$ui_dir"

echo "External plugin proof passed."
