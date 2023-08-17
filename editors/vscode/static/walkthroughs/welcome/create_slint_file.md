<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

The HelloWorld for Slint looks like this:

```slint
import { Button, VerticalBox } from "std-widgets.slint";

export component Demo {
    VerticalBox {
        alignment: start;
        Text {
            text: "Hello World!";
            font-size: 2rem;
            horizontal-alignment: center;
        }
        HorizontalLayout {
            alignment: center;
            Button { text: "OK"; }
        }
    }
}
```

To get started, copy and paste this into your new `.slint` file.

_Make sure to save the file with .slint extension for VSCode to accept that it is indeed a Slint file._
