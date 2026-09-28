#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Regenerate the slint-viewer store-listing screenshot: the idle "Connect Live
# Preview to ..." screen, rendered in headless mode with `slint-viewer --screenshot`.
#
# Set SLINT_VIEWER_BIN to a prebuilt host viewer; otherwise a debug build is
# used. Requires jq, and uses optipng to shrink the PNG if present.
# Set SLINT_BUILD_NUMBER to also show a build line.
#
# Usage: make-screenshots.sh [OUTPUT_PNG]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MAIN="$REPO_ROOT/tools/viewer/remote/main.slint"
OUT="${1:-$REPO_ROOT/tools/viewer/android/fastlane/metadata/android/en-US/images/phoneScreenshots/1.png}"

VIEWER="${SLINT_VIEWER_BIN:-$REPO_ROOT/target/debug/slint-viewer}"
if [ ! -x "$VIEWER" ]; then
    (cd "$REPO_ROOT" && cargo build -p slint-viewer)
fi

# The bare workspace version; the "Slint" prefix matches what remote.rs shows.
VERSION="${SLINT_VERSION:-$(awk -F'"' '/^version = / { print $2; exit }' "$REPO_ROOT/Cargo.toml")}"
# Left empty by default so the committed screenshot stays stable between builds.
BUILD_INFO=""
[ -n "${SLINT_BUILD_NUMBER:-}" ] && BUILD_INFO="Build $SLINT_BUILD_NUMBER"

# jq escapes the values so quotes or backslashes can't corrupt the JSON.
DATA=$(mktemp)
trap 'rm -f "$DATA"' EXIT
jq -n --arg name "Pixel 9" --arg address "192.168.1.42:8765" \
    --arg version "Slint $VERSION" --arg build "$BUILD_INFO" \
    '{name: $name, address: $address, "slint-version": $version, "build-info": $build}' \
    > "$DATA"

mkdir -p "$(dirname "$OUT")"
# 3x density renders the 360x800 logical idle screen at 1080x2400 px.
SLINT_SCALE_FACTOR=3 "$VIEWER" --component RemoteViewerWindow --size 360x800 \
    --load-data "$DATA" --screenshot "$OUT" "$MAIN"

# Shrink the PNG losslessly; the pixels are unchanged.
if command -v optipng > /dev/null; then
    optipng -quiet -o5 "$OUT"
fi

echo "Wrote $OUT"
