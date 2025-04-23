// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
// It would be nice to keep the compiler free of unsafe code
#![deny(unsafe_code)]

#[cfg(feature = "proc_macro_span")]
extern crate proc_macro;

use core::future::Future;
use core::pin::Pin;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
#[cfg(feature = "software-renderer")]
use std::sync::Arc;

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
pub mod pathutils;
#[cfg(feature = "bundle-translations")]
pub mod translations;
pub mod typeloader;
pub mod typeregister;

pub mod passes;

use crate::generator::OutputFormat;
use std::path::Path;

/// Specify how the resources are embedded by the compiler
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbedResourcesKind {
    /// Embeds nothing (only useful for interpreter)
    Nothing,
    /// Only embed builtin resources
    OnlyBuiltinResources,
    /// Do not embed resources, but list them in the Document as it they were embedded
    ListAllResources,
    /// Embed all images resources (the content of their files)
    EmbedAllResources,
    #[cfg(feature = "software-renderer")]
    /// Embed raw texture (process images and fonts)
    EmbedTextures,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
#[non_exhaustive]
pub enum ComponentSelection {
    /// All components that inherit from Window.
    ///
    /// Note: Components marked for export but lacking Window inheritance are not selected (this will produce a warning),
    /// For compatibility reason, the last exported component is still selected even if it doesn't inherit Window,
    /// and if no component is exported, the last component is selected
    #[default]
    ExportedWindows,

    /// The Last component (legacy for the viewer / interpreter)
    ///
    /// Only the last exported component is generated, regardless if this is a Window or not,
    /// (and it will be transformed in a Window)
    LastExported,

    /// The component with the given name is generated
    Named(String),
}

#[cfg(feature = "software-renderer")]
pub type FontCache = Rc<
    RefCell<
        std::collections::HashMap<
            i_slint_common::sharedfontdb::fontdb::ID,
            fontdue::FontResult<(Arc<fontdue::Font>, Arc<dyn AsRef<[u8]> + Send + Sync>, u32)>,
        >,
    >,
>;

/// CompilationConfiguration allows configuring different aspects of the compiler.
#[derive(Clone)]
pub struct CompilerConfiguration {
    /// Indicate whether to embed resources such as images in the generated output or whether
    /// to retain references to the resources on the file system.
    pub embed_resources: EmbedResourcesKind,
    /// Whether to use SDF when pre-rendering fonts.
    #[cfg(all(feature = "software-renderer", feature = "sdf-fonts"))]
    pub use_sdf_fonts: bool,
    /// The compiler will look in these paths for components used in the file to compile.
    pub include_paths: Vec<std::path::PathBuf>,
    /// The compiler will look in these paths for library imports.
    pub library_paths: HashMap<String, std::path::PathBuf>,
    /// the name of the style. (eg: "native")
    pub style: Option<String>,

    /// Callback to load import files which is called if the file could not be found
    ///
    /// The callback should open the file specified by the given file name and
    /// return an future that provides the text content of the file as output.
    pub open_import_fallback: Option<
        Rc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<std::io::Result<String>>>>>>,
    >,
    /// Callback to map URLs for resources
    ///
    /// The function takes the url and returns the mapped URL (or None if not mapped)
    pub resource_url_mapper:
        Option<Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Option<String>>>>>>,

    /// Run the pass that inlines all the elements.
    ///
    /// This may help optimization to optimize the runtime resources usages,
    /// but at the cost of much more generated code and binary size.
    pub inline_all_elements: bool,

    /// Compile time scale factor to apply to embedded resources such as images and glyphs.
    /// If != 1.0 then the scale factor will be set on the `slint::Window`.
    pub const_scale_factor: f64,

    /// expose the accessible role and properties
    pub accessibility: bool,

    /// Add support for experimental features
    pub enable_experimental: bool,

    /// The domain used as one of the parameter to the translate function
    pub translation_domain: Option<String>,
    /// When Some, this is the path where the translations are looked at to bundle the translations
    #[cfg(feature = "bundle-translations")]
    pub translation_path_bundle: Option<std::path::PathBuf>,

    /// Do not generate the hook to create native menus
    pub no_native_menu: bool,

    /// C++ namespace
    pub cpp_namespace: Option<String>,

    /// Generate debug information for elements (ids, type names)
    pub debug_info: bool,

    /// Generate debug hooks to inspect/override properties.
    pub debug_hooks: Option<std::hash::RandomState>,

    pub components_to_generate: ComponentSelection,

    #[cfg(feature = "software-renderer")]
    pub font_cache: FontCache,
}

impl CompilerConfiguration {
    pub fn new(output_format: OutputFormat) -> Self {
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
                OutputFormat::Rust => EmbedResourcesKind::EmbedAllResources,
                OutputFormat::Interpreter => EmbedResourcesKind::Nothing,
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
            Err(_) => output_format == OutputFormat::Interpreter,
        };

        let const_scale_factor = std::env::var("SLINT_SCALE_FACTOR")
            .ok()
            .and_then(|x| x.parse::<f64>().ok())
            .filter(|f| *f > 0.)
            .unwrap_or(1.);

        let enable_experimental = std::env::var_os("SLINT_ENABLE_EXPERIMENTAL_FEATURES").is_some();

        let debug_info = std::env::var_os("SLINT_EMIT_DEBUG_INFO").is_some();

        let cpp_namespace = match output_format {
            #[cfg(feature = "cpp")]
            OutputFormat::Cpp(config) => match config.namespace {
                Some(namespace) => Some(namespace),
                None => match std::env::var("SLINT_CPP_NAMESPACE") {
                    Ok(namespace) => Some(namespace),
                    Err(_) => None,
                },
            },
            _ => None,
        };

        Self {
            embed_resources,
            include_paths: Default::default(),
            library_paths: Default::default(),
            style: Default::default(),
            open_import_fallback: None,
            resource_url_mapper: None,
            inline_all_elements,
            const_scale_factor,
            accessibility: true,
            enable_experimental,
            translation_domain: None,
            no_native_menu: false,
            cpp_namespace,
            debug_info,
            debug_hooks: None,
            components_to_generate: ComponentSelection::ExportedWindows,
            #[cfg(feature = "software-renderer")]
            font_cache: Default::default(),
            #[cfg(all(feature = "software-renderer", feature = "sdf-fonts"))]
            use_sdf_fonts: false,
            #[cfg(feature = "bundle-translations")]
            translation_path_bundle: std::env::var("SLINT_BUNDLE_TRANSLATIONS")
                .ok()
                .map(|x| x.into()),
        }
    }

    #[cfg(feature = "software-renderer")]
    fn load_font_by_id(
        &self,
        face_id: i_slint_common::sharedfontdb::fontdb::ID,
    ) -> fontdue::FontResult<(Arc<fontdue::Font>, Arc<dyn AsRef<[u8]> + Send + Sync>, u32)> {
        self.font_cache
            .borrow_mut()
            .entry(face_id)
            .or_insert_with(|| {
                i_slint_common::sharedfontdb::FONT_DB.with(|fontdb| {
                    fontdb
                        .borrow()
                        .with_face_data(face_id, |font_data, face_index| {
                            fontdue::Font::from_bytes(
                                font_data,
                                fontdue::FontSettings {
                                    collection_index: face_index,
                                    scale: 40.,
                                    ..Default::default()
                                },
                            )
                            .map(|fontdue_font| {
                                (
                                    Arc::new(fontdue_font),
                                    Arc::new(font_data.to_vec())
                                        as Arc<dyn AsRef<[u8]> + Send + Sync>,
                                    face_index,
                                )
                            })
                        })
                        .unwrap_or_else(|| fontdue::FontResult::Err("internal error: corrupt font"))
                })
            })
            .clone()
    }
}

