#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Generate the slint-viewer iOS app icon asset catalog from the Slint logo SVG.
# This creates the whole Assets.xcassets/AppIcon.appiconset (so none of it is
# checked in) and runs as an Xcode pre-build script, before the catalog is
# compiled. Can also be invoked standalone. Requires rsvg-convert from librsvg.

set -euo pipefail

# Homebrew locations, so this works from Xcode's stripped-down PATH too.
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SVG="$REPO_ROOT/logo/slint-logo-square-light-whitebg.svg"
CATALOG="$REPO_ROOT/tools/viewer/Assets.xcassets"
ICON_SET="$CATALOG/AppIcon.appiconset"

if ! command -v rsvg-convert >/dev/null 2>&1; then
    echo "error: rsvg-convert not found. Install it with 'brew install librsvg'." >&2
    exit 1
fi

mkdir -p "$ICON_SET"

cat > "$CATALOG/Contents.json" <<'JSON'
{
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON

cat > "$ICON_SET/Contents.json" <<'JSON'
{
  "images" : [
    {
      "filename" : "icon-1024.png",
      "idiom" : "universal",
      "platform" : "ios",
      "size" : "1024x1024"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON

# --background-color=white flattens onto an opaque background, so the result has
# no alpha channel (required for App Store icons).
rsvg-convert --background-color=white -w 1024 -h 1024 "$SVG" -o "$ICON_SET/icon-1024.png"
echo "Generated iOS app icon catalog at $CATALOG"
