---
name: slint
description: Expert guidance for building, debugging, and working with Slint GUI applications. Covers the .slint markup language, project setup, debugging with the embedded MCP server, and language API bindings for Rust, C++, JavaScript, and Python.
---

# Slint Development Skill

You are an expert at building applications with [Slint](https://slint.dev), a declarative GUI toolkit for native user interfaces across desktop, embedded, mobile, and web platforms.

## The .slint Language

Slint UIs are written in `.slint` markup files. The language is declarative and reactive.

### Core Syntax

Elements are declared with `Name { }`. Properties use `property-name: value;`. Child elements are nested inside parent braces.

```slint
export component MyApp inherits Window {
    preferred-width: 400px;
    preferred-height: 300px;

    VerticalLayout {
        Text {
            text: "Hello World";
            font-size: 24px;
            color: #0044ff;
        }
        Rectangle {
            background: lightgray;
            border-radius: 8px;
            height: 40px;
        }
    }
}
```

### Components

Define reusable components with `component Name inherits Base { }`. The root file must `export component` at least one component.

```slint
component MyButton inherits Rectangle {
    in property <string> label: "Click me";
    callback clicked;

    background: ta.pressed ? darkblue : blue;
    border-radius: 4px;
    height: 36px;

    ta := TouchArea {
        clicked => { root.clicked(); }
    }

    Text {
        text: root.label;
        color: white;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}

export component App inherits Window {
    MyButton {
        label: "Submit";
        clicked => { debug("clicked"); }
    }
}
```

### Properties

Properties are declared with access qualifiers:
- `in property <type> name;` - settable from outside, read internally
- `out property <type> name;` - read from outside, set internally
- `in-out property <type> name;` - read/write from both sides
- `private property <type> name;` - internal only (default if no qualifier)

Primitive types: `int`, `float`, `string`, `bool`, `length`, `physical-length`, `duration`, `angle`, `color`, `brush`, `image`, `percent`.

Length literals: `100px` (logical pixels), `1phx` (physical pixels).
Duration literals: `250ms`, `1s`.
Angle literals: `90deg`, `1rad`, `0.5turn`.
Color literals: `#rrggbb`, `#rrggbbaa`, or named colors like `red`, `blue`, `transparent`.

### Binding Expressions and Reactivity

Every property assignment is a live binding. When dependencies change, expressions automatically re-evaluate.

```slint
export component App inherits Window {
    in-out property <int> counter: 0;

    Text {
        // This updates automatically when counter changes
        text: "Count: " + counter;
    }

    TouchArea {
        clicked => { counter += 1; }
    }
}
```

Two-way bindings: `property <=> other-element.property;`

### Callbacks

Declared with `callback name(param-type, ...) -> return-type;` and invoked with `name(args)`.

```slint
export component App inherits Window {
    callback submit(string);
    callback validated(string) -> bool;

    // Handler with code block
    submit(value) => {
        debug("submitted: ", value);
    }
}
```

### Structs and Enums

```slint no-test
export struct TodoItem {
    title: string,
    done: bool,
}

export enum Status {
    active,
    completed,
    archived,
}
```

### Models and Repeaters

Use `for item[index] in model :` to repeat elements. Models are typically `[ModelType]` arrays.

```slint no-test
export component TodoList inherits Window {
    in property <[TodoItem]> items;

    VerticalLayout {
        for item[i] in items: HorizontalLayout {
            CheckBox { checked: item.done; }
            Text { text: item.title; }
        }
    }
}
```

### Conditional Elements

```slint no-test
if condition : Text { text: "Shown when true"; }
```

### States and Transitions

```slint
component ToggleButton inherits Rectangle {
    in-out property <bool> active;

    states [
        pressed when active: {
            background: blue;
            border-width: 2px;

            in {
                animate background { duration: 200ms; easing: ease-in-out; }
            }
        }
    ]
}
```

### Animations

Inline: `animate property { duration: 250ms; easing: ease-in-out; }`

### Imports

```slint no-test
import { Button, LineEdit, VerticalBox } from "std-widgets.slint";
import { MyComponent } from "my-component.slint";
```

### Standard Widget Library

Import from `"std-widgets.slint"`. Key widgets:

**Basic**: `Button`, `CheckBox`, `ComboBox`, `LineEdit`, `Slider`, `SpinBox`, `Switch`, `ProgressIndicator`, `Spinner`
**Layouts**: `HorizontalBox`, `VerticalBox`, `GridBox`, `GroupBox`
**Views**: `ListView`, `StandardListView`, `StandardTableView`, `ScrollView`, `TabWidget`, `TextEdit`
**Window**: `Dialog`, `PopupWindow`, `ContextMenuArea`, `MenuBar`, `Menu`, `MenuItem`
**Misc**: `DatePicker`, `TimePicker`, `AboutSlint`
**Globals**: `Palette` (theme colors), `StyleMetrics`

Available styles: `fluent`, `material`, `cupertino`, `cosmic`, `native`, `qt`.
Set via: `SLINT_STYLE=fluent` environment variable, or in build script configuration.

### Built-in Elements

These are always available without imports:

**Visual**: `Rectangle`, `Text`, `Image`, `Path`, `StyledText`
**Layout**: `HorizontalLayout`, `VerticalLayout`, `GridLayout`, `FlexboxLayout`
**Input**: `TouchArea`, `FocusScope`, `Flickable`, `SwipeGestureHandler`, `ScaleRotateGestureHandler`
**Window**: `Window`, `PopupWindow`, `Dialog`, `ContextMenuArea`

### Global Singletons

```slint
export global AppState {
    in-out property <string> current-user;
    callback logout();
}
```

Accessed as `AppState.current-user` from any component in the same file, or from application code via the language bindings.

### Element IDs

Assign with `name := Element { }`. Reference with `name.property`. Qualified IDs for testing: `ComponentName::element-id`.

### Key Properties on Common Elements

**Rectangle**: `background`, `border-color`, `border-width`, `border-radius`, `clip`, `opacity`
**Text**: `text`, `font-size`, `font-weight`, `font-family`, `color`, `horizontal-alignment`, `vertical-alignment`, `wrap`, `overflow`, `letter-spacing`
**Image**: `source` (`@image-url("path")`), `image-fit` (`fill`, `contain`, `cover`), `image-rendering`, `colorize`, `rotation-angle`
**Window**: `title`, `icon`, `default-font-size`, `background`, `preferred-width`, `preferred-height`
**Layout shared**: `spacing`, `padding`, `alignment`
**All elements**: `x`, `y`, `width`, `height`, `visible`, `opacity`, `accessible-role`, `accessible-label`

### @-macros

- `@image-url("path/to/image.png")` - load an image at compile time
- `@tr("context" => "text {}", value)` - translatable string
- `@linear-gradient(angle, color stops...)` - gradient brush
- `@radial-gradient(circle, color stops...)` - radial gradient
- `@keys(Control + C)` - keyboard shortcut for MenuItems

## Project Setup

### Rust

```toml
# Cargo.toml
[dependencies]
slint = "1.x"

[build-dependencies]
slint-build = "1.x"
```

```rust
// build.rs
fn main() {
    slint_build::compile("ui/main.slint").unwrap();
}
```

```rust
// main.rs
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = MainWindow::new()?;
    // Set up callbacks, models, etc.
    app.run()
}
```

### C++

Use CMake with `FetchContent` or find_package:
```cmake
find_package(Slint)
slint_target_sources(my_app ui/main.slint)
```

### Node.js

```js
const slint = require("slint-ui");
const app = new slint.MainWindow();
app.run();
```

### Python

```python
import slint
# Load .slint files dynamically
```

## Debugging Slint Applications

### Common Issues

1. **Binding loops**: A property depends on itself through a chain of bindings. The compiler warns about these. Break the cycle by introducing an intermediate property or restructuring.

2. **Elements not visible**: Check `width`, `height` (may be 0 if not in a layout), `visible`, `opacity`, and parent clipping.

3. **Layout sizing**: Elements outside layouts need explicit `width`/`height`. Inside layouts, they get sized automatically. Use `preferred-width`, `min-width`, `max-width` to constrain.

4. **Type mismatches**: `length` and `int`/`float` are different types. Use `1px * my_int` to convert, or `my_length / 1px` to get a number.

5. **Performance**: Use `ListView` (not `for` in `ScrollView`) for long lists - it virtualizes. Use `image-rendering: pixelated` only when needed. Avoid deeply nested opacity/clip layers.

### Debug Helpers

- `debug("message", expression)` - prints to stderr at runtime
- `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console` - performance overlay
- Run with `SLINT_BACKEND=winit-skia` or other backend variants for testing

## MCP Server for AI-Assisted Debugging

Slint includes an embedded MCP (Model Context Protocol) server that lets you inspect and interact with a running Slint application in real time. This is especially powerful for debugging UI issues.

### Enabling the MCP Server

**Important**: The MCP server is exposed through the **internal** crate `i-slint-backend-selector`, not the public `slint` crate. This internal crate does not follow semver and **must be pinned to the exact Slint version** using `=`. If the project uses `slint = "1.16.0"`, then the backend selector must use `version = "=1.16.0"`. A version mismatch will cause build failures.

**Step 1**: Add the `i-slint-backend-selector` crate with the `mcp` feature to the project's `Cargo.toml`, pinned to the exact same version as the `slint` crate:

```toml
[dependencies]
slint = "1.16.0"
i-slint-backend-selector = { version = "=1.16.0", features = ["mcp"] }
```

If the project is part of a workspace that depends on Slint from a path (e.g. working within the Slint repo itself), use the workspace reference instead and enable the feature via `--features i-slint-backend-selector/mcp` on the cargo command line.

**Step 2**: Build with `SLINT_EMIT_DEBUG_INFO=1` so that element IDs and source locations are preserved in the compiled output. Without this, elements will lack the debug metadata needed for meaningful introspection. Set `SLINT_MCP_PORT` to an available port when running:

```sh
SLINT_EMIT_DEBUG_INFO=1 SLINT_MCP_PORT=9315 cargo run -p my-app
```

**Step 3**: Connect directly to the running application's MCP server at `http://localhost:9315/mcp` using Streamable HTTP transport. Use the MCP tools to inspect and interact with the UI.

### Version Requirements

The MCP server uses internal Slint APIs (`i-slint-backend-selector`), so the available features depend on the Slint version:

| Slint Version | MCP Support |
|---------------|-------------|
| < 1.16.0 | Not available |
| >= 1.16.0 | Full MCP server with `i-slint-backend-selector` `mcp` feature |

The `i-slint-backend-selector` crate does not follow semver — it must be pinned to the exact Slint version with `=` (e.g. `version = "=1.16.0"`). MCP features and tools may change between Slint releases without notice.

### When to Suggest MCP

Suggest enabling the MCP server when the user is:
- Debugging layout or visual issues ("my element isn't showing up", "the layout is wrong")
- Trying to understand the runtime element hierarchy
- Testing interactions programmatically
- Verifying accessibility properties
- Diagnosing event handling problems

## Documentation Reference

Full documentation is at https://slint.dev/docs. Key sections:
- Language guide: concepts, syntax, coding patterns
- Reference: all elements, properties, types, std-widgets
- Language integrations: Rust, C++, Node.js, Python API docs
- Tutorials: step-by-step guides for each language
