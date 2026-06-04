#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -ex

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
TS="${TREE_SITTER_CLI:-tree-sitter}"

cd "${SCRIPT_DIR}"

# Always start from a clean "generated tests" dir
rm -rf test/corpus/gen
mkdir -p ./test/corpus/gen/tests/
mkdir -p ./test/corpus/gen/examples/
mkdir -p ./test/corpus/gen/demos/

# Note: Make sure to update the ci_path_filters to include the corpus for the tree_sitter filter
find ../../tests/cases -type d -exec ./test-to-corpus.py --tests-directory {} --corpus-directory ./test/corpus/gen/tests \;
find ../../examples -type d -exec ./test-to-corpus.py --tests-directory {} --corpus-directory ./test/corpus/gen/examples \;
find ../../demos -type d -exec ./test-to-corpus.py --tests-directory {} --corpus-directory ./test/corpus/gen/demos \;

$TS generate
$TS build
# First run the tests with -u to update all tests that can be updated
# It's okay if this fails, this means there's a parse error, but the re-run will catch this with a better output
$TS test -u > /dev/null || true;
$TS test
