// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass fills the root component library_imports
use crate::object_tree::Document;

pub fn collect_libraries(doc: &mut Document) {
    doc.imports.iter().for_each(|import| {
        if let Some(library_info) = &import.library_info {
            library_info.exports.iter().for_each(|export_name| {
                doc.library_exports.insert(export_name.to_string(), library_info.clone());
            });
        }
    });
}
