/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This pass creates properties that are used but are otherwise not real

use crate::langtype::Type;
use crate::object_tree::*;
use std::collections::HashMap;
use std::rc::Rc;

pub fn materialize_fake_properties(component: &Rc<Component>) {
    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        visit_all_named_references_in_element(elem, |nr| {
            let elem = nr.element();
            let elem = elem.borrow_mut();
            let (base_type, mut property_declarations) =
                std::cell::RefMut::map_split(elem, |elem| {
                    (&mut elem.base_type, &mut elem.property_declarations)
                });
            maybe_materialize(&mut property_declarations, &base_type, nr.name());
        });
        let elem = elem.borrow_mut();
        let base_type = elem.base_type.clone();
        let (bindings, mut property_declarations) = std::cell::RefMut::map_split(elem, |elem| {
            (&mut elem.bindings, &mut elem.property_declarations)
        });
        for (prop, _) in bindings.iter() {
            maybe_materialize(&mut property_declarations, &base_type, prop);
        }
    })
}

fn maybe_materialize(
    property_declarations: &mut HashMap<String, PropertyDeclaration>,
    base_type: &Type,
    prop: &str,
) {
    if property_declarations.contains_key(prop) {
        return;
    }
    let has_declared_property = match &base_type {
        Type::Component(c) => has_declared_property(&c.root_element.borrow(), prop),
        Type::Builtin(b) => b.properties.contains_key(prop),
        Type::Native(n) => {
            n.lookup_property(prop).map_or(false, |prop_type| prop_type.is_property_type())
        }
        _ => false,
    };

    if !has_declared_property {
        let ty = crate::typeregister::reserved_property(prop);
        if ty != Type::Invalid {
            property_declarations.insert(
                prop.to_owned(),
                PropertyDeclaration { property_type: ty, ..PropertyDeclaration::default() },
            );
        }
    }
}

/// Returns true if the property is declared in this element or parent
/// (as opposed to being implicitly declared)
fn has_declared_property(elem: &Element, prop: &str) -> bool {
    if elem.property_declarations.contains_key(prop) {
        return true;
    }
    match &elem.base_type {
        Type::Component(c) => has_declared_property(&c.root_element.borrow(), prop),
        Type::Builtin(b) => b.properties.contains_key(prop),
        Type::Native(n) => n.lookup_property(prop).is_some(),
        _ => false,
    }
}
