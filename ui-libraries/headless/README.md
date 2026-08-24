# Headless Components for Slint

This experimental library provides unstyled semantic controls for Slint.
The controls implement interaction, focus, accessibility, and state behavior while applications
and widget libraries provide the visual design.

Map `headless` to `src/headless.slint` in the compiler configuration:

```rust
let library_paths = std::collections::HashMap::from([(
    "headless".to_string(),
    manifest_dir.join("path/to/slint/ui-libraries/headless/src/headless.slint"),
)]);
let config = slint_build::CompilerConfiguration::new().with_library_paths(library_paths);
slint_build::compile_with_config("ui/main.slint", config)?;
```

Import the controls from the mapped library:

```slint
import { Button, CheckBox, RadioButton, RadioGroup, Toggle } from "@headless";
```

## Current Scope

The first evaluation slice contains `Button`, `Toggle`, `CheckBox`, `RadioButton`, and
`RadioGroup`.
Radio buttons use explicit bindings and callbacks to communicate with their group because a pure
Slint library cannot inspect or rewrite arbitrary child components.
The public API may change while the component contract is reviewed.