fn prepare_for_compile(
    diagnostics: &mut diagnostics::BuildDiagnostics,
    #[allow(unused_mut)] mut compiler_config: CompilerConfiguration,
) -> typeloader::TypeLoader {
    #[cfg(feature = "software-renderer")]
    if compiler_config.embed_resources == EmbedResourcesKind::EmbedTextures {
        // HACK: disable accessibility when compiling for the software renderer
        // accessibility is not supported with backend that support software renderer anyway
        compiler_config.accessibility = false;
    }

    diagnostics.enable_experimental = compiler_config.enable_experimental;

    let global_type_registry = if compiler_config.enable_experimental {
        crate::typeregister::TypeRegister::builtin_experimental()
    } else {
        crate::typeregister::TypeRegister::builtin()
    };

    typeloader::TypeLoader::new(global_type_registry, compiler_config, diagnostics)
}

pub async fn compile_syntax_node(
    doc_node: parser::SyntaxNode,
    mut diagnostics: diagnostics::BuildDiagnostics,
    #[allow(unused_mut)] mut compiler_config: CompilerConfiguration,
) -> (object_tree::Document, diagnostics::BuildDiagnostics, typeloader::TypeLoader) {
    let mut loader = prepare_for_compile(&mut diagnostics, compiler_config);

    let doc_node: parser::syntax_nodes::Document = doc_node.into();

    let type_registry =
        Rc::new(RefCell::new(typeregister::TypeRegister::new(&loader.global_type_registry)));
    let (foreign_imports, reexports) =
        loader.load_dependencies_recursively(&doc_node, &mut diagnostics, &type_registry).await;

    let mut doc = crate::object_tree::Document::from_node(
        doc_node,
        foreign_imports,
        reexports,
        &mut diagnostics,
        &type_registry,
    );

    if !diagnostics.has_errors() {
        passes::run_passes(&mut doc, &mut loader, false, &mut diagnostics).await;
    } else {
        // Don't run all the passes in case of errors because because some invariants are not met.
        passes::run_import_passes(&doc, &loader, &mut diagnostics);
    }
    (doc, diagnostics, loader)
}

