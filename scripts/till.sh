#!/bin/sh
cd "$(dirname "$0")/.." || exit 1
export PATH="/usr/bin:/bin:${HOME}/.cargo/bin:${PATH}"
export POSTGRESQL_VERSION="${POSTGRESQL_VERSION:-=18.7.0}"
export PKG_CONFIG_PATH="${HOME}/.local/tauri-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig:${HOME}/.local/tauri-sysroot/usr/lib/pkgconfig:${HOME}/.local/tauri-sysroot/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
npm run build && cargo run --manifest-path src-tauri/Cargo.toml
