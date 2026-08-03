#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Run the slint-sc test suites and measure the runtime's code coverage.
# Writes into the directory given as the first argument:
#   test-results/  per-suite logs and CTRF-style JSON reports
#   coverage.json, lcov.info, html/  the coverage reports
#
# The compiler suites run uninstrumented; coverage measures only the runtime
# crate: its unit tests and the test driver, which compiles and runs the
# .slint test cases against the instrumented runtime. The docs build and
# build_safety_manual_coverage.sh both call this script so the tested and
# measured suite can't drift between them.
#
# Requires cargo-llvm-cov and the llvm-tools-preview rustup component.

set -euo pipefail
cd "$(dirname "$0")/.."

out="${1:?usage: $0 <output-dir>}"
results="$out/test-results"
mkdir -p "$results"

# SLINT_TEST_REPORT names one report file per cargo invocation; only one
# harness per invocation may honor it. The captured logs are parsed by
# slint-doc-generator, so keep them free of color escapes.
SLINT_TEST_REPORT="$PWD/$results/syntax-tests.json" CARGO_TERM_COLOR=never \
    cargo test -p i-slint-compiler --features slint-sc --no-default-features 2>&1 \
    | tee "$results/compiler-tests.log"
cargo test -p slint-compiler --features slint-sc --no-default-features

cargo llvm-cov clean --workspace

# Each .slint case is compiled into its own test binary that statically links
# the runtime, so the coverage it produces is recorded against that binary's
# copy of the crate, not the crate's own build. Keep those binaries so the
# report below can attribute their coverage; without them only the runtime's
# unit tests would count. The directory is rebuilt every run so cases that
# were removed don't leave stale binaries behind.
bins="$PWD/$out/test-bins"
rm -rf "$bins"

# --remap-path-prefix makes the reports use workspace-relative paths, so the
# safety manual's per-line links don't depend on the checkout location.
SLINT_TEST_REPORT="$PWD/$results/driver.json" SLINT_SC_COV_BIN_DIR="$bins" CARGO_TERM_COLOR=never \
    cargo llvm-cov --no-report --remap-path-prefix -p slint-sc 2>&1 \
    | tee "$results/runtime-tests.log"

# Report with llvm-cov directly rather than `cargo llvm-cov report`, which
# only knows the binaries cargo built: the object set is the crate's unit-test
# binary (its coverage map lists every runtime function, so untested ones are
# counted) plus the per-case test binaries (their coverage from the integration
# tests). The tools ship with the toolchain.
llvm_bin="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin"
profdata="$out/coverage.profdata"
"$llvm_bin/llvm-profdata" merge -sparse -o "$profdata" target/llvm-cov-target/*.profraw

unit_bin=$(find target/llvm-cov-target/debug/deps -maxdepth 1 -type f -executable \
    -name 'slint_sc-*' ! -name '*.d' -printf '%T@ %p\n' | sort -rn | head -1 | cut -d' ' -f2-)
objects=(-object "$unit_bin")
for bin in "$bins"/*; do
    objects+=(-object "$bin")
done

# Keep only the runtime sources: the generated and test code compiled into the
# per-case binaries lives at absolute paths or under tests/.
ignore='(^/|(^|/)tests/|(^|/)[A-Za-z0-9_-]+[_-]?tests\.rs$|^target/)'

# The full export (not --summary-only): the per-function region counts feed
# the fully/partially/untested statistics in the safety manual.
"$llvm_bin/llvm-cov" export -format=text -instr-profile="$profdata" \
    -ignore-filename-regex="$ignore" "${objects[@]}" > "$out/coverage.json"
"$llvm_bin/llvm-cov" export -format=lcov -instr-profile="$profdata" \
    -ignore-filename-regex="$ignore" "${objects[@]}" > "$out/lcov.info"
"$llvm_bin/llvm-cov" show -format=html -output-dir="$out/html" -instr-profile="$profdata" \
    -ignore-filename-regex="$ignore" "${objects[@]}"
