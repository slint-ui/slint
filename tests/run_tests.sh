#!/bin/sh
# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -eu
unset CDPATH

usage() {
  echo "Usage: $0 <rust|cpp|interpreter|nodejs> [<filter>] [<cargo test args>...]" >&2
}

fatal() {
  printf '%s\n' "$1" >&2
  usage
  exit 1
}

if [ "$#" -lt 1 ]; then
  usage
  exit 1
fi

driver="$1"

case "$driver" in
  rust|cpp|interpreter|nodejs) ;;
  *)
    fatal "Invalid driver: $driver"
    ;;
esac

shift
filter=""
if [ "$#" -ge 1 ]; then
  filter="$1"
  shift || true
fi

# For the rust driver, auto-detect the --test <category> from the filter to avoid
# building all test binaries (which is very slow). We look for .slint files matching
# the filter under tests/cases/ and extract the subdirectory name.
test_bin_flag=""
if [ "$driver" = "rust" ] && [ -n "$filter" ]; then
  # Locate the cases/ directory relative to this script
  cases_dir="$(cd "$(dirname "$0")/cases" && pwd)"
  # Find which subdirectories contain matching .slint files
  categories=$(find "$cases_dir" -name "*${filter}*" -name "*.slint" 2>/dev/null \
    | sed "s|^${cases_dir}/||" | cut -d/ -f1 | sort -u)
  num_categories=$(printf '%s\n' "$categories" | grep -c . || true)
  if [ "$num_categories" -eq 1 ]; then
    test_bin_flag="--test $categories"
  fi
fi

SLINT_TEST_FILTER="$filter" cargo test -p "test-driver-$driver" $test_bin_flag "$@"
