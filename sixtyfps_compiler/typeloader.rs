/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, FileDiagnostics, SourceFile};
use crate::object_tree::Document;
use crate::parser::{syntax_nodes, SyntaxKind, SyntaxTokenWithSourceFile};
use crate::typeregister::TypeRegister;
use crate::CompilerConfiguration;

/// Storage for a cache of all loaded documents
#[derive(Default)]
pub struct LoadedDocuments {
    /// maps from the canonical file name to the object_tree::Document
    docs: HashMap<PathBuf, Document>,
    currently_loading: HashSet<PathBuf>,
}

struct OpenFile {
    path: PathBuf,
    source_code_future:
        core::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<String>>>>,
}

trait DirectoryAccess<'a> {
    fn try_open(&self, file_path: &str) -> Option<OpenFile>;
}

impl<'a> DirectoryAccess<'a> for PathBuf {
    fn try_open(&self, file_path: &str) -> Option<OpenFile> {
        let candidate = self.join(file_path);

        std::fs::File::open(&candidate).ok().map(|mut f| OpenFile {
            path: candidate,
            source_code_future: Box::pin(async move {
                let mut buf = String::new();
                f.read_to_string(&mut buf).map(|_| buf)
            }),
        })
    }
}

pub struct VirtualFile<'a> {
    pub path: &'a str,
    pub contents: &'a str,
}

pub type VirtualDirectory<'a> = [&'a VirtualFile<'a>];

impl<'a> DirectoryAccess<'a> for &'a VirtualDirectory<'a> {
    fn try_open(&self, file_path: &str) -> Option<OpenFile> {
        self.iter().find_map(|virtual_file| {
            if virtual_file.path != file_path {
                return None;
            }
            Some(OpenFile {
                path: file_path.into(),
                source_code_future: Box::pin({
                    let source = virtual_file.contents.to_owned();
                    async move { Ok(source) }
                }),
            })
        })
    }
}

struct ImportedTypes {
    pub type_names: Vec<ImportedName>,
    pub import_token: SyntaxTokenWithSourceFile,
    pub file: OpenFile,
}

type DependenciesByFile = BTreeMap<PathBuf, ImportedTypes>;

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
    ) -> impl Iterator<Item = ImportedName> {
        import.ImportIdentifierList().ImportIdentifier().map(|importident| {
            let external_name = importident.ExternalName().text().to_string().trim().to_string();

            let internal_name = match importident.InternalName() {
                Some(name_ident) => name_ident.text().to_string().trim().to_string(),
                None => external_name.clone(),
            };

            ImportedName { internal_name, external_name }
        })
    }
}

pub struct TypeLoader<'a> {
    pub global_type_registry: &'a Rc<RefCell<TypeRegister>>,
    pub compiler_config: &'a CompilerConfiguration,
    pub builtin_library: Option<&'a VirtualDirectory<'a>>,
    all_documents: LoadedDocuments,
}

impl<'a> TypeLoader<'a> {
    pub fn new(
        global_type_registry: &'a Rc<RefCell<TypeRegister>>,
        compiler_config: &'a CompilerConfiguration,
        builtin_library: Option<&'a VirtualDirectory<'a>>,
    ) -> Self {
        Self {
            global_type_registry,
            compiler_config,
            builtin_library,
            all_documents: Default::default(),
        }
    }

    pub async fn load_dependencies_recursively(
        &mut self,
        doc: &syntax_nodes::Document,
        mut diagnostics: &mut FileDiagnostics,
        build_diagnostics: &mut BuildDiagnostics,
        registry_to_populate: &Rc<RefCell<TypeRegister>>,
    ) {
        let dependencies = self.collect_dependencies(&doc, &mut diagnostics).await;
        for import in dependencies.into_iter() {
            self.load_dependency(import, registry_to_populate, diagnostics, build_diagnostics)
                .await;
        }
    }

    pub async fn import_type(
        &mut self,
        file_to_import: &str,
        type_name: &str,
        diagnostics: &mut FileDiagnostics,
        build_diagnostics: &mut BuildDiagnostics,
    ) -> Option<crate::langtype::Type> {
        let file = match self.import_file(None, file_to_import) {
            Some(file) => file,
            None => return None,
        };

        let doc_path = match self
            .ensure_document_loaded(
                &file.path,
                file.source_code_future,
                None,
                diagnostics,
                build_diagnostics,
            )
            .await
        {
            Some(doc_path) => doc_path,
            None => return None,
        };

        let doc = self.all_documents.docs.get(&doc_path).unwrap();

        doc.exports().iter().find_map(|(export_name, ty)| {
            if type_name == *export_name {
                Some(ty.clone())
            } else {
                None
            }
        })
    }

