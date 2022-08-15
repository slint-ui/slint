// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Pass that lowers synthetic properties such as `opacity` and `layer` properties to their corresponding elements.
//! For example `f := Foo { opacity: <some float>; }` is mapped to `Opacity { opacity <=> f.opacity; f := Foo { ... } }`

use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::{self, Component, Element, ElementRc};
use crate::typeregister::TypeRegister;

// If any element in `component` declares a binding to the first property in `properties`, then a new
// element of type `element_name` is created, injected as a parent to the element and bindings to all
// remaining properties in `properties` are mapped. This way for example ["rotation-angle", "rotation-origin-x", "rotation-origin-y"]
// creates a `Rotate` element when `rotation-angle` is used and any optional `rotation-origin-*` bindings are redirected to the
// `Rotate` element.
pub(crate) fn lower_property_to_element(
    component: &Rc<Component>,
    properties: &[(&str, Type)],
    element_name: &str,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let property_name = properties[0].0;

    if let Some(b) = component.root_element.borrow().bindings.get(property_name) {
        diag.push_warning(
            format!(
                "The {} property cannot be used on the root element, it will not be applied",
                property_name
            ),
            &*b.borrow(),
        );
    }

    object_tree::recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        if elem.borrow().base_type.to_string() == element_name {
            return;
        }

        let old_children = {
            let mut elem = elem.borrow_mut();
            let new_children = Vec::with_capacity(elem.children.len());
            std::mem::replace(&mut elem.children, new_children)
        };

        let has_property_binding = |e: &ElementRc| {
            e.borrow().base_type.lookup_property(property_name).property_type != Type::Invalid
                && (e.borrow().bindings.contains_key(property_name)
                    || e.borrow()
                        .property_analysis
                        .borrow()
                        .get(property_name)
                        .map_or(false, |a| a.is_set))
        };

        for mut child in old_children {
            if child.borrow().repeated.is_some() {
                let root_elem = child.borrow().base_type.as_component().root_element.clone();
                if has_property_binding(&root_elem) {
                    object_tree::inject_element_as_repeated_element(
                        &child,
                        create_property_element(
                            &root_elem,
                            properties,
                            element_name,
                            type_register,
                        ),
                    )
                }
            } else if has_property_binding(&child) {
                let new_child =
                    create_property_element(&child, properties, element_name, type_register);
                crate::object_tree::adjust_geometry_for_injected_parent(&new_child, &child);
                new_child.borrow_mut().children.push(child);
                child = new_child;
            }

            elem.borrow_mut().children.push(child);
        }
    });
}

fn create_property_element(
    child: &ElementRc,
    properties: &[(&str, Type)],
    element_name: &str,
    type_register: &TypeRegister,
) -> ElementRc {
    let bindings = properties
        .iter()
        .filter_map(|(property_name, _)| {
            if child.borrow().bindings.contains_key(*property_name) {
                Some((
                    property_name.to_string(),
                    BindingExpression::new_two_way(NamedReference::new(child, property_name))
                        .into(),
                ))
            } else {
                None
            }
        })
        .collect();

    let element = Element {
        id: format!("{}-{}", child.borrow().id, properties[0].0),
        base_type: type_register.lookup_element(element_name).unwrap(),
        enclosing_component: child.borrow().enclosing_component.clone(),
        bindings,
        ..Default::default()
    };
    Rc::new(RefCell::new(element))
}
