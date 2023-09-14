// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::object_tree::{self, Document, ExportedName, Exports};
use crate::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxToken};
use crate::typeregister::TypeRegister;
use crate::CompilerConfiguration;
use crate::{fileaccess, parser};
use core::future::Future;

/// Storage for a cache of all loaded documents
#[derive(Default)]
struct LoadedDocuments {
    /// maps from the canonical file name to the object_tree::Document
    docs: HashMap<PathBuf, Document>,
    /// The .slint files that are currently being loaded, potentially asynchronously.
    /// When a task start loading a file, it will add an empty vector to this map, and
    /// the same task will remove the entry from the map when finished, and awake all
    /// wakers.
    currently_loading: HashMap<PathBuf, Vec<std::task::Waker>>,
}

pub enum ImportKind {
    ImportList(syntax_nodes::ImportSpecifier),
    ModuleReexport(syntax_nodes::ExportModule), // re-export all types, as per export * from "foo".
}

pub struct ImportedTypes {
    pub import_uri_token: SyntaxToken,
    pub import_kind: ImportKind,
    pub file: String,
}

#[derive(Debug)]
pub struct ImportedName {
    // name of export to match in the other file
    pub external_name: String,
    // name to be used locally
    pub internal_name: String,
}

impl ImportedName {
    pub fn extract_imported_names(
        import: &syntax_nodes::ImportSpecifier,
    ) -> impl Iterator<Item = ImportedName> + '_ {
        import.ImportIdentifierList().into_iter().flat_map(|import_identifiers| {
            import_identifiers.ImportIdentifier().map(Self::from_node)
        })
    }

    pub fn from_node(importident: syntax_nodes::ImportIdentifier) -> Self {
        let external_name =
            parser::normalize_identifier(importident.ExternalName().text().to_string().trim());

        let internal_name = match importident.InternalName() {
            Some(name_ident) => parser::normalize_identifier(name_ident.text().to_string().trim()),
            None => external_name.clone(),
        };

        ImportedName { internal_name, external_name }
    }
}

pub struct TypeLoader {
    pub global_type_registry: Rc<RefCell<TypeRegister>>,
    pub compiler_config: CompilerConfiguration,
    style: String,
    all_documents: LoadedDocuments,
}

struct BorrowedTypeLoader<'a> {
    tl: &'a mut TypeLoader,
    diag: &'a mut BuildDiagnostics,
}

impl TypeLoader {
    pub fn new(
        global_type_registry: Rc<RefCell<TypeRegister>>,
        compiler_config: CompilerConfiguration,
        diag: &mut BuildDiagnostics,
    ) -> Self {
        let style = compiler_config
        .style
        .clone()
        .or_else(|| std::env::var("SLINT_STYLE").ok())
        .unwrap_or_else(|| {
            let is_wasm = cfg!(target_arch = "wasm32")
                || std::env::var("TARGET").map_or(false, |t| t.starts_with("wasm"));
            if !is_wasm {
                diag.push_diagnostic_with_span("SLINT_STYLE not defined, defaulting to 'fluent', see https://github.com/slint-ui/slint/issues/83 for more info".to_owned(),
                    Default::default(),
                    crate::diagnostics::DiagnosticLevel::Warning
                );
            }
            String::from("fluent")
        });

        let myself = Self {
            global_type_registry,
            compiler_config,
            style: style.clone(),
            all_documents: Default::default(),
        };

        let known_styles = fileaccess::styles();
        if !known_styles.contains(&style.as_ref())
            && myself
                .find_file_in_include_path(None, &format!("{}/std-widgets.slint", style))
                .is_none()
        {
            diag.push_diagnostic_with_span(
                format!(
                    "Style {} in not known. Use one of the builtin styles [{}] or make sure your custom style is found in the include directories",
                    &style,
                    known_styles.join(", ")
                ),
                Default::default(),
                crate::diagnostics::DiagnosticLevel::Error,
            );
        }

        myself
    }

