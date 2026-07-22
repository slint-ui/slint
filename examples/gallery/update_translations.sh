#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# Run the script, translate, run the script again

find -name \*.slint | xargs cargo run -p slint-tr-extractor -- -d gallery -o gallery.pot

for po in lang/*/LC_MESSAGES
    do msgmerge $po/gallery.po gallery.pot -o $po/gallery.po
    msgfmt $po/gallery.po -o $po/gallery.mo
done
