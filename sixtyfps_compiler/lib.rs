/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
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

use std::{borrow::Cow, cell::RefCell, rc::Rc};

pub mod diagnostics;
pub mod expression_tree;
pub mod generator;
pub mod langtype;
pub mod layout;
pub mod lexer;
pub mod object_tree;
pub mod parser;
pub mod typeloader;
pub mod typeregister;

mod passes {
    pub mod collect_globals;
    pub mod collect_resources;
    pub mod compile_paths;
    pub mod deduplicate_property_read;
    pub mod focus_item;
    pub mod inlining;
    pub mod lower_layout;
    pub mod lower_states;
    pub mod materialize_fake_properties;
    pub mod move_declarations;
    pub mod remove_aliases;
    pub mod repeater_component;
    pub mod resolve_native_classes;
    pub mod resolving;
    pub mod unique_id;
}

#[derive(Default)]
/// CompilationConfiguration allows configuring different aspects of the compiler.
pub struct CompilerConfiguration<'a> {
    /// Indicate whether to embed resources such as images in the generated output or whether
    /// to retain references to the resources on the file system.
    pub embed_resources: bool,
    /// The compiler will look in these paths for components used in the file to compile.
    pub include_paths: &'a [std::path::PathBuf],
    /// the name of the style. (eg: "native")
    pub style: Option<&'a str>,
}

pub fn compile_syntax_node(
    doc_node: parser::SyntaxNodeWithSourceFile,
    mut diagnostics: diagnostics::FileDiagnostics,
    compiler_config: &CompilerConfiguration,
) -> (object_tree::Document, diagnostics::BuildDiagnostics) {
    let mut build_diagnostics = diagnostics::BuildDiagnostics::default();

    let global_type_registry = typeregister::TypeRegister::builtin();
    let type_registry =
        Rc::new(RefCell::new(typeregister::TypeRegister::new(&global_type_registry)));

    let doc_node: parser::syntax_nodes::Document = doc_node.into();

    let style = compiler_config
        .style
        .map(Cow::from)
        .or_else(|| std::env::var("SIXTYFPS_STYLE").map(Cow::from).ok())
        .unwrap_or_else(|| {
            let is_wasm = cfg!(target_arch = "wasm32")
                || std::env::var("TARGET").map_or(false, |t| t.starts_with("wasm"));
            if !is_wasm {
                diagnostics.push_diagnostic_with_span(
                    "SIXTYFPS_STYLE not defined, defaulting to 'ugly', see https://github.com/sixtyfpsui/sixtyfps/issues/83 for more info".to_owned(),
                    Default::default(),
                    diagnostics::Level::Warning,
                );
            }
            Cow::from("ugly")
        });

    if doc_node.source_file.is_some() {
        typeloader::load_dependencies_recursively(
            &doc_node,
            &mut diagnostics,
            &type_registry,
            &compiler_config,
            library::widget_library().iter().find(|x| x.0 == style).map(|x| x.1),
            &mut build_diagnostics,
        );
    }

    let doc = crate::object_tree::Document::from_node(doc_node, &mut diagnostics, &type_registry);

    build_diagnostics.add(diagnostics);

    if !build_diagnostics.has_error() {
        // FIXME: ideally we would be able to run more passes, but currently we panic because invariant are not met.
        run_passes(&doc, &mut build_diagnostics, compiler_config);
    }

    (doc, build_diagnostics)
}

pub fn run_passes(
    doc: &object_tree::Document,
    diag: &mut diagnostics::BuildDiagnostics,
    compiler_config: &CompilerConfiguration,
) {
    passes::resolving::resolve_expressions(doc, diag);
    passes::inlining::inline(doc);
    passes::compile_paths::compile_paths(&doc.root_component, &doc.local_registry, diag);
    passes::unique_id::assign_unique_id(&doc.root_component);
    passes::focus_item::determine_initial_focus_item(&doc.root_component, diag);
    passes::materialize_fake_properties::materialize_fake_properties(&doc.root_component);
    passes::collect_resources::collect_resources(&doc.root_component);
    doc.root_component.embed_file_resources.set(compiler_config.embed_resources);
    passes::lower_states::lower_states(&doc.root_component, diag);
    passes::repeater_component::process_repeater_components(&doc.root_component);
    passes::lower_layout::lower_layouts(&doc.root_component, diag);
    passes::deduplicate_property_read::deduplicate_property_read(&doc.root_component);
    passes::move_declarations::move_declarations(&doc.root_component, diag);
    passes::remove_aliases::remove_aliases(&doc.root_component, diag);
    passes::resolve_native_classes::resolve_native_classes(&doc.root_component);
    passes::collect_globals::collect_globals(&doc.root_component, diag);
}

mod library {
    include!(env!("SIXTYFPS_WIDGETS_LIBRARY"));
}
