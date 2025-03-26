// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hooks properties for live inspection.

use crate::{expression_tree, object_tree, typeloader};

pub fn inject_debug_hooks(
    doc: &mut object_tree::Document,
    type_loader: &mut typeloader::TypeLoader,
) {
    if !type_loader.compiler_config.debug_info {
        return;
    }

    let mut counter = 1_u64;

    doc.visit_all_used_components(|component| {
        object_tree::recurse_elem_including_sub_components(component, &(), &mut |e, &()| {
            process_element(e, counter, &type_loader.compiler_config);
            counter += 1;
        })
    });
}

fn property_id(counter: u64, name: &smol_str::SmolStr) -> smol_str::SmolStr {
    smol_str::format_smolstr!("?{counter}-{name}")
}

fn process_element(
    element: &object_tree::ElementRc,
    counter: u64,
    config: &crate::CompilerConfiguration,
) {
    let mut elem = element.borrow_mut();
    assert_eq!(elem.debug.len(), 1); // We did not merge Elements yet and we have debug info!

    if config.debug_hooks {
        elem.bindings.iter().for_each(|(name, be)| {
            let expr = std::mem::take(&mut be.borrow_mut().expression);
            be.borrow_mut().expression = {
                let stripped = super::ignore_debug_hooks(&expr);
                if matches!(stripped, expression_tree::Expression::Invalid) {
                    stripped.clone()
                } else {
                    expression_tree::Expression::DebugHook {
                        expression: Box::new(expr),
                        id: property_id(counter, name),
                    }
                }
            };
        });
    }
    elem.debug.first_mut().expect("There was one element a moment ago").element_id = counter;
}
