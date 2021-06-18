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

use std::collections::HashSet;
use std::rc::Rc;

use crate::langtype::{NativeClass, Type};
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

            let analysis = elem.property_analysis.borrow();
            let native_properties_used: HashSet<_> = elem
                .bindings
                .keys()
                .chain(analysis.iter().filter(|(_, v)| v.is_read || v.is_set).map(|(k, _)| k))
                .filter(|k| {
                    !elem.property_declarations.contains_key(*k)
                        && base_type.as_ref().properties.contains_key(*k)
                })
                .collect();

            select_minimal_class_based_on_property_usage(
                elem.base_type.as_builtin().native_class.clone(),
                native_properties_used.into_iter(),
            )
        };

        elem.borrow_mut().base_type = Type::Native(new_native_class);
    })
}

fn lookup_property_distance(mut class: Rc<NativeClass>, name: &str) -> (usize, Rc<NativeClass>) {
    eprintln!("lookup {} {}", class.class_name, name);
    let mut distance = 0;
    loop {
        if class.properties.contains_key(name) {
            return (distance, class);
        }
        distance += 1;
        class = class.parent.as_ref().unwrap().clone();
    }
}

fn select_minimal_class_based_on_property_usage<'a>(
    class: Rc<NativeClass>,
    properties_used: impl Iterator<Item = &'a String>,
) -> Rc<NativeClass> {
    let mut minimal_class = class.clone();
    while let Some(class) = minimal_class.parent.clone() {
        minimal_class = class;
    }
    let (_min_distance, minimal_class) = properties_used.fold(
        (std::usize::MAX, minimal_class),
        |(current_distance, current_class), prop_name| {
            let (prop_distance, prop_class) = lookup_property_distance(class.clone(), &prop_name);

            if prop_distance < current_distance {
                (prop_distance, prop_class)
            } else {
                (current_distance, current_class)
            }
        },
    );
    minimal_class
}

#[test]
fn select_minimal_class() {
    let tr = crate::typeregister::TypeRegister::builtin();
    let tr = tr.borrow();
    let rect = tr.lookup("Rectangle");
    let rect = rect.as_builtin();
    assert_eq!(
        rect.native_class
            .clone()
            .select_minimal_class_based_on_property_usage(
                ["x".to_owned(), "width".to_owned()].iter()
            )
            .class_name,
        "Rectangle",
    );
    assert_eq!(
        rect.native_class
            .clone()
            .select_minimal_class_based_on_property_usage([].iter())
            .class_name,
        "Rectangle",
    );
    assert_eq!(
        rect.native_class
            .clone()
            .select_minimal_class_based_on_property_usage(["border_width".to_owned()].iter())
            .class_name,
        "BorderRectangle",
    );
    assert_eq!(
        rect.native_class
            .clone()
            .select_minimal_class_based_on_property_usage(
                ["border_width".to_owned(), "x".to_owned()].iter()
            )
            .class_name,
        "BorderRectangle",
    );
}
