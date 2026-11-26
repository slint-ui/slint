#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cSpell: ignore wdth subsetting cmap NOTOSANS Bwdth Cwght COLR CBDT notofonts

# This script subsets the NotoSans variable fonts to a smaller character range for use in
# screenshot tests. It also subsets NotoSansSymbols2 to a few symbols NotoSans doesn't cover.
# The subsets keep the outline of `.notdef`, which renderers draw for missing characters.
# Two color emoji fonts are cut down to U+1F600 for the color glyph tests: Noto Color
# Emoji (bitmaps in a CBDT table) and Twemoji Mozilla (layers in a COLR version 0 table).

cd "$(dirname "$0")"

# Fetch the upstream fonts if they're not already present, at pinned revisions.
# GitHub serves the TTF files directly, unlike fonts.google.com/download,
# which returns a JS-rendered HTML page.
mkdir -p NotoSans-unhinted
# NotoSans comes from the Google Fonts build: unlike the notofonts one, it has a `gasp` table,
# which FemtoVG's rendering depends on.
GOOGLE_FONTS_REVISION="5bd87dfc525c8a0d1d8529d3684716e00c3fc59a"
sans_base="https://github.com/google/fonts/raw/$GOOGLE_FONTS_REVISION/ofl/notosans"
# The other Noto fonts come from https://github.com/notofonts/notofonts.github.io,
# the canonical build artifacts of Google's Noto fonts.
NOTO_REVISION="c95573421242b0925256d147e295c5bda3c400ed"
symbols_base="https://github.com/notofonts/notofonts.github.io/raw/$NOTO_REVISION/fonts/NotoSansSymbols2/full/ttf"
fetch() {
    local dest="$1" url="$2"
    if [ ! -f "$dest" ]; then
        echo "Downloading $(basename "$dest")..."
        curl -fsSL -o "$dest" "$url"
    fi
}
fetch "NotoSans-unhinted/NotoSans-VariableFont_wdth,wght.ttf" \
    "$sans_base/NotoSans%5Bwdth%2Cwght%5D.ttf"
fetch "NotoSans-unhinted/NotoSans-Italic-VariableFont_wdth,wght.ttf" \
    "$sans_base/NotoSans-Italic%5Bwdth%2Cwght%5D.ttf"
fetch "NotoSans-unhinted/NotoSansSymbols2-Regular.ttf" \
    "$symbols_base/NotoSansSymbols2-Regular.ttf"
NOTO_EMOJI_REVISION="99701e07d3d1fe1bdbcdb14964547f0fd592966f"
fetch "NotoSans-unhinted/NotoColorEmoji.ttf" \
    "https://github.com/googlefonts/noto-emoji/raw/$NOTO_EMOJI_REVISION/2D/fonts/NotoColorEmoji.ttf"
fetch "NotoSans-unhinted/Twemoji.Mozilla.ttf" \
    "https://github.com/mozilla/twemoji-colr/releases/download/v0.7.0/Twemoji.Mozilla.ttf"

# U+0020-00FF: Basic Latin + Latin-1 Supplement
# U+2022: bullet (markdown list items)
# U+2026: horizontal ellipsis (used for text truncation)
NOTOSANS_UNICODES="U+0020-00FF,U+2022,U+2026"

subset_font() {
    local input="$1" output="$2" unicodes="$3"
    uvx --from fonttools pyftsubset "$input" \
        --unicodes="$unicodes" \
        --notdef-outline \
        --output-file="$output"
}

# NotoSans subsets keep the wdth + wght axes.
subset_font "NotoSans-unhinted/NotoSans-VariableFont_wdth,wght.ttf" \
    NotoSans-Regular.ttf "$NOTOSANS_UNICODES"
subset_font "NotoSans-unhinted/NotoSans-Italic-VariableFont_wdth,wght.ttf" \
    NotoSans-Italic.ttf "$NOTOSANS_UNICODES"

# NotoSans doesn't include these, so ship them as a separate fallback:
# U+25CF BLACK CIRCLE is the default Slint password-input replacement character;
# U+25AA BLACK SMALL SQUARE and U+25E6 WHITE BULLET mark nested markdown list items.
subset_font NotoSans-unhinted/NotoSansSymbols2-Regular.ttf \
    NotoSansSymbols2-Regular.ttf "U+25AA,U+25CF,U+25E6"

# U+1F600 GRINNING FACE, once from each color emoji font.
subset_font NotoSans-unhinted/NotoColorEmoji.ttf NotoColorEmoji-Subset.ttf "U+1F600"
subset_font NotoSans-unhinted/Twemoji.Mozilla.ttf TwemojiMozilla-Subset.ttf "U+1F600"
# Draw the face's dark layer (eyes and mouth) in the text's color (palette index 0xFFFF), so the
# test also covers layers that take the text's color.
uvx --from fonttools python -c '
import sys
from fontTools.ttLib import TTFont
font = TTFont(sys.argv[1])
cpal = font["CPAL"].palettes[0]
for layers in font["COLR"].ColorLayers.values():
    for layer in layers:
        if cpal[layer.colorID].hex() == "#664500FF":
            layer.colorID = 0xFFFF
font.save(sys.argv[1])
' TwemojiMozilla-Subset.ttf

cat > NotoColorEmoji-Subset.ttf.license <<'LICENSE'
SPDX-FileCopyrightText: Google Inc. <https://github.com/googlefonts/noto-emoji>

SPDX-License-Identifier: OFL-1.1
LICENSE
cat > TwemojiMozilla-Subset.ttf.license <<'LICENSE'
SPDX-FileCopyrightText: Twitter, Inc and other contributors (graphics), Mozilla Foundation (font) <https://github.com/mozilla/twemoji-colr>

SPDX-License-Identifier: CC-BY-4.0
LICENSE

# Generate .license files
for f in NotoSans-Regular.ttf NotoSans-Italic.ttf NotoSansSymbols2-Regular.ttf; do
    cat > "$f.license" <<'LICENSE'
SPDX-FileCopyrightText: Google Inc. <https://fonts.google.com/noto/specimen/Noto+Sans/about>

SPDX-License-Identifier: OFL-1.1-RFN
LICENSE
done

# Verify that the variation axes survived subsetting (only for the NotoSans pair)
for f in NotoSans-Regular.ttf NotoSans-Italic.ttf; do
    axes=$(uvx --from fonttools fonttools ttx -o - -t fvar "$f" 2>/dev/null \
        | grep -o '<AxisTag>[^<]*</AxisTag>')
    echo "$f axes: $axes"
    if ! echo "$axes" | grep -q "wght"; then
        echo "ERROR: $f is missing the wght axis after subsetting!" >&2
        exit 1
    fi
done

# Verify U+25CF survived in the symbols subset
if ! uvx --from fonttools fonttools ttx -o - -t cmap NotoSansSymbols2-Regular.ttf 2>/dev/null \
    | grep -q '0x25cf'; then
    echo "ERROR: NotoSansSymbols2-Regular.ttf is missing U+25CF after subsetting!" >&2
    exit 1
fi

echo "Done. Font subsets created."
