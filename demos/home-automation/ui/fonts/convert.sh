#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# This script converts the Roboto Mono font from https://fonts.google.com/specimen/Roboto+Mono
# to a subset for this demo

# You need to install `pyftsubset` from the `fonttools`. That's available via `brew install fonttools`,
# or `sudo apt-get install fonttools`.

if [ -d Roboto_Mono ]; then
    pyftsubset Roboto_Mono/static/RobotoMono-Regular.ttf --text=" 0123456789:" --no-hinting --output-file=RobotoMono-Regular.ttf
fi