    /// Imports of files that don't have the .slint extension are returned.
    pub async fn load_dependencies_recursively<'a>(
        &'a mut self,
        doc: &'a syntax_nodes::Document,
        diag: &'a mut BuildDiagnostics,
        registry_to_populate: &'a Rc<RefCell<TypeRegister>>,
    ) -> (Vec<ImportedTypes>, Exports) {
        let state = RefCell::new(BorrowedTypeLoader { tl: self, diag });
        Self::load_dependencies_recursively_impl(
            &state,
            doc,
            registry_to_populate,
            &Default::default(),
        )
        .await
    }
    fn load_dependencies_recursively_impl<'a: 'b, 'b>(
        state: &'a RefCell<BorrowedTypeLoader<'a>>,
        doc: &'b syntax_nodes::Document,
        registry_to_populate: &'b Rc<RefCell<TypeRegister>>,
        import_stack: &'b HashSet<PathBuf>,
    ) -> core::pin::Pin<Box<dyn std::future::Future<Output = (Vec<ImportedTypes>, Exports)> + 'b>>
    {
        let mut foreign_imports = vec![];
        let mut dependencies = Self::collect_dependencies(state, doc)
            .filter_map(|mut import| {
                if import.file.ends_with(".60") || import.file.ends_with(".slint") {
                    Some(Box::pin(async move {
                        let file = import.file.as_str();
                        let doc_path = Self::ensure_document_loaded(
                            state,
                            file,
                            Some(import.import_uri_token.clone().into()),
                            import_stack.clone()
                        )
                        .await?;
                        let mut state = state.borrow_mut();
                        let state = &mut *state;
                        let doc = state.tl.all_documents.docs.get(&doc_path).unwrap();
                        match &import.import_kind {
                            ImportKind::ImportList(imported_types) => {
                                let mut imported_types =
                                    ImportedName::extract_imported_names(imported_types).peekable();
                                if imported_types.peek().is_some() {
                                    Self::register_imported_types(doc, &import, imported_types, registry_to_populate, state.diag);
                                } else {
                                    state.diag.push_error("Import names are missing. Please specify which types you would like to import".into(), &import.import_uri_token.parent());
                                }
                                None
                            }
                            ImportKind::ModuleReexport(export_module_syntax_node) => {
                                let mut exports = Exports::default();
                                exports.add_reexports(
                                    doc.exports.iter().map(|(exported_name, compo_or_type)| {
                                        (
                                            ExportedName {
                                                name: exported_name.name.clone(),
                                                name_ident: (**export_module_syntax_node).clone(),
                                            },
                                            compo_or_type.clone(),
                                        )
                                    }),
                                    state.diag,
                                );
                                Some((exports, export_module_syntax_node.clone()))
                            }
                        }
                    }))
                } else {
                    if let Some((path, _)) = state.borrow().tl.resolve_import_path(Some(&import.import_uri_token.clone().into()), &import.file) {
                        import.file = path.to_string_lossy().to_string();
                    };

                    foreign_imports.push(import);
                    None
                }
            })
            .collect::<Vec<_>>();

        Box::pin(async move {
            let mut reexports = None;
            std::future::poll_fn(|cx| {
                dependencies.retain_mut(|fut| {
                    let core::task::Poll::Ready(export) = fut.as_mut().poll(cx) else {
                        return true;
                    };
                    let Some((exports, node)) = export else { return false };
                    if reexports.is_none() {
                        reexports = Some(exports);
                    } else {
                        state.borrow_mut().diag.push_error(
                            "re-exporting modules is only allowed once per file".into(),
                            &node,
                        );
                    };
                    false
                });
                if dependencies.is_empty() {
                    core::task::Poll::Ready(())
                } else {
                    core::task::Poll::Pending
                }
            })
            .await;
            (foreign_imports, reexports.unwrap_or_default())
        })
    }

    pub async fn import_component(
        &mut self,
        file_to_import: &str,
        type_name: &str,
        diag: &mut BuildDiagnostics,
    ) -> Option<Rc<object_tree::Component>> {
        let state = RefCell::new(BorrowedTypeLoader { tl: self, diag });
        let doc_path =
            match Self::ensure_document_loaded(&state, file_to_import, None, Default::default())
                .await
            {
                Some(doc_path) => doc_path,
                None => return None,
            };

        let doc = self.all_documents.docs.get(&doc_path).unwrap();

        doc.exports.find(type_name).and_then(|compo_or_type| compo_or_type.left())
    }

    /// Append a possibly relative path to a base path. Returns the data if it resolves to a built-in (compiled-in)
    /// file.
    pub fn resolve_import_path(
        &self,
        import_token: Option<&NodeOrToken>,
        maybe_relative_path_or_url: &str,
    ) -> Option<(PathBuf, Option<&'static [u8]>)> {
        let referencing_file_or_url =
            import_token.and_then(|tok| tok.source_file().map(|s| s.path()));

        self.find_file_in_include_path(referencing_file_or_url, maybe_relative_path_or_url).or_else(
            || {
                referencing_file_or_url
                    .and_then(|base_path_or_url| {
                        let base_path_or_url_str = base_path_or_url.to_string_lossy();
                        if base_path_or_url_str.contains("://") {
                            url::Url::parse(&base_path_or_url_str).ok().and_then(|base_url| {
                                base_url
                                    .join(maybe_relative_path_or_url)
                                    .ok()
                                    .map(|url| url.to_string().into())
                            })
                        } else {
                            base_path_or_url.parent().and_then(|base_dir| {
                                dunce::canonicalize(base_dir.join(maybe_relative_path_or_url)).ok()
                            })
                        }
                    })
                    .map(|p| (p, None))
            },
        )
    }

    async fn ensure_document_loaded<'a: 'b, 'b>(
        state: &'a RefCell<BorrowedTypeLoader<'a>>,
        file_to_import: &'b str,
        import_token: Option<NodeOrToken>,
        mut import_stack: HashSet<PathBuf>,
    ) -> Option<PathBuf> {
        let mut borrowed_state = state.borrow_mut();

        let (path_canon, builtin) = match borrowed_state
            .tl
            .resolve_import_path(import_token.as_ref(), file_to_import)
        {
            Some(x) => x,
            None => match dunce::canonicalize(file_to_import) {
                Ok(path) => {
                    if import_token.as_ref().and_then(|x| x.source_file()).is_some() {
                        borrowed_state.diag.push_warning(
                        format!(
                            "Loading \"{file_to_import}\" relative to the work directory is deprecated. Files should be imported relative to their import location",
                        ),
                        &import_token,
                    );
                    }
                    (path, None)
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    // We will load using the `open_import_fallback`
                    // Simplify the path to remove the ".."
                    let mut path = import_token
                        .as_ref()
                        .and_then(|tok| tok.source_file().map(|s| s.path()))
                        .and_then(|p| p.parent())
                        .map_or(PathBuf::new(), |p| p.into());
                    for c in Path::new(file_to_import).components() {
                        use std::path::Component::*;
                        match c {
                            RootDir => path = PathBuf::new(),
                            CurDir => {}
                            ParentDir
                                if matches!(
                                    path.components().last(),
                                    Some(Normal(_) | RootDir)
                                ) =>
                            {
                                path.pop();
                            }
                            Prefix(_) | ParentDir | Normal(_) => path.push(c),
                        }
                    }
                    (path, None)
                }
                Err(err) => {
                    borrowed_state.diag.push_error(
                        format!("Error reading requested import \"{file_to_import}\": {err}",),
                        &import_token,
                    );
                    return None;
                }
            },
        };

        if !import_stack.insert(path_canon.clone()) {
            borrowed_state.diag.push_error(
                format!("Recursive import of \"{}\"", path_canon.display()),
                &import_token,
            );
            return None;
        }
        drop(borrowed_state);

        let is_loaded = core::future::poll_fn(|cx| {
            let mut state = state.borrow_mut();
            let all_documents = &mut state.tl.all_documents;
            match all_documents.currently_loading.entry(path_canon.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let waker = cx.waker();
                    if !e.get().iter().any(|w| w.will_wake(waker)) {
                        e.get_mut().push(cx.waker().clone());
                    }
                    core::task::Poll::Pending
                }
                std::collections::hash_map::Entry::Vacant(v) => {
                    if all_documents.docs.get(path_canon.as_path()).is_some() {
                        core::task::Poll::Ready(true)
                    } else {
                        v.insert(Default::default());
                        core::task::Poll::Ready(false)
                    }
                }
            }
        })
        .await;
        if is_loaded {
            return Some(path_canon);
        }

        let source_code_result = if let Some(builtin) = builtin {
            Ok(String::from(
                core::str::from_utf8(builtin)
                    .expect("internal error: embedded file is not UTF-8 source code"),
            ))
        } else if let Some(fallback) = {
            let fallback = state.borrow().tl.compiler_config.open_import_fallback.clone();
            fallback
        } {
            let result = fallback(path_canon.to_string_lossy().into()).await;
            result.unwrap_or_else(|| std::fs::read_to_string(&path_canon))
        } else {
            std::fs::read_to_string(&path_canon)
        };

        let ok = match source_code_result {
            Ok(source) => {
                Self::load_file_impl(
                    state,
                    &path_canon,
                    &path_canon,
                    source,
                    builtin.is_some(),
                    &import_stack,
                )
                .await;
                true
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                state.borrow_mut().diag.push_error(
                        format!(
                            "Cannot find requested import \"{file_to_import}\" in the include search path",
                        ),
                        &import_token,
                    );
                false
            }
            Err(err) => {
                state.borrow_mut().diag.push_error(
                    format!("Error reading requested import \"{}\": {}", path_canon.display(), err),
                    &import_token,
                );
                false
            }
        };

        let wakers = state
            .borrow_mut()
            .tl
            .all_documents
            .currently_loading
            .remove(path_canon.as_path())
            .unwrap();
        for x in wakers {
            x.wake();
        }

        ok.then_some(path_canon)
    }

    /// Load a file, and its dependency not run the passes.
    ///
    /// the path must be the canonical path
    pub async fn load_file(
        &mut self,
        path: &Path,
        source_path: &Path,
        source_code: String,
        is_builtin: bool,
        diag: &mut BuildDiagnostics,
    ) {
        let state = RefCell::new(BorrowedTypeLoader { tl: self, diag });
        Self::load_file_impl(
            &state,
            path,
            source_path,
            source_code,
            is_builtin,
            &Default::default(),
        )
        .await
    }

    async fn load_file_impl<'a>(
        state: &'a RefCell<BorrowedTypeLoader<'a>>,
        path: &Path,
        source_path: &Path,
        source_code: String,
        is_builtin: bool,
        import_stack: &HashSet<PathBuf>,
    ) {
        let dependency_doc: syntax_nodes::Document =
            crate::parser::parse(source_code, Some(source_path), state.borrow_mut().diag).into();

        let dependency_registry =
            Rc::new(RefCell::new(TypeRegister::new(&state.borrow().tl.global_type_registry)));
        dependency_registry.borrow_mut().expose_internal_types = is_builtin;
        let (foreign_imports, reexports) = Self::load_dependencies_recursively_impl(
            state,
            &dependency_doc,
            &dependency_registry,
            import_stack,
        )
        .await;

        if state.borrow().diag.has_error() {
            // If there was error (esp parse error) we don't want to report further error in this document.
            // because they might be nonsense (TODO: we should check that the parse error were really in this document).
            // But we still want to create a document to give better error messages in the root document.
            let mut ignore_diag = BuildDiagnostics::default();
            ignore_diag.push_error_with_span(
                "Dummy error because some of the code asserts there was an error".into(),
                Default::default(),
            );
            let doc = crate::object_tree::Document::from_node(
                dependency_doc,
                foreign_imports,
                reexports,
                &mut ignore_diag,
                &dependency_registry,
            );
            state.borrow_mut().tl.all_documents.docs.insert(path.to_owned(), doc);
            return;
        }
        let mut state = state.borrow_mut();
        let state = &mut *state;
        let doc = crate::object_tree::Document::from_node(
            dependency_doc,
            foreign_imports,
            reexports,
            state.diag,
            &dependency_registry,
        );
        crate::passes::run_import_passes(&doc, state.tl, state.diag);
        state.tl.all_documents.docs.insert(path.to_owned(), doc);
    }

    fn register_imported_types(
        doc: &Document,
        import: &ImportedTypes,
        imported_types: impl Iterator<Item = ImportedName>,
        registry_to_populate: &Rc<RefCell<TypeRegister>>,
        build_diagnostics: &mut BuildDiagnostics,
    ) {
        for import_name in imported_types {
            let imported_type = doc.exports.find(&import_name.external_name);

            let imported_type = match imported_type {
                Some(ty) => ty,
                None => {
                    build_diagnostics.push_error(
                        format!(
                            "No exported type called '{}' found in \"{}\"",
                            import_name.external_name, import.file
                        ),
                        &import.import_uri_token,
                    );
                    continue;
                }
            };

            match imported_type {
                itertools::Either::Left(c) => {
                    registry_to_populate.borrow_mut().add_with_name(import_name.internal_name, c)
                }
                itertools::Either::Right(ty) => registry_to_populate
                    .borrow_mut()
                    .insert_type_with_name(ty, import_name.internal_name),
            }
        }
    }

    /// Lookup a filename and try to find the absolute filename based on the include path or
    /// the current file directory
    pub fn find_file_in_include_path(
        &self,
        referencing_file: Option<&Path>,
        file_to_import: &str,
    ) -> Option<(PathBuf, Option<&'static [u8]>)> {
        // The directory of the current file is the first in the list of include directories.
        let maybe_current_directory = referencing_file.and_then(base_directory);
        maybe_current_directory
            .clone()
            .into_iter()
            .chain(self.compiler_config.include_paths.iter().map(PathBuf::as_path).map({
                |include_path| {
                    if include_path.is_relative() && maybe_current_directory.as_ref().is_some() {
                        maybe_current_directory.as_ref().unwrap().join(include_path)
                    } else {
                        include_path.to_path_buf()
                    }
                }
            }))
            .chain(
                (file_to_import == "std-widgets.slint"
                    || referencing_file.map_or(false, |x| x.starts_with("builtin:/")))
                .then(|| format!("builtin:/{}", self.style).into()),
            )
            .find_map(|include_dir| {
                let candidate = include_dir.join(file_to_import);
                crate::fileaccess::load_file(&candidate)
                    .map(|virtual_file| (virtual_file.canon_path, virtual_file.builtin_contents))
            })
    }

    fn collect_dependencies<'a: 'b, 'b>(
        state: &'a RefCell<BorrowedTypeLoader<'a>>,
        doc: &'b syntax_nodes::Document,
    ) -> impl Iterator<Item = ImportedTypes> + 'a {
        doc.ImportSpecifier()
            .map(|import| {
                let maybe_import_uri = import.child_token(SyntaxKind::StringLiteral);
                (maybe_import_uri, ImportKind::ImportList(import))
            })
            .chain(
                // process `export * from "foo"`
                doc.ExportsList().flat_map(|exports| exports.ExportModule()).map(|reexport| {
                    let maybe_import_uri = reexport.child_token(SyntaxKind::StringLiteral);
                    (maybe_import_uri, ImportKind::ModuleReexport(reexport))
                }),
            )
            .filter_map(|(maybe_import_uri, type_specifier)| {
                let import_uri = match maybe_import_uri {
                    Some(import_uri) => import_uri,
                    None => {
                        debug_assert!(state.borrow().diag.has_error());
                        return None;
                    }
                };
                let path_to_import = import_uri.text().to_string();
                let path_to_import = path_to_import.trim_matches('\"').to_string();

                if path_to_import.is_empty() {
                    state
                        .borrow_mut()
                        .diag
                        .push_error("Unexpected empty import url".to_owned(), &import_uri);
                    return None;
                }

                Some(ImportedTypes {
                    import_uri_token: import_uri,
                    import_kind: type_specifier,
                    file: path_to_import,
                })
            })
    }

    /// Return a document if it was already loaded
    pub fn get_document<'b>(&'b self, path: &Path) -> Option<&'b object_tree::Document> {
        dunce::canonicalize(path).map_or_else(
            |_| self.all_documents.docs.get(path),
            |path| self.all_documents.docs.get(&path),
        )
    }

    /// Return an iterator over all the loaded file path
    pub fn all_files(&self) -> impl Iterator<Item = &PathBuf> {
        self.all_documents.docs.keys()
    }

    /// Returns an iterator over all the loaded documents
    pub fn all_documents(&self) -> impl Iterator<Item = &object_tree::Document> + '_ {
        self.all_documents.docs.values()
    }
}

