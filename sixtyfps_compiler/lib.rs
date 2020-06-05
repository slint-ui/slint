/*!
# The SixtyFPS compiler library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead

The different modules take the source code and transform into data structures
according to the following schema

```text
source code -> parser -> object_tree -> lower -> generator
```

*/

#[cfg(feature = "proc_macro_span")]
extern crate proc_macro;

pub mod diagnostics;
pub mod expression_tree;
pub mod generator;
pub mod object_tree;
pub mod parser;
pub mod typeregister;

mod passes {
    pub mod inlining;
    pub mod move_declarations;
    pub mod resolving;
    pub mod unique_id;
}

pub fn run_passes(
    doc: &object_tree::Document,
    diag: &mut diagnostics::Diagnostics,
    tr: &mut typeregister::TypeRegister,
) {
    passes::resolving::resolve_expressions(doc, diag, tr);
    passes::inlining::inline(doc);
    passes::unique_id::assign_unique_id(&doc.root_component);
    passes::move_declarations::move_declarations(&doc.root_component);
}
