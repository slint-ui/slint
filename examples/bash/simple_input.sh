#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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
