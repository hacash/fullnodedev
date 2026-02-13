#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JSTARGET="${1:-nodejs}"

SDKNAME="hacashsdk"
LIBNAME="sdk"
TARGET="wasm32-unknown-unknown"
TARGET_DIR="${CARGO_TARGET_DIR:-$SCRIPT_DIR/target}"
BINARY="$TARGET_DIR/$TARGET/release/$LIBNAME.wasm"
BINARY2="$TARGET_DIR/$TARGET/release/$SDKNAME.wasm"
DIST_DIR="$SCRIPT_DIR/dist"

if ! rustup target list --installed | grep -q "^$TARGET$"; then
    rustup target add "$TARGET"
fi

if ! command -v wasm-bindgen >/dev/null 2>&1; then
    echo "wasm-bindgen CLI not found. Install with: cargo install wasm-bindgen-cli"
    exit 1
fi

# Build only sdk crate to avoid workspace-wide compilation.
cargo build \
    --manifest-path "$SCRIPT_DIR/Cargo.toml" \
    --target "$TARGET" \
    --release \
    --lib

mkdir -p "$DIST_DIR"
cp "$BINARY" "$BINARY2"
wasm-bindgen "$BINARY2" --out-dir "$DIST_DIR" --target "$JSTARGET"
