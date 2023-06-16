// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
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

pub mod passes;

/// Specify how the resources are embedded by the compiler
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbedResourcesKind {
    /// Only embed builtin resources
    OnlyBuiltinResources,
    /// Embed all images resources (the content of their files)
    EmbedAllResources,
    #[cfg(feature = "software-renderer")]
    /// Embed raw texture (process images and fonts)
    EmbedTextures,
}

/// CompilationConfiguration allows configuring different aspects of the compiler.
#[derive(Clone)]
pub struct CompilerConfiguration {
    /// Indicate whether to embed resources such as images in the generated output or whether
    /// to retain references to the resources on the file system.
    pub embed_resources: EmbedResourcesKind,
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

    /// expose the accessible role and properties
    pub accessibility: bool,

    /// The domain used as one of the parameter to the translate function
    pub translation_domain: Option<String>,
}

impl CompilerConfiguration {
    pub fn new(output_format: crate::generator::OutputFormat) -> Self {
        let embed_resources = if std::env::var_os("SLINT_EMBED_TEXTURES").is_some()
            || std::env::var_os("DEP_MCU_BOARD_SUPPORT_MCU_EMBED_TEXTURES").is_some()
        {
            #[cfg(not(feature = "software-renderer"))]
            panic!("the software-renderer feature must be enabled in i-slint-compiler when embedding textures");
            #[cfg(feature = "software-renderer")]
            EmbedResourcesKind::EmbedTextures
        } else if let Ok(var) = std::env::var("SLINT_EMBED_RESOURCES") {
            let var = var.parse::<bool>().unwrap_or_else(|_|{
                panic!("SLINT_EMBED_RESOURCES has incorrect value. Must be either unset, 'true' or 'false'")
            });
            match var {
                true => EmbedResourcesKind::EmbedAllResources,
                false => EmbedResourcesKind::OnlyBuiltinResources,
            }
        } else {
            match output_format {
                #[cfg(feature = "rust")]
                crate::generator::OutputFormat::Rust => EmbedResourcesKind::EmbedAllResources,
                _ => EmbedResourcesKind::OnlyBuiltinResources,
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
            accessibility: true,
            translation_domain: None,
        }
    }
}

pub async fn compile_syntax_node(
    doc_node: parser::SyntaxNode,
    mut diagnostics: diagnostics::BuildDiagnostics,
    #[allow(unused_mut)] mut compiler_config: CompilerConfiguration,
) -> (object_tree::Document, diagnostics::BuildDiagnostics) {
    #[cfg(feature = "software-renderer")]
    if compiler_config.embed_resources == EmbedResourcesKind::EmbedTextures {
        // HACK: disable accessibility when compiling for the software renderer
        // accessibility is not supported with backend that support sofware renderer anyway
        compiler_config.accessibility = false;
    }

    let global_type_registry = typeregister::TypeRegister::builtin();
    let type_registry =
        Rc::new(RefCell::new(typeregister::TypeRegister::new(&global_type_registry)));

    let doc_node: parser::syntax_nodes::Document = doc_node.into();

    let mut loader = typeloader::TypeLoader::new(
        global_type_registry,
        compiler_config.clone(),
        &mut diagnostics,
    );

    if diagnostics.has_error() {
        return (crate::object_tree::Document::default(), diagnostics);
    }

    let (foreign_imports, reexports) =
        loader.load_dependencies_recursively(&doc_node, &mut diagnostics, &type_registry).await;

    let doc = crate::object_tree::Document::from_node(
        doc_node,
        foreign_imports,
        reexports,
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
