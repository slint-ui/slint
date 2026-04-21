// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass extends the init code with font registration

use crate::embedded_resources::EmbeddedResourcesIdx;
use crate::{
    expression_tree::{BuiltinFunction, Expression, Unit},
    object_tree::*,
};
use smol_str::SmolStr;
use std::collections::{BTreeSet, HashMap};

pub fn collect_custom_fonts<'a>(
    doc: &Document,
    all_docs: impl Iterator<Item = &'a Document> + 'a,
    embed_fonts: bool,
) {
    let mut all_fonts = BTreeSet::new();

    for doc in all_docs {
        all_fonts.extend(doc.custom_fonts.iter().map(|(path, _)| path))
    }

    let registration_function = if embed_fonts {
        BuiltinFunction::RegisterCustomFontByMemory
    } else {
        BuiltinFunction::RegisterCustomFontByPath
    };

    let mut path_to_id = HashMap::<SmolStr, EmbeddedResourcesIdx>::new();
    let mut prepare_font_registration_argument: Box<dyn FnMut(&SmolStr) -> Expression> =
        if embed_fonts {
            Box::new(|font_path| {
                let resource_id = *path_to_id.entry(font_path.clone()).or_insert_with(|| {
                    doc.embedded_file_resources.borrow_mut().push_and_get_key(
                        crate::embedded_resources::EmbeddedResources {
                            path: Some(font_path.clone()),
                            kind: crate::embedded_resources::EmbeddedResourcesKind::FileData,
                        },
                    )
                });
                Expression::NumberLiteral(resource_id.0 as _, Unit::None)
            })
        } else {
            Box::new(|font_path| Expression::StringLiteral(font_path.clone()))
        };

    for c in doc.exported_roots() {
        c.init_code.borrow_mut().font_registration_code.extend(all_fonts.iter().map(|font_path| {
            Expression::FunctionCall {
                function: registration_function.clone().into(),
                arguments: vec![prepare_font_registration_argument(font_path)],
                source_location: None,
            }
        }));
    }
}
