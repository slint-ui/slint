<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

The HelloWorld for Slint looks like this:

```slint
import { Button, VerticalBox } from "std-widgets.slint";

export component Demo {
    VerticalBox {
        alignment: start;
        Text {
            text: "Hello World!";
            font-size: 24px;
            horizontal-alignment: center;
        }
        HorizontalLayout {
            alignment: center;
            Button { text: "OK!"; }
        }
    }
}
```

You can copy and paste that into your new `.slint` file to get you started.

_Make sure to save the file with .slint extension for VSCode to accept that it is indeed a Slint file_
