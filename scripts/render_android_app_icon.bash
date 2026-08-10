#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore xxxhdpi

# Render the slint-viewer Android launcher icons from the Slint logo SVG.
# The legacy ic_launcher.png is 192px on a white background; the adaptive
# foreground is a full-bleed 432px render that ic_launcher.xml insets into
# the 66dp/108dp safe zone. Sibling of render_ios_app_icon.bash.
#
# Requires resvg (cargo install resvg --locked).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SVG="$REPO_ROOT/logo/slint-logo-small-light.svg"
RES="$REPO_ROOT/tools/viewer/android/app/src/main/res/mipmap-xxxhdpi"

command -v resvg >/dev/null 2>&1 \
    || { echo "install resvg: cargo install resvg --locked" >&2; exit 1; }

mkdir -p "$RES"

resvg --background=white -w 192 -h 192 "$SVG" "$RES/ic_launcher.png"
resvg -w 432 -h 432 "$SVG" "$RES/ic_launcher_foreground.png"

echo "Rendered Android app icons under $RES/"