/// return the base directory from which imports are loaded
///
/// For a .slint file, this is the parent directory.
/// For a .rs file, this is relative to the CARGO_MANIFEST_DIR
///
/// Note: this function is only called for .rs path as part of the LSP or viewer.
/// Because from a proc_macro, we don't actually know the path of the current file, and this
/// is why we must be relative to CARGO_MANIFEST_DIR.
pub fn base_directory(referencing_file: &Path) -> Option<PathBuf> {
    if referencing_file.extension().map_or(false, |e| e == "rs") {
        // For .rs file, this is a rust macro, and rust macro locates the file relative to the CARGO_MANIFEST_DIR which is the directory that has a Cargo.toml file.
        let mut candidate = referencing_file;
        loop {
            candidate =
                if let Some(c) = candidate.parent() { c } else { break referencing_file.parent() };
            if candidate.join("Cargo.toml").exists() {
                break Some(candidate);
            }
        }
    } else {
        referencing_file.parent()
    }
    .map(|p| p.to_path_buf())
}

#[test]
fn test_dependency_loading() {
    let test_source_path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "typeloader"].iter().collect();

    let mut incdir = test_source_path.clone();
    incdir.push("incpath");

    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.include_paths = vec![incdir];
    compiler_config.style = Some("fluent".into());

    let mut main_test_path = test_source_path;
    main_test_path.push("dependency_test_main.slint");

    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse_file(main_test_path, &mut test_diags).unwrap();

    let doc_node: syntax_nodes::Document = doc_node.into();

    let global_registry = TypeRegister::builtin();

    let registry = Rc::new(RefCell::new(TypeRegister::new(&global_registry)));

    let mut build_diagnostics = BuildDiagnostics::default();

    let mut loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);

    spin_on::spin_on(loader.load_dependencies_recursively(
        &doc_node,
        &mut build_diagnostics,
        &registry,
    ));

    assert!(!test_diags.has_error());
    assert!(!build_diagnostics.has_error());
}

