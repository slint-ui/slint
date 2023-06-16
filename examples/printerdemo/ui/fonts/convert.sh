#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# This script converts the NotoSans font from https://www.google.com/get/noto/#sans-lgc
# to a subset for this demo

# You need to install `pyftsubset` from the `fonttools`. That's available via `brew install fonttools`,
# or `sudo apt-get install fonttools`.

cp NotoSans-unhinted/LICENSE_OFL.txt .

for weight in Light Regular Bold; do
    pyftsubset NotoSans-unhinted/NotoSans-$weight.ttf --unicodes="U+0020-007F,U+2026" --output-file=NotoSans-$weight.ttf
done
pyftsubset NotoSans-unhinted/NotoSans-Italic.ttf --unicodes="U+0020-007F,U+2026" --output-file=NotoSans-Italic.ttf
