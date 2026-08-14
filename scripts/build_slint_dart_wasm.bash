#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Build `slint_dart` for the web: a WebAssembly module plus the JavaScript
# module that loads it.
#
# Flutter web has no `dart:ffi`, so `package:slint` reaches the runtime through
# the `wasm-bindgen` entry points in `api/flutter/rust/wasm.rs` instead. Copy
# both output files into a Flutter application's `web/` directory and pass the
# JavaScript one to `initSlint()`.
#
#   scripts/build_slint_dart_wasm.bash examples/todo/flutter/web

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
STAGE="$TARGET_DIR/wasm-web"
DESTINATION="${1:-}"

# Only the software renderer, the same set the Apple slices use: the browser
# gets its frames from the embedded surface in `api/flutter/rust/embedded.rs`,
# which Flutter paints into the widget tree. Winit comes along regardless —
# `slint-interpreter` depends on it on wasm — but nothing calls it.
FEATURES="${SLINT_DART_FEATURES:---no-default-features --features renderer-software}"

if ! command -v wasm-pack >/dev/null; then
  echo "wasm-pack is required: cargo install wasm-pack" >&2
  exit 1
fi

if ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  echo "adding the wasm32-unknown-unknown target" >&2
  rustup target add wasm32-unknown-unknown
fi

wasm-pack build "$ROOT/api/flutter" \
  --target web \
  --out-dir "$STAGE" \
  --out-name slint_dart \
  --release \
  --no-typescript \
  --no-pack \
  -- $FEATURES

# `wasm-pack` writes a package the web build has no use for; the loader and the
# module are the whole payload.
rm -f "$STAGE/.gitignore" "$STAGE/package.json" "$STAGE/README.md"

if [ -n "$DESTINATION" ]; then
  mkdir -p "$DESTINATION"
  cp "$STAGE/slint_dart.js" "$STAGE/slint_dart_bg.wasm" "$DESTINATION"
  echo "copied slint_dart.js and slint_dart_bg.wasm to $DESTINATION" >&2
else
  echo "built $STAGE/slint_dart.js and $STAGE/slint_dart_bg.wasm" >&2
fi