#[test]
fn test_dependency_loading_from_rust() {
    let test_source_path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "typeloader"].iter().collect();

    let mut incdir = test_source_path.clone();
    incdir.push("incpath");

    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.include_paths = vec![incdir];
    compiler_config.style = Some("fluent".into());

    let mut main_test_path = test_source_path;
    main_test_path.push("some_rust_file.rs");

    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse_file(main_test_path, &mut test_diags).unwrap();

    let doc_node: syntax_nodes::Document = doc_node.into();

    let global_registry = TypeRegister::builtin();

    let registry = Rc::new(RefCell::new(TypeRegister::new(&global_registry)));

    let mut build_diagnostics = BuildDiagnostics::default();

    let mut loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);

    spin_on::spin_on(loader.load_dependencies_recursively(
        &doc_node,
        &mut build_diagnostics,
        &registry,
    ));

    assert!(!test_diags.has_error());
    assert!(!build_diagnostics.has_error());
}

#[test]
fn test_load_from_callback_ok() {
    let ok = Rc::new(core::cell::Cell::new(false));
    let ok_ = ok.clone();

    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    compiler_config.open_import_fallback = Some(Rc::new(move |path| {
        let ok_ = ok_.clone();
        Box::pin(async move {
            assert_eq!(path.replace('\\', "/"), "../FooBar.slint");
            assert!(!ok_.get());
            ok_.set(true);
            Some(Ok("export XX := Rectangle {} ".to_owned()))
        })
    }));

    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();
    let doc_node = crate::parser::parse(
        r#"
/* ... */
import { XX } from "../Ab/.././FooBar.slint";
X := XX {}
"#
        .into(),
        Some(std::path::Path::new("HELLO")),
        &mut test_diags,
    );

    let doc_node: syntax_nodes::Document = doc_node.into();
    let global_registry = TypeRegister::builtin();
    let registry = Rc::new(RefCell::new(TypeRegister::new(&global_registry)));
    let mut build_diagnostics = BuildDiagnostics::default();
    let mut loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);
    spin_on::spin_on(loader.load_dependencies_recursively(
        &doc_node,
        &mut build_diagnostics,
        &registry,
    ));
    assert!(ok.get());
    assert!(!test_diags.has_error());
    assert!(!build_diagnostics.has_error());
}

