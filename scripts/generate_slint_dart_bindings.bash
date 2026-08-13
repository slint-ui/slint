#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Regenerate the Dart FFI bindings for the `slint-dart` C ABI.
#
#   api/flutter/rust  --cbindgen-->  target/slint_dart.h
#                     --ffigen---->  api/flutter/slint/lib/src/ffi.g.dart
#
# The generated file is committed, so `--check` verifies it still matches the
# Rust sources. Without that check the two sides could drift apart again,
# which is the whole reason the bindings are generated.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE="$ROOT/api/flutter/slint"
HEADER="${CARGO_TARGET_DIR:-$ROOT/target}/slint_dart.h"
GENERATED="$PACKAGE/lib/src/ffi.g.dart"

check=0
if [ "${1:-}" = "--check" ]; then
    check=1
elif [ $# -gt 0 ]; then
    echo "usage: $(basename "$0") [--check]" >&2
    exit 2
fi

# cbindgen is a workspace dependency but not a build-time one, so it has to be
# on PATH or in the standard cargo location.
cbindgen=cbindgen
if ! command -v "$cbindgen" >/dev/null 2>&1; then
    cbindgen="$HOME/.cargo/bin/cbindgen"
fi
if [ ! -x "$cbindgen" ] && ! command -v "$cbindgen" >/dev/null 2>&1; then
    echo "cbindgen not found. Install it with: cargo install cbindgen" >&2
    exit 1
fi

# The package pins its Dart SDK with FVM; fall back to a plain dart on PATH.
dart=(dart)
if command -v fvm >/dev/null 2>&1 && [ -f "$PACKAGE/.fvmrc" ]; then
    dart=(fvm dart)
fi

mkdir -p "$(dirname "$HEADER")"
# The crate directory is passed explicitly: cbindgen otherwise resolves it
# from the working directory, which is wherever this script was invoked.
"$cbindgen" --config "$ROOT/api/flutter/cbindgen.toml" \
    --crate slint-dart --output "$HEADER" --quiet "$ROOT/api/flutter"

if [ "$check" -eq 1 ]; then
    # ffigen only writes to the path in ffigen.yaml, so generate in place and
    # restore the committed file afterwards either way.
    backup="$(mktemp)"
    cp "$GENERATED" "$backup"
    trap 'cp "$backup" "$GENERATED"; rm -f "$backup"' EXIT
fi

(cd "$PACKAGE" && "${dart[@]}" run ffigen --config ffigen.yaml >/dev/null)

if [ "$check" -eq 1 ]; then
    if ! diff -u "$backup" "$GENERATED"; then
        echo >&2
        echo "ffi.g.dart is out of date with api/flutter/rust." >&2
        echo "Regenerate it with $(basename "$0") and commit the result." >&2
        exit 1
    fi
    echo "ffi.g.dart is up to date."
else
    echo "$GENERATED"
fi
