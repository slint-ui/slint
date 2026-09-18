#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cspell:ignore getconf NPROCESSORS ONLN worksteal

set -eu

if [ "${1:-}" = "--visible" ]; then
    shift
    export SLINT_EDITOR_UI_TEST_BACKEND=winit-skia
else
    worker_count=$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '1\n')
    if [ "$worker_count" -gt 4 ]; then
        worker_count=4
    fi
    set -- -n "$worker_count" --dist=worksteal "$@"
fi

UV_CACHE_DIR="${UV_CACHE_DIR:-/private/tmp/slint-editor-ui-uv-cache}" \
uv run --no-sync pytest -ra "$@"
