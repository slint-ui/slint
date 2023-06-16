#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

OUTPUT=$(slint-viewer - --save-data - << EOF
import { StandardButton, GridBox, LineEdit } from "std-widgets.slint";
_ := Dialog {
    property name <=> name-le.text;
    property address <=> address-le.text;
    StandardButton { kind: ok; }
    StandardButton { kind: cancel; }
    preferred-width: 300px;
    GridBox {
        Row {
            Text { text: "Enter your name:"; }
            name-le := LineEdit { }
        }
        Row {
            Text { text: "Address:"; }
            address-le := LineEdit { }
        }
    }
}
EOF
)
NAME=$(jq -r ".name" <<< "$OUTPUT")
ADDRESS=$(jq -r ".address" <<< "$OUTPUT")

echo "Your name is $NAME and you live in $ADDRESS!"