/// Pass a file to the compiler and process it fully, applying all the
/// necessary compilation passes.
///
/// This returns a `Tuple` containing the actual cleaned `path` to the file,
/// a set of `BuildDiagnostics` and a `TypeLoader` with all compilation passes applied.
pub async fn load_root_file(
    path: &Path,
    source_path: &Path,
    source_code: String,
    mut diagnostics: diagnostics::BuildDiagnostics,
    #[allow(unused_mut)] mut compiler_config: CompilerConfiguration,
) -> (std::path::PathBuf, diagnostics::BuildDiagnostics, typeloader::TypeLoader) {
    let mut loader = prepare_for_compile(&mut diagnostics, compiler_config);

    let (path, _) =
        loader.load_root_file(path, source_path, source_code, false, &mut diagnostics).await;

    (path, diagnostics, loader)
}

/// Pass a file to the compiler and process it fully, applying all the
/// necessary compilation passes, just like `load_root_file`.
///
/// This returns a `Tuple` containing the actual cleaned `path` to the file,
/// a set of `BuildDiagnostics`, a `TypeLoader` with all compilation passes
/// applied and another `TypeLoader` with a minimal set of passes applied to it.
pub async fn load_root_file_with_raw_type_loader(
    path: &Path,
    source_path: &Path,
    source_code: String,
    mut diagnostics: diagnostics::BuildDiagnostics,
    #[allow(unused_mut)] mut compiler_config: CompilerConfiguration,
) -> (
    std::path::PathBuf,
    diagnostics::BuildDiagnostics,
    typeloader::TypeLoader,
    Option<typeloader::TypeLoader>,
) {
    let mut loader = prepare_for_compile(&mut diagnostics, compiler_config);

    let (path, raw_type_loader) =
        loader.load_root_file(path, source_path, source_code, true, &mut diagnostics).await;

    (path, diagnostics, loader, raw_type_loader)
}
