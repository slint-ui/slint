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

# --remap-path-prefix makes the reports use workspace-relative paths, so the
# safety manual's per-line links don't depend on the checkout location.
SLINT_TEST_REPORT="$PWD/$results/driver.json" CARGO_TERM_COLOR=never \
    cargo llvm-cov --no-report --remap-path-prefix -p slint-sc 2>&1 \
    | tee "$results/runtime-tests.log"

# The full export (not --summary-only): the per-function region counts feed
# the fully/partially/untested statistics in the safety manual.
cargo llvm-cov report --remap-path-prefix --json --output-path "$out/coverage.json"
cargo llvm-cov report --remap-path-prefix --lcov --output-path "$out/lcov.info"
cargo llvm-cov report --remap-path-prefix --html --output-dir "$out"
