#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Render Play Store listing screenshots of the slint-viewer idle screen at
# the resolutions Google Play accepts.
#
# Usage: take-screenshots.sh [OUTPUT_DIR]
#
# Point SLINT_VIEWER_BIN at a host viewer; otherwise the script falls back to
# building target/release/slint-viewer. Requires jq and git.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
OUT_DIR="${1:-$SCRIPT_DIR/output}"

mkdir -p "$OUT_DIR"

VIEWER="${SLINT_VIEWER_BIN:-$REPO_ROOT/target/release/slint-viewer}"
if [ ! -x "$VIEWER" ]; then
    echo "Building slint-viewer (release)..."
    (cd "$REPO_ROOT" && cargo build --release -p slint-viewer)
fi

# Scale logical px in idle.slint to physical px in the PNG (3x is phone density).
export SLINT_SCALE_FACTOR="${SLINT_SCALE_FACTOR:-3}"

# SLINT_VERSION is the bare workspace version; the "Slint" prefix gets added
# at JSON time so the rendered screen shows "Slint <version>" like remote.rs.
if [ -z "${SLINT_VERSION:-}" ]; then
    SLINT_VERSION=$(awk -F'"' '/^version = / { print $2; exit }' "$REPO_ROOT/Cargo.toml")
    [ -n "$SLINT_VERSION" ] || { echo "can't read workspace version from $REPO_ROOT/Cargo.toml" >&2; exit 1; }
fi
: "${SLINT_BUILD_NUMBER:=$(git -C "$REPO_ROOT" rev-list --count HEAD)}"

DATA_FILE=$(mktemp)
trap 'rm -f "$DATA_FILE"' EXIT
# jq escapes the values so quotes/backslashes/newlines don't corrupt the JSON.
jq -n \
    --arg version "Slint $SLINT_VERSION" \
    --arg build "Build $SLINT_BUILD_NUMBER" \
    '{"ScreenshotData.slint-version": $version, "ScreenshotData.build-info": $build}' \
    > "$DATA_FILE"

for variant in PhonePortrait Tablet10; do
    out="$OUT_DIR/idle-${variant}.png"
    echo "Rendering $out"
    "$VIEWER" --component "$variant" --load-data "$DATA_FILE" \
        --screenshot "$out" "$SCRIPT_DIR/idle.slint"
done

echo "Screenshots written to $OUT_DIR"
