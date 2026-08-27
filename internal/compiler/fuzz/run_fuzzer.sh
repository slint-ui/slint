#!/bin/bash
# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Fuzz the Slint compiler: parse + all middle-end passes + LLR lowering + codegen.
#
# Effective fuzzing here needs a good starting point, so on startup this script
# generates a seed corpus from the real .slint files in the test suites. Random
# byte mutation almost never discovers valid Slint grammar from scratch, so
# without seeds the fuzzer just bounces off the lexer/parser.
#
#   seeds/      - generated here from tests/**/*.slint (gitignored, regenerable).
#   slint.dict  - Slint keywords/operators spliced into mutations (checked in).
#   corpus/     - mutated in place by libFuzzer, grows over time (gitignored).

set -e
unset CDPATH
FUZZ_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$FUZZ_DIR/../../.." && pwd)
SEEDS="$FUZZ_DIR/seeds"

# (Re)generate the seed corpus: every .slint file <=16k from the test suites.
# Bigger files just slow each iteration without adding grammar coverage.
echo "Generating seed corpus in $SEEDS ..."
rm -rf "$SEEDS"
mkdir -p "$SEEDS"
n=0
while IFS= read -r f; do
    [ "$(stat -c%s "$f")" -gt 16384 ] && continue
    cp "$f" "$SEEDS/$(echo "$f" | sed "s#$ROOT/##; s#/#_#g")"
    n=$((n + 1))
done < <(find "$ROOT/tests/cases" "$ROOT/internal/compiler/tests" -name '*.slint')
echo "Seeded $n files."

CORPUS_DIR="$FUZZ_DIR/corpus/compiler_fuzzing"
mkdir -p "$CORPUS_DIR"

# Needs +nightly because cargo-fuzz enabled address sanitizer
cargo +nightly fuzz run --fuzz-dir "$FUZZ_DIR" compiler_fuzzing \
    "$CORPUS_DIR" \
    "$SEEDS" \
    -- \
    -dict="$FUZZ_DIR/slint.dict" \
    -max_len=16384 \
    -runs=10000000
