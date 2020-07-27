//! This pass creates properties that are used but are otherwise not real

use crate::{object_tree::*, typeregister::Type};
use std::rc::Rc;

pub fn materialize_fake_properties(component: &Rc<Component>) {
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        let elem = elem.borrow_mut();
        let base_type = elem.base_type.clone();
        let (bindings, mut property_declarations) = std::cell::RefMut::map_split(elem, |elem| {
            (&mut elem.bindings, &mut elem.property_declarations)
        });
        for (prop, _) in bindings.iter() {
            if property_declarations.contains_key(prop) {
                continue;
            }
            let has_declared_property = match &base_type {
                crate::typeregister::Type::Component(c) => {
                    has_declared_property(&c.root_element.borrow(), prop)
                }
                crate::typeregister::Type::Builtin(b) => b.properties.contains_key(prop),
                crate::typeregister::Type::Native(n) => n.lookup_property(prop).is_some(),
                _ => false,
            };

            if !has_declared_property {
                let ty = crate::typeregister::reserved_property(prop);
                if ty != Type::Invalid {
                    property_declarations.insert(
                        prop.to_owned(),
                        PropertyDeclaration {
                            property_type: ty,
                            type_node: None,
                            expose_in_public_api: false,
                        },
                    );
                }
            }
        }
    })
}

/// Returns true if the property is declared in this element or parent
/// (as opposed to being implicitly declared)
fn has_declared_property(elem: &Element, prop: &str) -> bool {
    if elem.property_declarations.contains_key(prop) {
        return true;
    }
    match &elem.base_type {
        crate::typeregister::Type::Component(c) => {
            has_declared_property(&c.root_element.borrow(), prop)
        }
        crate::typeregister::Type::Builtin(b) => b.properties.contains_key(prop),
        crate::typeregister::Type::Native(n) => n.lookup_property(prop).is_some(),
        _ => false,
    }
}
