/*!
# The SixtyFPS compiler library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead

*/

// It would be nice to keep the compiler free of unsafe code
#![deny(unsafe_code)]

#[cfg(feature = "proc_macro_span")]
extern crate proc_macro;

pub mod diagnostics;
pub mod expression_tree;
pub mod generator;
pub mod layout;
pub mod object_tree;
pub mod parser;
pub mod typeregister;

mod passes {
    pub mod collect_resources;
    pub mod compile_paths;
    pub mod inlining;
    pub mod lower_layout;
    pub mod move_declarations;
    pub mod repeater_component;
    pub mod resolving;
    pub mod unique_id;
}

#[derive(Default)]
pub struct CompilerConfiguration {
    pub embed_resources: bool,
}

pub fn run_passes(
    doc: &object_tree::Document,
    diag: &mut diagnostics::Diagnostics,
    tr: &mut typeregister::TypeRegister,
    compiler_config: &CompilerConfiguration,
) {
    passes::resolving::resolve_expressions(doc, diag, tr);
    passes::inlining::inline(doc);
    passes::lower_layout::lower_layouts(&doc.root_component, diag);
    passes::compile_paths::compile_paths(&doc.root_component , diag);
    passes::unique_id::assign_unique_id(&doc.root_component);
    if compiler_config.embed_resources {
        passes::collect_resources::collect_resources(&doc.root_component);
    }
    passes::repeater_component::create_repeater_components(&doc.root_component, diag);
    passes::move_declarations::move_declarations(&doc.root_component);
}
