// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hooks properties for live inspection.

use crate::{expression_tree, object_tree, typeloader};

pub fn inject_debug_hooks(doc: &object_tree::Document, type_loader: &typeloader::TypeLoader) {
    let Some(random_state) = &type_loader.compiler_config.debug_hooks else {
        return;
    };

    doc.visit_all_used_components(|component| {
        object_tree::recurse_elem_including_sub_components(component, &(), &mut |e, &()| {
            process_element(e, random_state);
        })
    });
}

fn property_id(element_id: u64, name: &smol_str::SmolStr) -> smol_str::SmolStr {
    smol_str::format_smolstr!("?{element_id}-{name}")
}

fn calculate_element_hash(
    elem: &object_tree::Element,
    random_state: &std::hash::RandomState,
) -> u64 {
    let node = &elem.debug.first().expect("There was one element a moment ago").node;

    let elem_path = node.source_file.path();
    let elem_offset = node
        .child_token(crate::parser::SyntaxKind::LBrace)
        .expect("All elements have a opening Brace")
        .text_range()
        .start();

    use std::hash::{BuildHasher, Hasher};
    let mut hasher = random_state.build_hasher();
    hasher.write(elem_path.as_os_str().as_encoded_bytes());
    hasher.write_u32(elem_offset.into());
    hasher.finish()
}

fn process_element(element: &object_tree::ElementRc, random_state: &std::hash::RandomState) {
    let mut elem = element.borrow_mut();
    // We did not merge Elements yet and we have debug info!
    assert_eq!(elem.debug.len(), 1);

    // Ignore nodes previously set up
    if elem.debug.first().expect("There was one element a moment ago").element_hash != 0 {
        return;
    }

    let element_hash = calculate_element_hash(&elem, random_state);

    elem.bindings.iter().for_each(|(name, be)| {
        let expr = std::mem::take(&mut be.borrow_mut().expression);
        be.borrow_mut().expression = {
            let stripped = super::ignore_debug_hooks(&expr);
            if matches!(stripped, expression_tree::Expression::Invalid) {
                stripped.clone()
            } else {
                expression_tree::Expression::DebugHook {
                    expression: Box::new(expr),
                    id: property_id(element_hash, name),
                }
            }
        };
    });

    elem.debug.first_mut().expect("There was one element a moment ago").element_hash = element_hash;
}
