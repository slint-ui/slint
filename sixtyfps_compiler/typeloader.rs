use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::diagnostics::{FileDiagnostics, SourceFile};
use crate::parser::{syntax_nodes::Document, SyntaxKind, SyntaxTokenWithSourceFile};
use crate::typeregister::TypeRegister;
use crate::CompilerConfiguration;

pub fn load_dependencies_recursively(
    doc: &Document,
    mut diagnostics: &mut FileDiagnostics,
    registry: &Rc<RefCell<TypeRegister>>,
    compiler_config: &CompilerConfiguration,
) {
    let dependencies = collect_dependencies(&doc, &mut diagnostics, compiler_config);
    for (dependency_path, imported_types) in dependencies {
        load_dependency(dependency_path, imported_types, &registry, diagnostics, compiler_config);
    }
}

fn load_dependency(
    path: SourceFile,
    imported_types: ImportedTypes,
    registry_to_populate: &Rc<RefCell<TypeRegister>>,
    importer_diagnostics: &mut FileDiagnostics,
    compiler_config: &CompilerConfiguration,
) {
    let (dependency_doc, mut dependency_diagnostics) = match crate::parser::parse_file(&*path) {
        Ok((node, diag)) => (node.into(), diag),
        Err(err) => {
            importer_diagnostics.push_error(
                format!("Error loading {} from disk for import: {}", path.to_string_lossy(), err),
                &imported_types.import_token,
            );
            return;
        }
    };

    let dependency_registry = Rc::new(RefCell::new(TypeRegister::new(&registry_to_populate)));
    load_dependencies_recursively(
        &dependency_doc,
        &mut dependency_diagnostics,
        &dependency_registry,
        compiler_config,
    );

    let doc = crate::object_tree::Document::from_node(
        dependency_doc,
        &mut dependency_diagnostics,
        &dependency_registry,
    );

    let exports = doc.exports();

    for import_name in imported_types.type_names {
        let imported_type = exports.iter().find_map(|(export_name, component)| {
            if import_name == *export_name {
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
                        import_name,
                        dependency_diagnostics.current_path.to_string_lossy()
                    ),
                    &imported_types.import_token,
                );
                continue;
            }
        };

        registry_to_populate.borrow_mut().add_with_name(import_name, imported_type);
    }
}

struct ImportedTypes {
    pub type_names: Vec<String>,
    pub import_token: SyntaxTokenWithSourceFile,
}

type DependenciesByFile = BTreeMap<SourceFile, ImportedTypes>;

fn collect_dependencies(
    doc: &Document,
    doc_diagnostics: &mut FileDiagnostics,
    compiler_config: &CompilerConfiguration,
) -> DependenciesByFile {
    // The directory of the current file is the first in the list of include directories.
    let mut current_directory = doc.source_file.as_ref().unwrap().clone().as_ref().clone();
    current_directory.pop();

    let imports = doc
        .ImportSpecifier()
        .filter_map(|import| {
            let type_names = import
                .ImportIdentifierList()
                .ImportIdentifier()
                .map(|importident| importident.text().to_string().trim().to_string())
                .collect();

            let import_uri = import.child_token(SyntaxKind::StringLiteral).expect(
                "Internal error: missing import uri literal, this is a parsing/grammar bug",
            );
            let path_to_import = import_uri.text().to_string();
            let path_to_import = path_to_import.trim_matches('\"');
            if path_to_import.is_empty() {
                doc_diagnostics.push_error("Unexpected empty import url".to_owned(), &import_uri);
                return None;
            }

            let file_to_load = [current_directory.clone()]
                .iter()
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
                .find_map(|include_dir| {
                    let mut candidate = include_dir.clone();
                    candidate.push(&path_to_import);
                    if candidate.exists() && candidate.is_file() {
                        Some(SourceFile::new(candidate))
                    } else {
                        None
                    }
                });

            if let Some(file_to_load) = file_to_load {
                Some((file_to_load, ImportedTypes { type_names, import_token: import_uri.clone() }))
            } else {
                doc_diagnostics.push_error(
                    format!(
                        "Cannot find requested import {} in the include search path",
                        path_to_import
                    ),
                    &import_uri,
                );
                None
            }
        })
        .collect::<Vec<_>>();

    let mut dependencies = DependenciesByFile::new();

    for (file_to_load, imported_types) in imports.into_iter() {
        let type_names = imported_types.type_names;
        let import_token = imported_types.import_token;

        dependencies
            .entry(file_to_load)
            .or_insert_with(|| ImportedTypes { import_token, type_names: Vec::new() })
            .type_names
            .extend(type_names.into_iter());
    }

    dependencies
}

#[test]
fn test_dependency_loading() {
    let test_source_path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "typeloader"].iter().collect();

    let mut incdir = test_source_path.clone();
    incdir.push("incpath");

    let compiler_config = CompilerConfiguration { include_paths: &[incdir], ..Default::default() };

    let mut main_test_path = test_source_path.clone();
    main_test_path.push("dependency_test_main.60");

    let (doc_node, mut test_diags) = crate::parser::parse_file(main_test_path.clone()).unwrap();

    let doc_node: Document = doc_node.into();

    let registry = Rc::new(RefCell::new(TypeRegister::new(&TypeRegister::builtin())));

    load_dependencies_recursively(&doc_node, &mut test_diags, &registry, &compiler_config);

    assert!(!test_diags.has_error());
}
