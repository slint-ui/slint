// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::expression_tree::Expression;
use crate::object_tree::*;

pub fn embed_include_strings(doc: &Document) {
    let global_embedded_resources = &doc.embedded_file_resources;

    doc.visit_all_used_components(|component| {
        visit_all_expressions(component, |e, _| {
            if let Expression::IncludeString(path) = e {
                let path = path.clone();
                let mut resources = global_embedded_resources.borrow_mut();
                if !resources.contains_key(&path) {
                    let id = resources.len();
                    resources.insert(
                        path,
                        crate::embedded_resources::EmbeddedResources {
                            id,
                            kind: crate::embedded_resources::EmbeddedResourcesKind::RawData,
                        },
                    );
                }
            }
        });
    });
}
