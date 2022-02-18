// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]
// It would be nice to keep the compiler free of unsafe code
#![deny(unsafe_code)]

#[cfg(feature = "proc_macro_span")]
extern crate proc_macro;

use core::future::Future;
use core::pin::Pin;
use std::cell::RefCell;
use std::rc::Rc;

pub mod builtin_macros;
pub mod diagnostics;
pub mod embedded_resources;
pub mod expression_tree;
pub mod fileaccess;
pub mod generator;
pub mod langtype;
pub mod layout;
pub mod lexer;
pub mod literals;
pub mod llr;
pub(crate) mod load_builtins;
pub mod lookup;
pub mod namedreference;
pub mod object_tree;
pub mod parser;
pub mod typeloader;
pub mod typeregister;

mod passes;

/// CompilationConfiguration allows configuring different aspects of the compiler.
#[derive(Clone)]
pub struct CompilerConfiguration {
    /// Indicate whether to embed resources such as images in the generated output or whether
    /// to retain references to the resources on the file system.
    pub embed_resources: bool,
    /// The compiler will look in these paths for components used in the file to compile.
    pub include_paths: Vec<std::path::PathBuf>,
    /// the name of the style. (eg: "native")
    pub style: Option<String>,

    /// Callback to load import files which is called if the file could not be found
    ///
    /// The callback should open the file specified by the given file name and
    /// return an future that provides the text content of the file as output.
    pub open_import_fallback: Option<
        Rc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<std::io::Result<String>>>>>>,
    >,

    /// Run the pass that inlines all the elements.
    ///
    /// This may help optimization to optimize the runtime resources usages,
    /// but at the cost of much more generated code and binary size.
    pub inline_all_elements: bool,

    /// Compile time scale factor to apply to embedded resources such as images and glyphs.
    pub scale_factor: f64,
}

impl CompilerConfiguration {
    pub fn new(output_format: crate::generator::OutputFormat) -> Self {
        let embed_resources = match std::env::var("SLINT_EMBED_RESOURCES") {
            Ok(var) => {
                var.parse().unwrap_or_else(|_|{
                    panic!("SLINT_EMBED_RESOURCES has incorrect value. Must be either unset, 'true' or 'false'")
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

        let inline_all_elements = match std::env::var("SLINT_INLINING") {
            Ok(var) => var.parse::<bool>().unwrap_or_else(|_| {
                panic!(
                    "SLINT_INLINING has incorrect value. Must be either unset, 'true' or 'false'"
                )
            }),
            // Currently, the interpreter needs the inlining to be on.
            Err(_) => output_format == crate::generator::OutputFormat::Interpreter,
        };

        let scale_factor = std::env::var("SLINT_SCALE_FACTOR")
            .ok()
            .and_then(|x| x.parse::<f64>().ok())
            .filter(|f| *f > 0.)
            .unwrap_or(1.);

        Self {
            embed_resources,
            include_paths: Default::default(),
            style: Default::default(),
            open_import_fallback: Default::default(),
            inline_all_elements,
            scale_factor,
        }
    }
}

pub async fn compile_syntax_node(
    doc_node: parser::SyntaxNode,
    mut diagnostics: diagnostics::BuildDiagnostics,
    compiler_config: CompilerConfiguration,
) -> (object_tree::Document, diagnostics::BuildDiagnostics) {
    let global_type_registry = typeregister::TypeRegister::builtin();
    let type_registry =
        Rc::new(RefCell::new(typeregister::TypeRegister::new(&global_type_registry)));

    let doc_node: parser::syntax_nodes::Document = doc_node.into();

    let mut loader =
        typeloader::TypeLoader::new(global_type_registry, &compiler_config, &mut diagnostics);

    if diagnostics.has_error() {
        return (crate::object_tree::Document::default(), diagnostics);
    }

    let foreign_imports =
        loader.load_dependencies_recursively(&doc_node, &mut diagnostics, &type_registry).await;

    let doc = crate::object_tree::Document::from_node(
        doc_node,
        foreign_imports,
        &mut diagnostics,
        &type_registry,
    );

    if let Some((_, node)) = &*doc.root_component.child_insertion_point.borrow() {
        diagnostics
            .push_error("@children placeholder not allowed in the final component".into(), node)
    }

    if !diagnostics.has_error() {
        // FIXME: ideally we would be able to run more passes, but currently we panic because invariant are not met.
        passes::run_passes(&doc, &mut diagnostics, &mut loader, &compiler_config).await;
    }

    diagnostics.all_loaded_files = loader.all_files().cloned().collect();

    (doc, diagnostics)
}
