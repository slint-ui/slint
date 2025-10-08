// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg_attr(docsrs, feature(doc_cfg))]

/*!
# Slint interpreter library

With this crate, you can load a .slint file at runtime and show its UI.

You only need to use this crate if you do not want to use pre-compiled .slint
code, which is the normal way to use Slint, using the `slint` crate

The entry point for this crate is the [`Compiler`] type, which you can
use to create [`CompilationResult`] with the [`Compiler::build_from_source`] or [`Compiler::build_from_path`]
functions. [`CompilationResult`] provides access to all components declared for export. Obtain a [`ComponentDefinition`]
for each and use [`ComponentDefinition::create()`] to instantiate a component. The returned [`ComponentInstance`]
in turn provides access to properties, callbacks, functions, global singletons, as well as implementing [`ComponentHandle`].

### Note about `async` functions

Compiling a component is `async` but in practice, this is only asynchronous if [`Compiler::set_file_loader`]
is set and its future is actually asynchronous.  If that is not used, then it is fine to use a very simple
executor, such as the one provided by the `spin_on` crate

## Examples

This example loads a `.slint` dynamically from a path and show errors if any:

```rust
use slint_interpreter::{ComponentDefinition, Compiler, ComponentHandle};

let compiler = Compiler::default();
let result = spin_on::spin_on(compiler.build_from_path("hello.slint"));
let diagnostics : Vec<_> = result.diagnostics().collect();
# #[cfg(feature="print_diagnostics")]
diagnostics.print();
if let Some(definition) = result.component("Foo") {
    let instance = definition.create().unwrap();
    instance.run().unwrap();
}
```

This example load a `.slint` from a string and set some properties:

```rust
# i_slint_backend_testing::init_no_event_loop();
use slint_interpreter::{ComponentDefinition, Compiler, Value, SharedString, ComponentHandle};

let code = r#"
    export component MyWin inherits Window {
        in property <string> my_name;
        Text {
            text: "Hello, " + my_name;
        }
    }
"#;

let mut compiler = Compiler::default();
let result =
    spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
assert_eq!(result.diagnostics().count(), 0);
let definition = result.component("MyWin");
let instance = definition.unwrap().create().unwrap();
instance.set_property("my_name", Value::from(SharedString::from("World"))).unwrap();
# return; // we don't want to call run in the tests
instance.run().unwrap();
```
*/
//! ## Feature flags
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]
#![warn(missing_docs)]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

#[cfg(not(feature = "compat-1-2"))]
compile_error!(
    "The feature `compat-1-2` must be enabled to ensure \
    forward compatibility with future version of this crate"
);

mod api;
mod dynamic_item_tree;
mod dynamic_type;
mod eval;
mod eval_layout;
mod global_component;
#[cfg(feature = "internal-highlight")]
pub mod highlight;
#[cfg(feature = "internal-json")]
pub mod json;
#[cfg(feature = "internal-live-preview")]
pub mod live_preview;
mod value_model;

#[doc(inline)]
pub use api::*;

#[cfg(feature = "internal")]
#[doc(hidden)]
pub use eval::default_value_for_type;

/// (Re-export from corelib.)
#[doc(inline)]
pub use i_slint_core::{Brush, Color, SharedString, SharedVector};

#[cfg(test)]
mod tests;
