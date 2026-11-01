// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass fills the root document's library_exports, also from nested imports
use crate::object_tree::Document;
use crate::typeloader::TypeLoader;
use std::collections::HashSet;

pub fn collect_libraries(doc: &mut Document, type_loader: &TypeLoader) {
    let mut visited = HashSet::new();
    let mut stack: Vec<_> = doc.imports.iter().collect();
    while let Some(import) = stack.pop() {
        if let Some(library_info) = &import.library_info {
            library_info.exports.iter().for_each(|export_name| {
                doc.library_exports.insert(export_name.to_string(), library_info.clone());
            });
        } else if let Some(resolved) = &import.resolved
            && visited.insert(resolved)
            && let Some(imported_doc) = type_loader.get_document(resolved)
        {
            stack.extend(imported_doc.imports.iter());
        }
    }
}
