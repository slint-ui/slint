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

pub(crate) fn lower_property_to_element(
    component: &Rc<Component>,
    property_name: &str,
    element_name: &str,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
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
                            property_name,
                            element_name,
                            type_register,
                        ),
                    )
                }
            } else if has_property_binding(&child) {
                let new_child =
                    create_property_element(&child, property_name, element_name, type_register);
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
    property_name: &str,
    element_name: &str,
    type_register: &TypeRegister,
) -> ElementRc {
    let element = Element {
        id: format!("{}-{}", child.borrow().id, property_name),
        base_type: type_register.lookup_element(element_name).unwrap(),
        enclosing_component: child.borrow().enclosing_component.clone(),
        bindings: std::iter::once((
            property_name.to_owned(),
            BindingExpression::new_two_way(NamedReference::new(child, property_name)).into(),
        ))
        .collect(),
        ..Default::default()
    };
    Rc::new(RefCell::new(element))
}
