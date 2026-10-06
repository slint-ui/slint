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
#     font itself and passes it to `register_default_font_from_memory()` before compiling;
#     the slintpad preview page does that for `preview.html?font=<url>`. The font must be
#     TrueType or OpenType: the `woff2` feature would accept WOFF2 too, but its decoder costs
#     more (90 KB gzipped) than serving a .ttf (55 KB gzipped, 48 KB with brotli);
#   - text is drawn as paths from the glyph outlines (the `outline-text` feature), which leaves
#     femtovg's glyph rasterizer out; color glyphs (emoji) aren't drawn;
#   - `@markdown` text is shown as plain text (the `no-markdown` feature), which leaves the
#     markdown parser out;
#   - the date functions behind DatePicker and TimePicker format, parse and know nothing (the
#     `no-date-time` feature), which leaves chrono out;
#   - skrifa is replaced by a copy with patches/skrifa-0.44-no-hinting.patch applied, which
#     leaves the font hinters out. Cargo.lock is restored afterwards.
#
# The nightly toolchain rebuilds the standard library with `panic = "immediate-abort"` and without
# panic locations, which saves about 8%. A panic then traps without a message, and
# console_error_panic_hook has nothing to print. NIGHTLY=0 builds with the default toolchain.
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

cargo_args=(--features no-embedded-font,outline-text,no-markdown,no-date-time)

# Replace skrifa with a patched copy, for this build only.
skrifa_src=$(cargo metadata --format-version 1 --locked | python3 -c '
import json, sys
print(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"]
           if p["name"] == "skrifa" and p["version"].startswith("0.44.")).rsplit("/", 1)[0])')
skrifa_patched="$CARGO_TARGET_DIR/patched/skrifa"
rm -rf "$skrifa_patched"
mkdir -p "$(dirname "$skrifa_patched")"
cp -r "$skrifa_src" "$skrifa_patched"
patch --quiet -p1 -d "$skrifa_patched" < patches/skrifa-0.44-no-hinting.patch
cargo_args+=(--config "patch.crates-io.skrifa.path=\"$skrifa_patched\"")
# The patch rewrites Cargo.lock's entry for skrifa; put it back.
cp ../../Cargo.lock "$CARGO_TARGET_DIR/Cargo.lock.orig"
trap 'cp "$CARGO_TARGET_DIR/Cargo.lock.orig" ../../Cargo.lock' EXIT

if [ "${NIGHTLY:-1}" = 1 ]; then
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
