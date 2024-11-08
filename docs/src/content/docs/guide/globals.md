<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Global Singletons

Declare a global singleton with `global Name { /* .. properties or callbacks .. */ }` to
make properties and callbacks available throughout the entire project. Access them using `Name.property`.

For example, this can be useful for a common color palette:

```slint,no-preview
global Palette  {
    in-out property<color> primary: blue;
    in-out property<color> secondary: green;
}

export component Example inherits Rectangle {
    background: Palette.primary;
    border-color: Palette.secondary;
    border-width: 2px;
}
```

Export a global to make it accessible from other files (see [Modules](modules.md)). Export a global from
the file also exporting the main application component to make it visible
to native code in the business logic.

```slint,ignore
export global Logic  {
    in-out property <int> the-value;
    pure callback magic-operation(int) -> int;
}
// ...
```

<details data-snippet-language="rust">
<summary>Usage from Rust</summary>

```rust
slint::slint!{
export global Logic {
    in-out property <int> the-value;
    pure callback magic-operation(int) -> int;
}

export component App inherits Window {
    // ...
}
}

fn main() {
    let app = App::new();
    app.global::<Logic>().on_magic_operation(|value| {
        eprintln!("magic operation input: {}", value);
        value * 2
    });
    app.global::<Logic>().set_the_value(42);
    // ...
}
```

</details>

<details data-snippet-language="cpp">
<summary>Usage from C++</summary>

```cpp
#include "app.h"

fn main() {
    auto app = App::create();
    app->global<Logic>().on_magic_operation([](int value) -> int {
        return value * 2;
    });
    app->global<Logic>().set_the_value(42);
    // ...
}
```

</details>

<details data-snippet-language="javascript">
<summary>Usage from JavaScript</summary>

```js
let slint = require("slint-ui");
let file = slint.loadFile("app.slint");
let app = new file.App();
app.Logic.magic_operation = (value) => {
    return value * 2;
};
app.Logic.the_value = 42;
// ...
```

</details>

It's possible to re-expose a callback or properties from a global using the two way binding syntax.

```slint,no-preview
global Logic  {
    in-out property <int> the-value;
    pure callback magic-operation(int) -> int;
}

component SomeComponent inherits Text {
    // use the global in any component
    text: "The magic value is:" + Logic.magic-operation(42);
}

export component MainWindow inherits Window {
    // re-expose the global properties such that the native code
    // can access or modify them
    in-out property the-value <=> Logic.the-value;
    pure callback magic-operation <=> Logic.magic-operation;

    SomeComponent {}
}
```
