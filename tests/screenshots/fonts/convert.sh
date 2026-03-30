#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# This script subsets the NotoSans variable fonts from https://fonts.google.com/noto/specimen/Noto+Sans
# to a smaller character range for use in screenshot tests.

# Subset the upright variable font (keeps wdth + wght axes)
uvx --from fonttools pyftsubset NotoSans-unhinted/NotoSans-VariableFont_wdth,wght.ttf \
    --unicodes="U+0020-00FF,U+2026" \
    --output-file=NotoSans-Regular.ttf

# Subset the italic variable font (keeps wdth + wght axes)
uvx --from fonttools pyftsubset NotoSans-unhinted/NotoSans-Italic-VariableFont_wdth,wght.ttf \
    --unicodes="U+0020-00FF,U+2026" \
    --output-file=NotoSans-Italic.ttf

# Generate .license files
for f in NotoSans-Regular.ttf NotoSans-Italic.ttf; do
    cat > "$f.license" <<'LICENSE'
SPDX-FileCopyrightText: Google Inc. <https://fonts.google.com/noto/specimen/Noto+Sans/about>

SPDX-License-Identifier: OFL-1.1-RFN
LICENSE
done

# Verify that the variation axes survived subsetting
for f in NotoSans-Regular.ttf NotoSans-Italic.ttf; do
    axes=$(uvx --from fonttools fonttools ttx -o - -t fvar "$f" 2>/dev/null \
        | grep -o '<AxisTag>[^<]*</AxisTag>')
    echo "$f axes: $axes"
    if ! echo "$axes" | grep -q "wght"; then
        echo "ERROR: $f is missing the wght axis after subsetting!" >&2
        exit 1
    fi
done

echo "Done. Variable font subsets created with axes intact."
