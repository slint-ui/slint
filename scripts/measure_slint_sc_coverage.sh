#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Measure the code coverage of the slint-sc runtime and write the reports
# to the directory given as the first argument: coverage.json, lcov.info,
# and html/ with per-line execution counts.
#
# Only the runtime crate is measured: its unit tests and the test driver,
# which compiles and runs the .slint test cases against the instrumented
# runtime. The docs build and build_safety_manual_coverage.sh both call
# this script so the measured suite can't drift between them.
#
# Requires cargo-llvm-cov and the llvm-tools-preview rustup component.

set -euo pipefail
cd "$(dirname "$0")/.."

out="${1:?usage: $0 <output-dir>}"

cargo llvm-cov clean --workspace

# --remap-path-prefix makes the reports use workspace-relative paths, so the
# safety manual's per-line links don't depend on the checkout location.
cargo llvm-cov --no-report --remap-path-prefix -p slint-sc

mkdir -p "$out"
# The full export (not --summary-only): the per-function region counts feed
# the fully/partially/untested statistics in the safety manual.
cargo llvm-cov report --remap-path-prefix --json --output-path "$out/coverage.json"
cargo llvm-cov report --remap-path-prefix --lcov --output-path "$out/lcov.info"
cargo llvm-cov report --remap-path-prefix --html --output-dir "$out"
