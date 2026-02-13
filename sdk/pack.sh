#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="$SCRIPT_DIR/dist"
JS_LAYER_DIR="$SCRIPT_DIR/js"

# nodejs
"$SCRIPT_DIR/build.sh" nodejs
mkdir -p "$DIST_DIR/nodejs"
mv "$DIST_DIR/hacashsdk.js" "$DIST_DIR/nodejs"
mv "$DIST_DIR/hacashsdk_bg.wasm" "$DIST_DIR/nodejs"

# web
"$SCRIPT_DIR/build.sh" web
mkdir -p "$DIST_DIR/web"
mv "$DIST_DIR/hacashsdk.js" "$DIST_DIR/web"
mv "$DIST_DIR/hacashsdk_bg.wasm" "$DIST_DIR/web"

# page
"$SCRIPT_DIR/build.sh" no-modules
node "$SCRIPT_DIR/pack.js"
mkdir -p "$DIST_DIR/page"
mv "$DIST_DIR/hacashsdk_bg.js" "$DIST_DIR/page"
cp "$SCRIPT_DIR/tests/test.html" "$DIST_DIR/page/test.html"
cp "$SCRIPT_DIR/tests/friendly_test.html" "$DIST_DIR/page/friendly_test.html"

# clean root dist artifacts
rm -f "$DIST_DIR"/*.js "$DIST_DIR"/*.ts "$DIST_DIR"/*.wasm

# js friendly layer
mkdir -p "$DIST_DIR/js"
cp "$JS_LAYER_DIR/hacashsdk.mjs" "$DIST_DIR/js/hacashsdk.mjs"
cp "$JS_LAYER_DIR/hacashsdk.cjs" "$DIST_DIR/js/hacashsdk.cjs"
cp "$JS_LAYER_DIR/hacashsdk.global.js" "$DIST_DIR/js/hacashsdk.global.js"
