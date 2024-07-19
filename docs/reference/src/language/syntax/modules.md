<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Modules

Components declared in a `.slint` file can be used as elements in other
`.slint` files, by means of exporting and importing them.

By default, every type declared in a `.slint` file is private. The `export`
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

In the above example, `Button` is accessible from other `.slint` files, but
`ButtonHelper` isn't.

It's also possible to change the name just for the purpose of exporting, without
affecting its internal use:

```slint,no-preview
component Button inherits Rectangle {
    // ...
}

export { Button as ColorButton }
```

In the above example, `Button` isn't accessible from the outside, but
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

## Module Syntax

The following syntax is supported for importing types:

```slint,ignore
import { export1 } from "module.slint";
import { export1, export2 } from "module.slint";
import { export1 as alias1 } from "module.slint";
import { export1, export2 as alias2, /* ... */ } from "module.slint";
```

The following syntax is supported for exporting types:

```slint,ignore
// Export declarations
export component MyButton inherits Rectangle { /* ... */ }

// Export lists
component MySwitch inherits Rectangle { /* ... */ }
export { MySwitch }
export { MySwitch as Alias1, MyButton as Alias2 }

// Re-export types from other module
export { MyCheckBox, MyButton as OtherButton } from "other_module.slint";

// Re-export all types from other module (only possible once per file)
export * from "other_module.slint";
```

## Component Libraries

Splitting your code base into separate module files promotes re-use and
improves encapsulation by allow you to hide helper components. This works
well within a project's own directory structure. To share libraries of
components between projects without hardcoding their relative paths, use
the component library syntax:

```slint,ignore
import { MySwitch } from "@mylibrary/switch.slint";
import { MyButton } from "@otherlibrary";
```

In the above example, the `MySwitch` component will be imported from a component
library called `mylibrary`, in which Slint looks for the `switch.slint` file. Therefore `mylibrary` must be
declared to refer to a directory, so that the subsequent search for `switch.slint`
succeeds. `MyButton` will be imported from `otherlibrary`. Therefore `otherlibrary`
must be declared to refer to a `.slint` file that exports `MyButton`.

The path to each library, as file or directory, must be defined separately at compilation time.
Use one of the following methods to help the Slint compiler resolve libraries to the correct
path on disk:

* When using Rust and `build.rs`, call [`with_library_paths`](slint-build-rust:struct.CompilerConfiguration#method.with_library_paths)
  to provide a mapping from library name to path.
* When using C++, use `LIBRARY_PATHS` with [`slint_target_sources`](slint-cpp:cmake_reference#slint-target-sources).
* When invoking the `slint-viewer` from the command line, pass `-Lmylibrary=/path/to/my/library` for each component
  library.
* When using the VS Code extension, configure the Slint extension's library path
  using the `Slint: Library Paths` setting. Example below:
  ```json
  "slint.libraryPaths": {
      "mylibrary": "/path/to/my/library",
      "otherlibrary": "/path/to/otherlib/index.slint",
  },
  ```
* With other editors, you can configure them to pass the `-L` argument to the `slint-lsp` just like for the slint-viewer.