    fn ensure_document_loaded<'b>(
        &'b mut self,
        path: &'b PathBuf,
        source_code_future: core::pin::Pin<
            Box<dyn std::future::Future<Output = std::io::Result<String>>>,
        >,
        import_token: Option<SyntaxTokenWithSourceFile>,
        importer_diagnostics: &'b mut FileDiagnostics,
        mut build_diagnostics: &'b mut BuildDiagnostics,
    ) -> core::pin::Pin<Box<dyn std::future::Future<Output = Option<PathBuf>> + 'b>> {
        Box::pin(async move {
            let path_canon = path.canonicalize().unwrap_or(path.clone());

            if self.all_documents.docs.get(path_canon.as_path()).is_some() {
                return Some(path_canon);
            }

            if !self.all_documents.currently_loading.insert(path_canon.clone()) {
                importer_diagnostics
                    .push_error(format!("Recursive import of {}", path.display()), &import_token);
                return None;
            }

            let source_code = match source_code_future.await {
                Ok(source) => source,
                Err(err) => {
                    importer_diagnostics.push_error(
                        format!("Error reading requested import {}: {}", path.display(), err),
                        &import_token,
                    );
                    return None;
                }
            };

            let (dependency_doc, mut dependency_diagnostics) =
                crate::parser::parse(source_code, Some(&path));

            dependency_diagnostics.current_path = SourceFile::new(path.clone());

            if dependency_diagnostics.has_error() {
                build_diagnostics.add(dependency_diagnostics);
                return None;
            }

            let dependency_doc: syntax_nodes::Document = dependency_doc.into();

            let dependency_registry =
                Rc::new(RefCell::new(TypeRegister::new(&self.global_type_registry)));
            self.load_dependencies_recursively(
                &dependency_doc,
                &mut dependency_diagnostics,
                &mut build_diagnostics,
                &dependency_registry,
            )
            .await;

            let doc = crate::object_tree::Document::from_node(
                dependency_doc,
                &mut dependency_diagnostics,
                &dependency_registry,
            );
            crate::passes::resolving::resolve_expressions(&doc, build_diagnostics);

            // Add diagnostics regardless whether they're empty or not. This is used by the syntax_tests to
            // also verify that imported files have no errors.
            build_diagnostics.add(dependency_diagnostics);

            let _ok = self.all_documents.currently_loading.remove(path_canon.as_path());
            assert!(_ok);

            match self.all_documents.docs.entry(path_canon.clone()) {
                std::collections::hash_map::Entry::Occupied(_) => unreachable!(),
                std::collections::hash_map::Entry::Vacant(e) => e.insert(doc),
            };

            Some(path_canon)
        })
    }

    fn load_dependency<'b>(
        &'b mut self,
        import: ImportedTypes,
        registry_to_populate: &'b Rc<RefCell<TypeRegister>>,
        importer_diagnostics: &'b mut FileDiagnostics,
        build_diagnostics: &'b mut BuildDiagnostics,
    ) -> core::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'b>> {
        Box::pin(async move {
            let doc_path = match self
                .ensure_document_loaded(
                    &import.file.path,
                    import.file.source_code_future,
                    Some(import.import_token.clone()),
                    importer_diagnostics,
                    build_diagnostics,
                )
                .await
            {
                Some(path) => path,
                None => return,
            };

            let doc = self.all_documents.docs.get(&doc_path).unwrap();
            let exports = doc.exports();

            for import_name in import.type_names {
                let imported_type = exports.iter().find_map(|(export_name, ty)| {
                    if import_name.external_name == *export_name {
                        Some(ty.clone())
                    } else {
                        None
                    }
                });

                let imported_type = match imported_type {
                    Some(ty) => ty,
                    None => {
                        importer_diagnostics.push_error(
                            format!(
                                "No exported type called {} found in {}",
                                import_name.external_name,
                                import.file.path.display()
                            ),
                            &import.import_token,
                        );
                        continue;
                    }
                };

                registry_to_populate
                    .borrow_mut()
                    .insert_type_with_name(imported_type, import_name.internal_name);
            }
        })
    }

    fn import_file(
        &self,
        referencing_file: Option<&std::path::Path>,
        file_to_import: &str,
    ) -> Option<OpenFile> {
        // The directory of the current file is the first in the list of include directories.
        let maybe_current_directory =
            referencing_file.and_then(|path| path.parent()).map(|p| p.to_path_buf());
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
            .find_map(|include_dir| include_dir.try_open(file_to_import))
            .or_else(|| self.builtin_library.and_then(|lib| lib.try_open(file_to_import)))
            .or_else(|| {
                self.compiler_config
                    .resolve_import_fallback
                    .as_ref()
                    .map_or_else(
                        || Some(file_to_import.to_owned()),
                        |resolve_import_callback| {
                            resolve_import_callback(file_to_import.to_owned())
                        },
                    )
                    .and_then(|resolved_absolute_path| {
                        self.compiler_config
                            .open_import_fallback
                            .as_ref()
                            .map(|cb| cb(resolved_absolute_path.clone()))
                            .and_then(|future| {
                                Some(OpenFile {
                                    path: resolved_absolute_path.into(),
                                    source_code_future: future,
                                })
                            })
                    })
            })
    }

    async fn collect_dependencies(
        &mut self,
        doc: &syntax_nodes::Document,
        doc_diagnostics: &mut FileDiagnostics,
    ) -> impl Iterator<Item = ImportedTypes> {
        let referencing_file = doc.source_file.as_ref().unwrap().clone();

        let mut dependencies = DependenciesByFile::new();

        for import in doc.ImportSpecifier() {
            let import_uri = import.child_token(SyntaxKind::StringLiteral).expect(
                "Internal error: missing import uri literal, this is a parsing/grammar bug",
            );
            let path_to_import = import_uri.text().to_string();
            let path_to_import = path_to_import.trim_matches('\"').to_string();
            if path_to_import.is_empty() {
                doc_diagnostics.push_error("Unexpected empty import url".to_owned(), &import_uri);
                continue;
            }

            let dependency_entry = if let Some(dependency_file) =
                self.import_file(Some(&referencing_file), &path_to_import)
            {
                match dependencies.entry(dependency_file.path.clone()) {
                    std::collections::btree_map::Entry::Vacant(vacant_entry) => vacant_entry
                        .insert(ImportedTypes {
                            type_names: vec![],
                            import_token: import_uri,
                            file: dependency_file,
                        }),
                    std::collections::btree_map::Entry::Occupied(existing_entry) => {
                        existing_entry.into_mut()
                    }
                }
            } else {
                doc_diagnostics.push_error(
                    format!(
                        "Cannot find requested import {} in the include search path",
                        path_to_import
                    ),
                    &import_uri,
                );
                continue;
            };
            dependency_entry.type_names.extend(ImportedName::extract_imported_names(&import));
        }

        dependencies.into_iter().map(|(_, value)| value)
    }
}