#[test]
fn test_load_error_twice() {
    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    let mut test_diags = crate::diagnostics::BuildDiagnostics::default();

    let doc_node = crate::parser::parse(
        r#"
/* ... */
import { XX } from "error.slint";
component Foo { XX {} }
"#
        .into(),
        Some(std::path::Path::new("HELLO")),
        &mut test_diags,
    );

    let doc_node: syntax_nodes::Document = doc_node.into();
    let global_registry = TypeRegister::builtin();
    let registry = Rc::new(RefCell::new(TypeRegister::new(&global_registry)));
    let mut build_diagnostics = BuildDiagnostics::default();
    let mut loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);
    spin_on::spin_on(loader.load_dependencies_recursively(
        &doc_node,
        &mut build_diagnostics,
        &registry,
    ));
    assert!(!test_diags.has_error());
    assert!(build_diagnostics.has_error());
    let diags = build_diagnostics.to_string_vec();
    assert_eq!(
        diags,
        &["HELLO:3: Cannot find requested import \"error.slint\" in the include search path"]
    );
    // Try loading another time with the same registery
    let mut build_diagnostics = BuildDiagnostics::default();
    spin_on::spin_on(loader.load_dependencies_recursively(
        &doc_node,
        &mut build_diagnostics,
        &registry,
    ));
    assert!(build_diagnostics.has_error());
    let diags = build_diagnostics.to_string_vec();
    assert_eq!(
        diags,
        &["HELLO:3: Cannot find requested import \"error.slint\" in the include search path"]
    );
}

