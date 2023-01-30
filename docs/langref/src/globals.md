# Global Singletons

Declare a global singleton with `global Name := { /* .. properties or callbacks .. */ }` when you want to
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

A global can be declared in another module file, and imported from many files.

Access properties and callbacks from globals in native code by marking them as exported
in the file that exports your main application component. In the above example it is
sufficient to directly export the `Logic` global:

```slint,ignore
export global Logic  {
    in-out property <int> the-value;
    pure callback magic-operation(int) -> int;
}
// ...
```

It's also possible to export globals from other files:

```slint,ignore
import { Logic as MathLogic } from "math.slint";
export { MathLogic } // known as "MathLogic" when using native APIs to access globals
```

<details data-snippet-language="rust">
<summary>Usage from Rust</summary>

```rust
slint::slint!{
export global Logic := {
    property <int> the-value;
    callback magic-operation(int) -> int;
}

export App := Window {
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

It is possible to re-expose a callback or properties from a global using the two way binding syntax.

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
