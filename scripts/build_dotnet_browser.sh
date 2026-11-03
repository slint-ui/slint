#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Builds slint-dotnet as a static library for wasm32-unknown-emscripten into target/browser/slint_dotnet.a,
# which Slint.targets links into browser-wasm apps.
# Needs rustup's wasm32-unknown-emscripten target and llvm-tools component.
# Usage: build_dotnet_browser.sh [cargo profile, release by default]

set -eu

profile=${1:-release}
cd "$(dirname "$0")/.."
dir=$profile
[ "$profile" = dev ] && dir=debug
export CARGO_TARGET_DIR=target/emscripten
# The app links every import of Slint.dll statically, so the library needs the testing exports too.
cargo rustc --profile "$profile" -p slint-dotnet --target wasm32-unknown-emscripten \
    --no-default-features --features testing --crate-type staticlib

# The Emscripten of .NET 10 rejects feature names that are newer than it, which Rust's standard library has.
ar=$(ls "$(rustc --print sysroot)"/lib/rustlib/*/bin/llvm-ar)
objects=target/browser/objects
rm -rf "$objects" target/browser/slint_dotnet.a
mkdir -p "$objects"
(cd "$objects" && "$ar" x "../../emscripten/wasm32-unknown-emscripten/$dir/libslint_dotnet.a")
python3 -I scripts/strip_wasm_features.py "$objects"/*.o
"$ar" rcs target/browser/slint_dotnet.a "$objects"/*
rm -rf "$objects"
