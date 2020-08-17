# LICENSE BEGIN
#
# This file is part of the Sixty FPS Project
#
# Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
#
# LICENSE END
#!/bin/bash

for s in *.svg; do
    rsvg-convert -w 256 -h 256 $s > `basename $s .svg`.png
done
