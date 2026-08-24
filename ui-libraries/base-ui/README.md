# Headless Components for Slint

This experimental library provides unstyled semantic controls for Slint.
The controls own interaction, focus, accessibility, and state behavior while applications and
widget libraries provide the visual design.

The public API is intentionally small and may change while the component contracts are reviewed.
The initial slice does not replace or modify Slint's standard widgets.

Map a library name to `src/base-ui.slint` in your compiler configuration:

```rust
let library_paths = std::collections::HashMap::from([(
    "headless".to_string(),
    manifest_dir.join("path/to/slint/ui-libraries/base-ui/src/base-ui.slint"),
)]);
let config = slint_build::CompilerConfiguration::new().with_library_paths(library_paths);
slint_build::compile_with_config("ui/main.slint", config)?;
```

Import components through the mapped entry point:

```slint
import { Button } from "@headless";
```

## Current Scope

The first slice contains `Button`, `Toggle`, `CheckBox`, `RadioButton`, and `RadioGroup`.
Radio buttons and groups use explicit wiring instead of implicit component context.

This slice does not include radio-group arrow navigation or roving tab stops, an indeterminate
checkbox state, interaction-modality tracking, or standard-widget style integration.