#[test]
fn test_dependency_loading() {
    let test_source_path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "typeloader"].iter().collect();

    let mut incdir = test_source_path.clone();
    incdir.push("incpath");

    let compiler_config =
        CompilerConfiguration { include_paths: vec![incdir], ..Default::default() };

    let mut main_test_path = test_source_path.clone();
    main_test_path.push("dependency_test_main.60");

    let (doc_node, mut test_diags) = crate::parser::parse_file(main_test_path.clone()).unwrap();

    let doc_node: syntax_nodes::Document = doc_node.into();

    let global_registry = TypeRegister::builtin();

    let registry = Rc::new(RefCell::new(TypeRegister::new(&global_registry)));

    let mut build_diagnostics = BuildDiagnostics::default();

    let mut loader = TypeLoader::new(&global_registry, &compiler_config, None);

    spin_on::spin_on(loader.load_dependencies_recursively(
        &doc_node,
        &mut test_diags,
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

    let compiler_config = CompilerConfiguration {
        open_import_fallback: Some(Box::new(move |path| {
            let ok_ = ok_.clone();
            Box::pin(async move {
                assert_eq!(path, "../FooBar.60");
                assert_eq!(ok_.get(), false);
                ok_.set(true);
                Ok("export XX := Rectangle {} ".to_owned())
            })
        })),
        ..Default::default()
    };

    let (doc_node, mut test_diags) = crate::parser::parse(
        r#"
/* ... */
import { XX } from "../FooBar.60";
X := XX {}
"#
        .into(),
        Some(&std::path::Path::new("HELLO")),
    );

    let doc_node: syntax_nodes::Document = doc_node.into();
    let global_registry = TypeRegister::builtin();
    let registry = Rc::new(RefCell::new(TypeRegister::new(&global_registry)));
    let mut build_diagnostics = BuildDiagnostics::default();
    let mut loader = TypeLoader::new(&global_registry, &compiler_config, None);
    spin_on::spin_on(loader.load_dependencies_recursively(
        &doc_node,
        &mut test_diags,
        &mut build_diagnostics,
        &registry,
    ));
    assert_eq!(ok.get(), true);
    assert!(!test_diags.has_error());
    assert!(!build_diagnostics.has_error());
}

#[test]
fn test_manual_import() {
    let compiler_config = CompilerConfiguration { ..Default::default() };
    let mut test_diags = FileDiagnostics::default();
    let global_registry = TypeRegister::builtin();
    let mut build_diagnostics = BuildDiagnostics::default();
    let builtin_lib = crate::library::widget_library().iter().find(|x| x.0 == "ugly").map(|x| x.1);
    let mut loader = TypeLoader::new(&global_registry, &compiler_config, builtin_lib);

    let maybe_button_type = spin_on::spin_on(loader.import_type(
        "sixtyfps_widgets.60",
        "Button",
        &mut test_diags,
        &mut build_diagnostics,
    ));

    assert!(!test_diags.has_error());
    assert!(!build_diagnostics.has_error());
    assert!(maybe_button_type.is_some());
}
