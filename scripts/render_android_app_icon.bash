#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore xxxhdpi

# Render the slint-viewer Android launcher icons from the Slint logo SVG.
# The legacy ic_launcher.png is 192px on a white background; the adaptive
# foreground is 432px with the symbol inside Android's 66dp/108dp safe zone
# so launcher masks don't clip it. Sibling of render_ios_app_icon.bash.
#
# Requires resvg (cargo install resvg --locked).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SVG="$REPO_ROOT/logo/slint-logo-small-light.svg"
RES="$REPO_ROOT/tools/viewer/android-aab/app/src/main/res/mipmap-xxxhdpi"

command -v resvg >/dev/null 2>&1 \
    || { echo "install resvg: cargo install resvg --locked" >&2; exit 1; }

mkdir -p "$RES"

resvg --background=white -w 192 -h 192 "$SVG" "$RES/ic_launcher.png"

# Nest the source SVG at (84,84) with a 264x264 envelope so it fits inside the
# 66dp/108dp safe zone. The sed extraction needs the opening tag on line 1 and
# the closing tag as the last </svg>. Fail loudly if either changes.
head -1 "$SVG" | grep -qE '^<svg[^>]*>$' \
    || { echo "expected single-line <svg> opener in $SVG" >&2; exit 1; }
PADDED_SVG="<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"432\" height=\"432\" viewBox=\"0 0 432 432\"><svg x=\"84\" y=\"84\" width=\"264\" height=\"264\" viewBox=\"0 0 64 64\">$(sed -n '2,$p' "$SVG" | sed '$s|</svg>||')</svg></svg>"
resvg -w 432 -h 432 - "$RES/ic_launcher_foreground.png" <<<"$PADDED_SVG"

echo "Rendered Android app icons under $RES/"