#[test]
fn test_manual_import() {
    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.style = Some("fluent".into());
    let global_registry = TypeRegister::builtin();
    let mut build_diagnostics = BuildDiagnostics::default();
    let mut loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);

    let maybe_button_type = spin_on::spin_on(loader.import_component(
        "std-widgets.slint",
        "Button",
        &mut build_diagnostics,
    ));

    assert!(!build_diagnostics.has_error());
    assert!(maybe_button_type.is_some());
}

#[test]
fn test_builtin_style() {
    let test_source_path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "typeloader"].iter().collect();

    let incdir = test_source_path.join("custom_style");

    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.include_paths = vec![incdir];
    compiler_config.style = Some("fluent".into());

    let global_registry = TypeRegister::builtin();
    let mut build_diagnostics = BuildDiagnostics::default();
    let _loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);

    assert!(!build_diagnostics.has_error());
}

#[test]
fn test_user_style() {
    let test_source_path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "typeloader"].iter().collect();

    let incdir = test_source_path.join("custom_style");

    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.include_paths = vec![incdir];
    compiler_config.style = Some("TestStyle".into());

    let global_registry = TypeRegister::builtin();
    let mut build_diagnostics = BuildDiagnostics::default();
    let _loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);

    assert!(!build_diagnostics.has_error());
}

#[test]
fn test_unknown_style() {
    let test_source_path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "typeloader"].iter().collect();

    let incdir = test_source_path.join("custom_style");

    let mut compiler_config =
        CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
    compiler_config.include_paths = vec![incdir];
    compiler_config.style = Some("FooBar".into());

    let global_registry = TypeRegister::builtin();
    let mut build_diagnostics = BuildDiagnostics::default();
    let _loader = TypeLoader::new(global_registry, compiler_config, &mut build_diagnostics);

    assert!(build_diagnostics.has_error());
    let diags = build_diagnostics.to_string_vec();
    assert_eq!(diags.len(), 1);
    assert!(diags[0].starts_with("Style FooBar in not known. Use one of the builtin styles ["));
}
