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

struct OpenFile<'a> {
    path: PathBuf,
    file: Box<dyn Read + 'a>,
}

trait DirectoryAccess<'a> {
    fn try_open(&self, file_path: &str) -> Option<OpenFile<'a>>;
}

impl<'a> DirectoryAccess<'a> for PathBuf {
    fn try_open(&self, file_path: &str) -> Option<OpenFile<'a>> {
        let candidate = self.join(file_path);

        std::fs::File::open(&candidate)
            .ok()
            .map(|f| OpenFile { path: candidate, file: Box::new(f) as Box<dyn Read> })
    }
}

pub struct VirtualFile<'a> {
    pub path: &'a str,
    pub contents: &'a str,
}

pub type VirtualDirectory<'a> = [&'a VirtualFile<'a>];

impl<'a> DirectoryAccess<'a> for &'a VirtualDirectory<'a> {
    fn try_open(&self, file_path: &str) -> Option<OpenFile<'a>> {
        self.iter().find_map(|virtual_file| {
            if virtual_file.path != file_path {
                return None;
            }
            Some(OpenFile {
                path: file_path.into(),
                file: Box::new(std::io::Cursor::new(virtual_file.contents.as_bytes())),
            })
        })
    }
}

pub async fn load_dependencies_recursively<'a>(
    doc: &syntax_nodes::Document,
    mut diagnostics: &mut FileDiagnostics,
    registry_to_populate: &Rc<RefCell<TypeRegister>>,
    global_type_registry: &Rc<RefCell<TypeRegister>>,
    compiler_config: &CompilerConfiguration,
    builtin_library: Option<&'a VirtualDirectory<'a>>,
    all_documents: &mut LoadedDocuments,
    build_diagnostics: &mut BuildDiagnostics,
) {
    let dependencies =
        collect_dependencies(&doc, &mut diagnostics, compiler_config, builtin_library).await;
    for (dependency_path, imported_types) in dependencies {
        load_dependency(
            dependency_path,
            imported_types,
            registry_to_populate,
            global_type_registry,
            diagnostics,
            compiler_config,
            builtin_library,
            all_documents,
            build_diagnostics,
        )
        .await;
    }
}

