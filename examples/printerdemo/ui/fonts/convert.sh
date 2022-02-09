#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

# This script converts the NotoSans font from https://www.google.com/get/noto/#sans-lgc
# to a subset for this demo

# You need to install `pyftsubset` from the `fonttools`. That's available via `brew install fonttools`,
# or `sudo apt-get install fonttools`.

cp NotoSans-unhinted/LICENSE_OFL.txt .

for weight in Light Regular Bold; do
    pyftsubset NotoSans-unhinted/NotoSans-$weight.ttf --unicodes="U+0020-007F,U+2026" --output-file=NotoSans-$weight.ttf
done
