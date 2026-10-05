#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# cspell:ignore binaryen Cpanic multivalue nontrapping numfmt vite Zfmt Zlocation Zunstable
#
# Build the interpreter as small as it gets, for a web page that shows previews of its own
# snippets (slint.dev) rather than an editor that has to accept anything (SlintPad).
#
# Compared to `wasm-pack build --release`:
#   - the code is optimized for size, with fat LTO and a single codegen unit,
#     then run through `wasm-opt -Oz`;
#   - only the styles in SLINT_COMPILER_BUILTIN_STYLES are embedded (default: fluent),
#     so `import ... from "std-widgets.slint"` works in those styles only;
#   - the default font is not embedded (the `no-embedded-font` feature). The page fetches a
#     TrueType or OpenType file itself and passes it to `register_default_font_from_memory()`
#     before compiling; the slintpad preview page does that for `preview.html?font=<url>`.
#
# With NIGHTLY=1, the standard library is rebuilt with `panic = "immediate-abort"` and without
# panic locations, which saves about another 8%. A panic then traps without a message, and
# console_error_panic_hook has nothing to print.
#
# The package goes to `pkg/` (or OUT_DIR), where the slintpad build picks it up:
#   ./build-small.sh && cd ../../tools/slintpad && pnpm vite build
# Extra arguments go to cargo.

set -euo pipefail
cd "$(dirname "$0")"

if ! command -v wasm-opt > /dev/null; then
    echo "wasm-opt is needed: install binaryen, or 'cargo install wasm-opt'" >&2
    exit 1
fi

export SLINT_COMPILER_BUILTIN_STYLES="${SLINT_COMPILER_BUILTIN_STYLES:-fluent}"
export CARGO_PROFILE_RELEASE_OPT_LEVEL=z
export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
# The settings above would otherwise rebuild everything in the shared target directory.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/../../target/wasm-small}"
out_dir="${OUT_DIR:-pkg}"

cargo_args=(--features no-embedded-font)
if [ "${NIGHTLY:-}" = 1 ]; then
    export RUSTUP_TOOLCHAIN=nightly
    export RUSTFLAGS="${RUSTFLAGS:-} -Zunstable-options -Cpanic=immediate-abort -Zlocation-detail=none -Zfmt-debug=shallow"
    cargo_args+=(-Zbuild-std=std,panic_abort)
fi

wasm-pack build --release --no-opt --target web --out-dir "$out_dir" -- "${cargo_args[@]}" "$@"

wasm="$out_dir/slint_wasm_interpreter_bg.wasm"
# The features rustc enables by default for wasm32-unknown-unknown.
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue --strip-debug --strip-producers "$wasm" -o "$wasm"
echo "$(du -h "$wasm" | cut -f1) $wasm, $(gzip -9 -c "$wasm" | wc -c | numfmt --to=iec) gzipped"