fn load_dependency<'a>(
    path: PathBuf,
    imported_types: ImportedTypes,
    registry_to_populate: &'a Rc<RefCell<TypeRegister>>,
    global_type_registry: &'a Rc<RefCell<TypeRegister>>,
    importer_diagnostics: &'a mut FileDiagnostics,
    compiler_config: &'a CompilerConfiguration,
    builtin_library: Option<&'a VirtualDirectory<'a>>,
    all_documents: &'a mut LoadedDocuments,
    build_diagnostics: &'a mut BuildDiagnostics,
) -> core::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        let path_canon = path.canonicalize().unwrap_or(path.clone());

        let doc = if let Some(doc) = all_documents.docs.get(path_canon.as_path()) {
            doc
        } else {
            if !all_documents.currently_loading.insert(path_canon.clone()) {
                importer_diagnostics.push_error(
                    format!("Recursive import of {}", path.display()),
                    &imported_types.import_token,
                );
                return;
            }

            let (dependency_doc, mut dependency_diagnostics) =
                crate::parser::parse(imported_types.source_code, Some(&path));

            dependency_diagnostics.current_path = SourceFile::new(path.clone());

            let dependency_doc: syntax_nodes::Document = dependency_doc.into();

            let dependency_registry =
                Rc::new(RefCell::new(TypeRegister::new(&global_type_registry)));
            load_dependencies_recursively(
                &dependency_doc,
                &mut dependency_diagnostics,
                &dependency_registry,
                &global_type_registry,
                compiler_config,
                builtin_library,
                all_documents,
                build_diagnostics,
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

            let _ok = all_documents.currently_loading.remove(path_canon.as_path());
            assert!(_ok);

            match all_documents.docs.entry(path_canon) {
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

struct ImportedTypes {
    pub type_names: Vec<ImportedName>,
    pub import_token: SyntaxTokenWithSourceFile,
    pub source_code: String,
}

type DependenciesByFile = BTreeMap<PathBuf, ImportedTypes>;

async fn collect_dependencies<'a>(
    doc: &'a syntax_nodes::Document,
    doc_diagnostics: &mut FileDiagnostics,
    compiler_config: &'a CompilerConfiguration,
    builtin_library: Option<&'a VirtualDirectory<'a>>,
) -> DependenciesByFile {
    // The directory of the current file is the first in the list of include directories.
    let mut current_directory = doc.source_file.as_ref().unwrap().clone().as_ref().clone();
    current_directory.pop();

    let open_file_from_include_paths = |file_path: &str| {
        core::iter::once(&current_directory)
            .chain(compiler_config.include_paths.iter())
            .map(|include_path| {
                if include_path.is_relative() {
                    let mut abs_path = current_directory.clone();
                    abs_path.push(include_path);
                    abs_path
                } else {
                    include_path.clone()
                }
            })
            .find_map(|include_dir| include_dir.try_open(file_path))
            .or_else(|| builtin_library.and_then(|lib| lib.try_open(file_path)))
    };

    let mut dependencies = DependenciesByFile::new();

    for import in doc.ImportSpecifier() {
        let import_uri = import
            .child_token(SyntaxKind::StringLiteral)
            .expect("Internal error: missing import uri literal, this is a parsing/grammar bug");
        let path_to_import = import_uri.text().to_string();
        let path_to_import = path_to_import.trim_matches('\"').to_string();
        if path_to_import.is_empty() {
            doc_diagnostics.push_error("Unexpected empty import url".to_owned(), &import_uri);
            continue;
        }

        let dependency_entry = if let Some(mut dependency_file) =
            open_file_from_include_paths(&path_to_import)
        {
            match dependencies.entry(dependency_file.path.clone()) {
                std::collections::btree_map::Entry::Vacant(vacant_entry) => {
                    let mut source_code = String::new();
                    if dependency_file.file.read_to_string(&mut source_code).is_err() {
                        doc_diagnostics.push_error(
                            format!(
                                "Error reading requested import {}",
                                dependency_file.path.display()
                            ),
                            &import_uri,
                        );
                        continue;
                    }
                    vacant_entry.insert(ImportedTypes {
                        type_names: vec![],
                        import_token: import_uri,
                        source_code,
                    })
                }
                std::collections::btree_map::Entry::Occupied(existing_entry) => {
                    existing_entry.into_mut()
                }
            }
        } else {
            let absolute_path = if let Some(path) = &compiler_config
                .resolve_import_fallback
                .as_ref()
                .map_or(Some(path_to_import.clone()), |cb| cb(path_to_import.clone()))
            {
                path.clone()
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
            match dependencies.entry(absolute_path.clone().into()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let source = if let Some(cb) = &compiler_config.open_import_fallback {
                        cb(absolute_path).await
                    } else {
                        Err(String::new())
                    };
                    let source_code = match source {
                        Ok(x) => x,
                        Err(err) => {
                            doc_diagnostics.push_error(
                                    if err.is_empty() {
                                        format!("Cannot find requested import {} in the include search path", path_to_import)
                                    } else {
                                        err
                                    },
                                    &import_uri,
                                );
                            continue;
                        }
                    };
                    entry.insert(ImportedTypes {
                        type_names: vec![],
                        import_token: import_uri,
                        source_code,
                    })
                }
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            }
        };
        dependency_entry.type_names.extend(ImportedName::extract_imported_names(&import));
    }

    dependencies
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

    spin_on::spin_on(load_dependencies_recursively(
        &doc_node,
        &mut test_diags,
        &registry,
        &global_registry,
        &compiler_config,
        None,
        &mut docs,
        &mut build_diagnostics,
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
    let mut docs = Default::default();
    spin_on::spin_on(load_dependencies_recursively(
        &doc_node,
        &mut test_diags,
        &registry,
        &global_registry,
        &compiler_config,
        None,
        &mut docs,
        &mut build_diagnostics,
    ));
    assert_eq!(ok.get(), true);
    assert!(!test_diags.has_error());
    assert!(!build_diagnostics.has_error());
}
