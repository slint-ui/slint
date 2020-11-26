/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! After inlining and moving declarations, all Element::base_type should be Type::BuiltinElement. This pass resolves them
//! to NativeClass and picking a variant that only contains the used properties.

use crate::langtype::Type;
use crate::object_tree::{recurse_elem_including_sub_components, Component};

pub fn resolve_native_classes(component: &Component) {
    recurse_elem_including_sub_components(&component, &(), &mut |elem, _| {
        let new_native_class = {
            let elem = elem.borrow();

            let base_type = match &elem.base_type {
                Type::Component(_) => {
                    // recurse_elem_including_sub_components will recurse into it
                    return;
                }
                Type::Builtin(b) => b,
                Type::Native(_) => {
                    // already native
                    return;
                }
                _ => panic!("This should not happen"),
            };

            let native_properties_used = elem.bindings.keys().filter(|k| {
                if elem.property_declarations.contains_key(*k) {
                    return false;
                }

                base_type.as_ref().properties.get(*k).is_some()
            });

            let current_native_class = elem.base_type.as_builtin().native_class.clone();
            current_native_class
                .select_minimal_class_based_on_property_usage(native_properties_used)
        };

        elem.borrow_mut().base_type = Type::Native(new_native_class);
    })
}
