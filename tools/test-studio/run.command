#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -eu
studio_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$studio_dir"
if [ ! -x .venv/bin/python ]; then
    echo "Studio environment missing. Run: cd \"$studio_dir\" && uv sync --locked" >&2
    exit 1
fi
exec .venv/bin/python app.py "$@"
