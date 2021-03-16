/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
# SixtyFPS interpreter library

With this crate, you can load a .60 at runtime and show its UI.

You only need to use this crate if you do not want to use pre-compiled .60
code, which is the normal way to use SixtyFPS, using the `sixtyfps` crate

The entry point for this crate is the [`ComponentDefinition`] type, which you can
instantiate with the [`ComponentDefinition::from_source`] or [`ComponentDefinition::from_path`]
functions.

### Note about `async` functions

Compiling a component is `async` but in practice, this is only assynchronious if [`CompilerConfiguration::with_file_loader`]
is set and its future is actually asynchronious.  If that is not used, then it is fine to use a very simple
executor, such as the one provided by the `spin_on` crate

## Examples

This example load a `.60` dynamically from a path and show error if any

```rust
use sixtyfps_interpreter::{ComponentDefinition, CompilerConfiguration};

let (definition, diagnostics) =
    spin_on::spin_on(ComponentDefinition::from_path("hello.60", CompilerConfiguration::new()));
# #[cfg(feature="print_diagnostics")]
sixtyfps_interpreter::print_diagnostics(&diagnostics);
if let Some(definition) = definition {
    let instance = definition.create();
    instance.run();
}
```

This example load a `.60` from a string and set some properties

```rust
use sixtyfps_interpreter::{ComponentDefinition, CompilerConfiguration, Value, SharedString};

let code = r#"
    MyWin := Window {
        property <string> my_name;
        Text {
            text: "Hello, " + my_name;
        }
    }
"#;

let (definition, diagnostics) =
    spin_on::spin_on(ComponentDefinition::from_source(code.into(), Default::default(), CompilerConfiguration::new()));
assert!(diagnostics.is_empty());
let instance = definition.unwrap().create();
instance.set_property("my_name", Value::from(SharedString::from("World"))).unwrap();
# return; // we don't want to call run in the tests
instance.run();
```

## Features

**display-diagnostics**: enable the `[print_diagnostics]` function to show diagnostic in the console output
*/
#![warn(missing_docs)]
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

mod api;
mod dynamic_component;
mod dynamic_type;
mod eval;
mod global_component;
mod value_model;

#[doc(inline)]
pub use api::*;

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided path must refer to a valid TrueType
/// font.
pub fn register_font_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_path(path.as_ref())
}

/// This function can be used to register a custom TrueType font with SixtyFPS,
/// for use with the `font-family` property. The provided slice must be a valid TrueType
/// font.
pub fn register_font_from_memory(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_memory(data)
}

/// (Re-export from corelib.)
#[doc(inline)]
pub use sixtyfps_corelib::{Brush, Color, SharedString, SharedVector};
