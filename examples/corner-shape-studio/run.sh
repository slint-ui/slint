#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -eu

corner_demo_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
corner_repo_dir=$(CDPATH= cd -- "$corner_demo_dir/../.." && pwd)

export SLINT_BACKEND=winit-skia
exec cargo run --release --locked --manifest-path "$corner_repo_dir/Cargo.toml" \
    -p slint-viewer --no-default-features --features backend-winit,renderer-skia \
    -- "$@" "$corner_demo_dir/main.slint"
