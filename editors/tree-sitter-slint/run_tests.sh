#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -ex

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
TS="${TREE_SITTER_CLI:-tree-sitter}"

cd "${SCRIPT_DIR}"

# Generate & build early to catch errors in the grammar quickly
$TS generate
$TS build

# Always start from a clean "generated tests" dir
rm -rf test/corpus/gen
mkdir -p ./test/corpus/gen/tests/
mkdir -p ./test/corpus/gen/examples/
mkdir -p ./test/corpus/gen/demos/

# Note: Make sure to update the ci_path_filters to include the corpus for the tree_sitter filter
find ../../tests/cases -type d -exec ./test-to-corpus.py --tests-directory {} --corpus-directory ./test/corpus/gen/tests \;
find ../../examples -type d -exec ./test-to-corpus.py --tests-directory {} --corpus-directory ./test/corpus/gen/examples \;
find ../../demos -type d -exec ./test-to-corpus.py --tests-directory {} --corpus-directory ./test/corpus/gen/demos \;

# First run the tests with -u to update all tests that can be updated
# It's okay if this fails, this means there's a parse error, but the re-run will catch this with a better output
$TS test -u > /dev/null || true;
$TS test

# Currently the tree-sitter CLI fails to update the test files if they contain ERROR nodes
# However, let's ensure there are actually no error nodes in there for good measure
# (to catch any errors if the behavior of the tree-sitter CLI changes).
#
# Note: Wrapped in separate bash, as otherwise the expanded glob pattern is printed, which is annoying
bash -c "! grep -nC10 ERROR test/corpus/gen/**/*.txt"
echo "🌳 TREE-SITTER TESTS PASSED 🎉"
