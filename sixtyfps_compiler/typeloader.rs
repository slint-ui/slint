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
    pub source_code_future:
        core::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<String>>>>,
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
    pub all_documents: &'a mut LoadedDocuments,
    pub build_diagnostics: &'a mut BuildDiagnostics,
}

impl<'a> TypeLoader<'a> {
    pub async fn load_dependencies_recursively(
        &mut self,
        doc: &syntax_nodes::Document,
        mut diagnostics: &mut FileDiagnostics,
        registry_to_populate: &Rc<RefCell<TypeRegister>>,
    ) {
        let dependencies = self.collect_dependencies(&doc, &mut diagnostics).await;
        for (dependency_path, imported_types) in dependencies {
            self.load_dependency(
                dependency_path,
                imported_types,
                registry_to_populate,
                diagnostics,
            )
            .await;
        }
    }

    fn load_dependency<'b>(
        &'b mut self,
        path: PathBuf,
        imported_types: ImportedTypes,
        registry_to_populate: &'b Rc<RefCell<TypeRegister>>,
        importer_diagnostics: &'b mut FileDiagnostics,
    ) -> core::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'b>> {
        Box::pin(async move {
            let path_canon = path.canonicalize().unwrap_or(path.clone());

            let doc = if let Some(doc) = self.all_documents.docs.get(path_canon.as_path()) {
                doc
            } else {
                if !self.all_documents.currently_loading.insert(path_canon.clone()) {
                    importer_diagnostics.push_error(
                        format!("Recursive import of {}", path.display()),
                        &imported_types.import_token,
                    );
                    return;
                }

                let source_code = match imported_types.source_code_future.await {
                    Ok(source) => source,
                    Err(err) => {
                        importer_diagnostics.push_error(
                            format!("Error reading requested import {}: {}", path.display(), err),
                            &imported_types.import_token,
                        );
                        return;
                    }
                };

                let (dependency_doc, mut dependency_diagnostics) =
                    crate::parser::parse(source_code, Some(&path));

                dependency_diagnostics.current_path = SourceFile::new(path.clone());

                if dependency_diagnostics.has_error() {
                    self.build_diagnostics.add(dependency_diagnostics);
                    return;
                }

                let dependency_doc: syntax_nodes::Document = dependency_doc.into();

                let dependency_registry =
                    Rc::new(RefCell::new(TypeRegister::new(&self.global_type_registry)));
                self.load_dependencies_recursively(
                    &dependency_doc,
                    &mut dependency_diagnostics,
                    &dependency_registry,
                )
                .await;

                let doc = crate::object_tree::Document::from_node(
                    dependency_doc,
                    &mut dependency_diagnostics,
                    &dependency_registry,
                );
                crate::passes::resolving::resolve_expressions(&doc, self.build_diagnostics);

                // Add diagnostics regardless whether they're empty or not. This is used by the syntax_tests to
                // also verify that imported files have no errors.
                self.build_diagnostics.add(dependency_diagnostics);

                let _ok = self.all_documents.currently_loading.remove(path_canon.as_path());
                assert!(_ok);

                match self.all_documents.docs.entry(path_canon) {
                    std::collections::hash_map::Entry::Occupied(_) => unreachable!(),
                    std::collections::hash_map::Entry::Vacant(e) => e.insert(doc),
                }
            };

            let exports = doc.exports();

            for import_name in imported_types.type_names {
                let imported_type = exports.iter().find_map(|(export_name, component)| {
                    if import_name.external_name == *export_name {
                        Some(component.clone())
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
                                path.display()
                            ),
                            &imported_types.import_token,
                        );
                        continue;
                    }
                };

                registry_to_populate
                    .borrow_mut()
                    .add_with_name(import_name.internal_name, imported_type);
            }
        })
    }

    fn import_file(
        &self,
        referencing_file: &std::path::Path,
        file_to_import: &str,
    ) -> Option<OpenFile> {
        // The directory of the current file is the first in the list of include directories.
        let maybe_current_directory = referencing_file.parent();
        core::iter::once(maybe_current_directory.map(|dir| dir.to_path_buf()).as_ref())
            .filter_map(|dir| dir)
            .chain(self.compiler_config.include_paths.iter())
            .map(|include_path| {
                if include_path.is_relative() && maybe_current_directory.is_some() {
                    let mut abs_path = maybe_current_directory.unwrap().to_path_buf();
                    abs_path.push(include_path);
                    abs_path
                } else {
                    include_path.clone()
                }
            })
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
    ) -> DependenciesByFile {
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
                self.import_file(&referencing_file, &path_to_import)
            {
                match dependencies.entry(dependency_file.path.clone()) {
                    std::collections::btree_map::Entry::Vacant(vacant_entry) => vacant_entry
                        .insert(ImportedTypes {
                            type_names: vec![],
                            import_token: import_uri,
                            source_code_future: dependency_file.source_code_future,
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

        dependencies
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
    let mut docs = Default::default();

    let mut loader = TypeLoader {
        global_type_registry: &global_registry,
        compiler_config: &compiler_config,
        builtin_library: None,
        all_documents: &mut docs,
        build_diagnostics: &mut build_diagnostics,
    };

    spin_on::spin_on(loader.load_dependencies_recursively(&doc_node, &mut test_diags, &registry));

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
    let mut docs = Default::default();
    let mut loader = TypeLoader {
        global_type_registry: &global_registry,
        compiler_config: &compiler_config,
        builtin_library: None,
        all_documents: &mut docs,
        build_diagnostics: &mut build_diagnostics,
    };
    spin_on::spin_on(loader.load_dependencies_recursively(&doc_node, &mut test_diags, &registry));
    assert_eq!(ok.get(), true);
    assert!(!test_diags.has_error());
    assert!(!build_diagnostics.has_error());
}
