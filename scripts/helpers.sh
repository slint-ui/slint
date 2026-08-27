#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

log() {
    echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"
}

die() {
    echo "error: $*" >&2
    exit 1
}

require_env() {
    local name
    for name in "$@"; do
        if [ -z "${!name:-}" ]; then
            die "$name is required"
        fi
    done
}

abs_path() {
    local path="$1"
    local dir

    dir="$(cd "$(dirname "$path")" && pwd)" || return 1
    printf "%s/%s\n" "$dir" "$(basename "$path")"
}
