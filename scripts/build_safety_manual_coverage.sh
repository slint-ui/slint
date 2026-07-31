#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Build the Slint SC Safety Manual with the measured code coverage of the
# slint-sc test suite in its Test Coverage chapter.
#
# Runs the test suite under LLVM source-based coverage instrumentation,
# exports the results, generates the manual's content with the coverage
# report, and builds the site into docs/safety/dist/.
# An HTML report with per-line detail is also written for local inspection.
#
# Requires cargo-llvm-cov, the llvm-tools-preview rustup component, and pnpm.

set -euo pipefail
cd "$(dirname "$0")/.."

coverage_dir=target/slint-sc-coverage

scripts/measure_slint_sc_coverage.sh "$coverage_dir"

cargo run -p slint-doc-generator -- --slint-sc \
    --coverage-json "$coverage_dir/coverage.json" \
    --coverage-html "$coverage_dir/html" \
    generate-mdx

pnpm install --frozen-lockfile --ignore-scripts
pnpm -C docs/safety build

echo "Safety manual built in docs/safety/dist/"
echo "Detailed HTML coverage report in $coverage_dir/html/"
