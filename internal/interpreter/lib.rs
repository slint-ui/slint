// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
# Slint interpreter library

With this crate, you can load a .slint file at runtime and show its UI.

You only need to use this crate if you do not want to use pre-compiled .slint
code, which is the normal way to use Slint, using the `slint` crate

The entry point for this crate is the [`ComponentCompiler`] type, which you can
use to create [`ComponentDefinition`] with the [`ComponentCompiler::build_from_source`] or [`ComponentCompiler::build_from_path`]
functions.

### Note about `async` functions

Compiling a component is `async` but in practice, this is only asynchronous if [`ComponentCompiler::set_file_loader`]
is set and its future is actually asynchronous.  If that is not used, then it is fine to use a very simple
executor, such as the one provided by the `spin_on` crate

## Examples

This example loads a `.slint` dynamically from a path and show errors if any:

```rust
use slint_interpreter::{ComponentDefinition, ComponentCompiler, ComponentHandle};

let mut compiler = ComponentCompiler::default();
let definition =
    spin_on::spin_on(compiler.build_from_path("hello.slint"));
# #[cfg(feature="print_diagnostics")]
slint_interpreter::print_diagnostics(&compiler.diagnostics());
if let Some(definition) = definition {
    let instance = definition.create().unwrap();
    instance.run().unwrap();
}
```

This example load a `.slint` from a string and set some properties:

```rust
use slint_interpreter::{ComponentDefinition, ComponentCompiler, Value, SharedString, ComponentHandle};

let code = r#"
    export component MyWin inherits Window {
        in property <string> my_name;
        Text {
            text: "Hello, " + my_name;
        }
    }
"#;

let mut compiler = ComponentCompiler::default();
let definition =
    spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
assert!(compiler.diagnostics().is_empty());
let instance = definition.unwrap().create().unwrap();
instance.set_property("my_name", Value::from(SharedString::from("World"))).unwrap();
# return; // we don't want to call run in the tests
instance.run().unwrap();
```
*/
//! ## Feature flags
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
#![warn(missing_docs)]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

#[cfg(not(feature = "compat-1-0"))]
compile_error!(
    "The feature `compat-1-0` must be enabled to ensure \
    forward compatibility with future version of this crate"
);

mod api;
mod dynamic_component;
mod dynamic_type;
mod eval;
mod eval_layout;
mod global_component;
#[cfg(feature = "highlight")]
mod highlight;
mod value_model;

#[doc(inline)]
pub use api::*;

/// (Re-export from corelib.)
#[doc(inline)]
pub use i_slint_core::{Brush, Color, SharedString, SharedVector};

/// One need to use at least one function in each module in order to get them
/// exported in the final binary.
/// This only use functions from modules which are not otherwise used.
#[doc(hidden)]
#[cold]
#[cfg(feature = "ffi")]
pub fn use_modules() -> usize {
    crate::api::ffi::slint_interpreter_value_new as usize
}

#[cfg(test)]
mod tests;
