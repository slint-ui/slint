<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Libraries

Libraries are collections of components and types that can be used in multiple
projects. In order to use a library, you need to add it as a dependency to your
project. The exact way to do this depends on the language you are using:

<details data-snippet-language="toml">
<summary>Rust</summary>

Specify the [dependency](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
in `Cargo.toml`. For example:

```toml
[dependencies]
example-widgets = "1.0.0"
```

</details>

<details data-snippet-language="json">
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

See also [Creating Libraries](../../advanced/creating_libraries.md).
