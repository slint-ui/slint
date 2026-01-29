#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# This script converts the NotoSans font from https://www.google.com/get/noto/#sans-lgc
# to a subset for this demo

# You need to install `pyftsubset` from the `fonttools`. That's available via `brew install fonttools`,
# or `sudo apt-get install fonttools`.


for weight in Regular Medium SemiBold Bold; do
    pyftsubset unhinted/Inter-24pt-$weight.ttf --unicodes="U+0020-00FF,U+2026" --output-file=Inter-24pt-$weight.ttf
done
