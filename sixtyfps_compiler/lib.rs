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

use core::future::Future;
use core::pin::Pin;
use std::cell::RefCell;
use std::rc::Rc;

pub mod diagnostics;
pub mod expression_tree;
pub mod generator;
pub mod langtype;
pub mod layout;
pub mod lexer;
pub(crate) mod load_builtins;
pub mod object_tree;
pub mod parser;
pub mod typeloader;
pub mod typeregister;

mod passes {
    pub mod apply_default_properties_from_style;
    pub mod check_expressions;
    pub mod collect_globals;
    pub mod collect_structs;
    pub mod compile_paths;
    pub mod deduplicate_property_read;
    pub mod default_geometry;
    pub mod embed_resources;
    pub mod focus_item;
    pub mod inlining;
    pub mod lower_layout;
    pub mod lower_popups;
    pub mod lower_states;
    pub mod materialize_fake_properties;
    pub mod move_declarations;
    pub mod remove_aliases;
    pub mod repeater_component;
    pub mod resolve_native_classes;
    pub mod resolving;
    pub mod unique_id;
}

/// CompilationConfiguration allows configuring different aspects of the compiler.
pub struct CompilerConfiguration {
    /// Indicate whether to embed resources such as images in the generated output or whether
    /// to retain references to the resources on the file system.
    pub embed_resources: bool,
    /// The compiler will look in these paths for components used in the file to compile.
    pub include_paths: Vec<std::path::PathBuf>,
    /// the name of the style. (eg: "native")
    pub style: Option<String>,

    /// Fallback callback to resolve a relative path to an absolute path for file imports.
    ///
    /// The callback should return the absolute path for the file specified or None if no
    /// resolution is possible.
    pub resolve_import_fallback: Option<Box<dyn Fn(String) -> Option<String>>>,

    /// Callback to load import files which is called if the file could not be found
    ///
    /// The callback should open the file specified by the given file name and
    /// return a `Ok(String)` containing the text content of the file, or a `Err(String)`
    /// containing an error message
    pub open_import_fallback:
        Option<Box<dyn Fn(String) -> Pin<Box<dyn Future<Output = std::io::Result<String>>>>>>,
}

impl CompilerConfiguration {
    pub fn new(output_format: crate::generator::OutputFormat) -> Self {
        let embed_resources = match std::env::var("SIXTYFPS_EMBED_RESOURCES") {
            Ok(var) => {
                var.parse().unwrap_or_else(|_|{
                    panic!("SIXTYFPS_EMBED_RESOURCES has incorrect value. Must be either unset, 'true' or 'false'")
                })
            }
            Err(_) => {
                match output_format {
                    #[cfg(feature = "rust")]
                    crate::generator::OutputFormat::Rust => true,
                    _ => false,
                }
            }
        };

        Self {
            embed_resources,
            include_paths: Default::default(),
            style: Default::default(),
            resolve_import_fallback: Default::default(),
            open_import_fallback: Default::default(),
        }
    }
}

pub async fn compile_syntax_node(
    doc_node: parser::SyntaxNodeWithSourceFile,
    mut diagnostics: diagnostics::FileDiagnostics,
    compiler_config: CompilerConfiguration,
) -> (object_tree::Document, diagnostics::BuildDiagnostics) {
    let mut build_diagnostics = diagnostics::BuildDiagnostics::default();

    let global_type_registry = typeregister::TypeRegister::builtin();
    let type_registry =
        Rc::new(RefCell::new(typeregister::TypeRegister::new(&global_type_registry)));

    let doc_node: parser::syntax_nodes::Document = doc_node.into();

    let mut loader =
        typeloader::TypeLoader::new(global_type_registry, &compiler_config, &mut build_diagnostics);
    if doc_node.source_file.is_some() {
        loader
            .load_dependencies_recursively(
                &doc_node,
                &mut diagnostics,
                &mut build_diagnostics,
                &type_registry,
            )
            .await;
    }

    let doc = crate::object_tree::Document::from_node(doc_node, &mut diagnostics, &type_registry);

    build_diagnostics.add(diagnostics);

    if !build_diagnostics.has_error() {
        // FIXME: ideally we would be able to run more passes, but currently we panic because invariant are not met.
        run_passes(&doc, &mut build_diagnostics, &mut loader, &compiler_config).await;
    }

    (doc, build_diagnostics)
}

pub async fn run_passes(
    doc: &object_tree::Document,
    diag: &mut diagnostics::BuildDiagnostics,
    mut type_loader: &mut typeloader::TypeLoader<'_>,
    compiler_config: &CompilerConfiguration,
) {
    passes::resolving::resolve_expressions(doc, &type_loader, diag);
    passes::inlining::inline(doc);
    passes::check_expressions::check_expressions(doc, diag);
    passes::compile_paths::compile_paths(&doc.root_component, &doc.local_registry, diag);
    passes::unique_id::assign_unique_id(&doc.root_component);
    passes::focus_item::determine_initial_focus_item(&doc.root_component, diag);
    passes::materialize_fake_properties::materialize_fake_properties(&doc.root_component);
    if compiler_config.embed_resources {
        passes::embed_resources::embed_resources(&doc.root_component);
    }
    passes::lower_states::lower_states(&doc.root_component, &doc.local_registry, diag);
    passes::repeater_component::process_repeater_components(&doc.root_component);
    passes::lower_popups::lower_popups(&doc.root_component, &doc.local_registry, diag);
    passes::lower_layout::lower_layouts(&doc.root_component, &mut type_loader, diag).await;
    passes::default_geometry::default_geometry(&doc.root_component);
    passes::apply_default_properties_from_style::apply_default_properties_from_style(
        &doc.root_component,
        &mut type_loader,
        diag,
    )
    .await;
    passes::deduplicate_property_read::deduplicate_property_read(&doc.root_component);
    passes::move_declarations::move_declarations(&doc.root_component, diag);
    passes::remove_aliases::remove_aliases(&doc.root_component, diag);
    passes::resolve_native_classes::resolve_native_classes(&doc.root_component);
    passes::collect_globals::collect_globals(&doc.root_component, diag);
    passes::collect_structs::collect_structs(&doc.root_component, diag);

    fn generate_item_indices(component: &Rc<object_tree::Component>) {
        let mut current_item_index: usize = 0;
        generator::build_array_helper(&component, move |item_rc, _, is_flickable_rect| {
            let item = item_rc.borrow();
            if is_flickable_rect {
                current_item_index += 1;
            } else if item.base_type == crate::langtype::Type::Void {
            } else if item.repeated.is_some() {
                generate_item_indices(&*item.base_type.as_component());
                current_item_index += 1;
            } else {
                item.item_index.set(current_item_index).unwrap();
                current_item_index += 1;
            }
        });
    }

    generate_item_indices(&doc.root_component);
}

mod library {
    include!(env!("SIXTYFPS_WIDGETS_LIBRARY"));
}
