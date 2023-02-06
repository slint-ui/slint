# Modules

Components declared in a `.slint` file can be shared with components in other
`.slint` files, by means of exporting and importing them.

By default, everything declared in a `.slint` file is private. The `export`
keyword changes this.

```slint,no-preview
component ButtonHelper inherits Rectangle {
    // ...
}

component Button inherits Rectangle {
    // ...
    ButtonHelper {
        // ...
    }
}

export { Button }
```

In the above example, `Button` is usable from other `.slint` files, but
`ButtonHelper` isn't.

It's also possible to change the name just for the purpose of exporting, without
affecting its internal use:

```slint,no-preview
component Button inherits Rectangle {
    // ...
}

export { Button as ColorButton }
```

In the above example, `Button` is not accessible from the outside, but
is available under the name `ColorButton` instead.

For convenience, a third way of exporting a component is to declare it exported
right away:

```slint,no-preview
export component Button inherits Rectangle {
    // ...
}
```

Similarly, components exported from other files may be imported:

```slint,ignore
import { Button } from "./button.slint";

export component App inherits Rectangle {
    // ...
    Button {
        // ...
    }
}
```

In the event that two files export a type under the same name, then you have the option
of assigning a different name at import time:

```slint,ignore
import { Button } from "./button.slint";
import { Button as CoolButton } from "../other_theme/button.slint";

export component App inherits Rectangle {
    // ...
    CoolButton {} // from other_theme/button.slint
    Button {} // from button.slint
}
```

Elements, globals and structs can be exported and imported.

It's also possible to export globals (see [Global Singletons](globals.md)) imported from
other files:

```slint,ignore
import { Logic as MathLogic } from "math.slint";
export { MathLogic } // known as "MathLogic" when using native APIs to access globals
```
