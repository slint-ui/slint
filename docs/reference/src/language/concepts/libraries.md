<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Libraries

Libraries are collections of components and types that can be used in multiple
projects. In order to use a library, you need to add it as a dependency to your
project. The exact way to do this depends on the language you are using:

<details data-snippet-language="rust">
<summary>Rust</summary>

Specify the [dependency](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
in `Cargo.toml`. For example:

```toml
[dependencies]
example-widgets = "1.0.0"
```

</details>

<details data-snippet-language="js">
<summary>JavaScript</summary>

Specify the [dependency](https://docs.npmjs.com/specifying-dependencies-and-devdependencies-in-a-package-json-file)
in `package.json`. For example:

```json
{
    "dependencies": {
        "example-widgets": "^1.0.0"
    }
}
```

</details>

Once you have added the dependency, you can import types from the library with
the `import` statement: `import { ComponentName } from "@library-name";` in a
.slint file:

```slint,ignore
import { ExampleButton } from "@example-widgets";

export component MyApp inherits Window {
    ExampleButton {
        /* ... */
    }
}
```

## Creating Libraries

In order to create a library, you need to export the desired types from the library.

```slint,ignore
// ui/button.slint
export component ExampleButton {
    /* ... */
}
```

```slint,ignore
// ui/slider.slint
export component ExampleSlider {
    /* ... */
}
```

```slint,ignore
// ui/lib.slint
import { ExampleButton } from "./button.slint";
export { ExampleButton }

import { ExampleSlider } from "./slider.slint";
export { ExampleSlider }
```

The rest depends on the language you are using:

<details data-snippet-language="rust">
<summary>Rust</summary>

Export the entry-point of the library in `Cargo.toml`:

```toml
[package.metadata.slint]
export = "ui/lib.slint"
```

</details>

<details data-snippet-language="js">
<summary>JavaScript</summary>

Export the entry-point of the library in `package.json`:

```json
{
    "slint": {
        "export": "ui/lib.slint"
    }
}
```

</details>
