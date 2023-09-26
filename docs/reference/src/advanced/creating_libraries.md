<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Creating Libraries

[Libraries](../language/concepts/libraries.md) are collections of components and
types that can be used in multiple projects. In order to create a library, you
need to export the desired types from the library.

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

<details data-snippet-language="toml">
<summary>Rust</summary>

Export the entry-point of the library in `Cargo.toml`:

```toml
[package.metadata.slint]
export = "ui/lib.slint"
```

</details>

<details data-snippet-language="json">
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
