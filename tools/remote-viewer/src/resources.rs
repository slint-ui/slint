use std::collections::HashSet;

use i_slint_compiler::typeloader::TypeLoader;
use lsp_types::Url;

fn extract_resources(dependencies: &HashSet<Url>, type_loader: &TypeLoader) -> HashSet<Url> {
    let mut result: HashSet<Url> = Default::default();

    for dependency in dependencies {
        let Ok(path) = dependency.to_file_path() else {
            continue;
        };
        let Some(doc) = type_loader.get_document(&path) else {
            continue;
        };

        result.extend(
            doc.embedded_file_resources
                .borrow()
                .iter()
                .filter_map(|er| Url::from_file_path(er.path.as_deref()?).ok()),
        );
    }

    result
}
