#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Build the Slint SC Safety Manual with the slint-sc test results and the
# measured code coverage in its Test Results and Test Coverage chapters.
#
# Runs the test suites, exports the results and coverage, generates the
# manual's content, and builds the site into docs/safety/dist/.
# An HTML report with per-line detail is also written for local inspection.
#
# Fails when the runtime isn't completely covered or a requirement paragraph
# is declared by no test, so run this before pushing a change to slint-sc.
#
# Requires cargo-llvm-cov, the llvm-tools-preview rustup component, and pnpm.

set -euo pipefail
cd "$(dirname "$0")/.."

coverage_dir=target/slint-sc-coverage

scripts/slint_sc_test_suite.sh "$coverage_dir"

# A gap exits 2 with the chapters written, so build the manual before failing:
# the reports of the run that found the gap are then there to look at.
set +e
cargo run -p slint-doc-generator -- --slint-sc \
    --coverage-json "$coverage_dir/coverage.json" \
    --coverage-html "$coverage_dir/html" \
    --test-results "$coverage_dir/test-results" \
    --fail-on-gaps \
    generate-mdx
status=$?
set -e
if [ $status -ne 0 ] && [ $status -ne 2 ]; then
    exit $status
fi

pnpm install --frozen-lockfile --ignore-scripts
pnpm -C docs/safety build

echo "Safety manual built in docs/safety/dist/"
echo "Detailed HTML coverage report in $coverage_dir/html/"

if [ $status -eq 2 ]; then
    echo "The safety manual has gaps; see the generator output above." >&2
    exit 1
fi
