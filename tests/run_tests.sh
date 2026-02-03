# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#!/bin/sh
set -eu

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

SLINT_TEST_FILTER="$filter" cargo test -p "test-driver-$driver" "$@"
