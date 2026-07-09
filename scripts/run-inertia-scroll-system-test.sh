#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

export SLINT_EMIT_DEBUG_INFO=1

cargo build -p inertia-scroll-probe --features slint/system-testing

export INERTIA_SCROLL_PROBE_BIN="${INERTIA_SCROLL_PROBE_BIN:-$REPO_ROOT/target/debug/inertia-scroll-probe}"
export UV_INDEX="${UV_INDEX:-slint-private=https://testing.slint.dev/simple/}"
export UV_INDEX_SLINT_PRIVATE_USERNAME="${UV_INDEX_SLINT_PRIVATE_USERNAME:-__token__}"

if [[ -z "${UV_INDEX_SLINT_PRIVATE_PASSWORD:-}" ]]; then
    echo "Set UV_INDEX_SLINT_PRIVATE_PASSWORD to your Slint testing token." >&2
    exit 2
fi

uv run --project tools/inertia-scroll-testing pytest tools/inertia-scroll-testing/tests "$@"
