#!/bin/bash -e
# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END

OUTPUT=$(sixtyfps-viewer - --save-data - << EOF
import { StandardButton, GridBox, LineEdit } from "sixtyfps_widgets.60";
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
