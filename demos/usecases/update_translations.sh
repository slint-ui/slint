#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

# Run the script, translate, run the script again

find . -type f -name *.slint | xargs cargo run -p slint-tr-extractor -- -d usecases -o usecases.pot

for po in lang/*/LC_MESSAGES
    do msgmerge $po/usecases.po usecases.pot -o $po/usecases.po
    msgfmt $po/usecases.po -o $po/usecases.mo
done

