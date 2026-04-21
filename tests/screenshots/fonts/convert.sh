#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# This script subsets the NotoSans variable fonts from https://fonts.google.com/noto/specimen/Noto+Sans
# to a smaller character range for use in screenshot tests. It also subsets
# NotoSansSymbols2 down to a single glyph (U+25CF BLACK CIRCLE) — the default
# Slint password-input replacement character, which is not covered by NotoSans.

cd "$(dirname "$0")"

# Fetch the upstream fonts if they're not already present.
# Source: https://github.com/notofonts/notofonts.github.io (the canonical build artifacts
# of Google's Noto fonts), which redirects to raw.githubusercontent.com and serves the
# TTF directly — unlike fonts.google.com/download, which returns a JS-rendered HTML page.
mkdir -p NotoSans-unhinted
# Pinned for reproducibility — bump when a newer upstream revision is needed.
NOTO_REVISION="c95573421242b0925256d147e295c5bda3c400ed"
sans_base="https://github.com/notofonts/notofonts.github.io/raw/$NOTO_REVISION/fonts/NotoSans/full/variable-ttf"
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

# U+0020-00FF: Basic Latin + Latin-1 Supplement
# U+2026: horizontal ellipsis (used for text truncation)
NOTOSANS_UNICODES="U+0020-00FF,U+2026"

# Subset the upright variable font (keeps wdth + wght axes)
uvx --from fonttools pyftsubset NotoSans-unhinted/NotoSans-VariableFont_wdth,wght.ttf \
    --unicodes="$NOTOSANS_UNICODES" \
    --output-file=NotoSans-Regular.ttf

# Subset the italic variable font (keeps wdth + wght axes)
uvx --from fonttools pyftsubset NotoSans-unhinted/NotoSans-Italic-VariableFont_wdth,wght.ttf \
    --unicodes="$NOTOSANS_UNICODES" \
    --output-file=NotoSans-Italic.ttf

# Subset NotoSansSymbols2 down to just U+25CF BLACK CIRCLE — the default
# password-input replacement character used by Slint. NotoSans doesn't
# include this glyph, so we ship it as a separate fallback font.
uvx --from fonttools pyftsubset NotoSans-unhinted/NotoSansSymbols2-Regular.ttf \
    --unicodes="U+25CF" \
    --output-file=NotoSansSymbols2-Regular.ttf

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
