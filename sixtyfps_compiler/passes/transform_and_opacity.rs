// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

//! Pass that lowers synthetic `opacity`, `visibility`, or `rotate` properties to their Element.
//! TODO: the rotation is not yet implemented

use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::{self, Component, Element, ElementRc};
use crate::typeregister::TypeRegister;

pub(crate) fn handle_transform_and_opacity(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    if let Some(b) = component.root_element.borrow().bindings.get("opacity") {
        diag.push_warning(
            "The opacity property cannot be used on the root element, it will not be applied"
                .to_string(),
            &*b.borrow(),
        );
    }

    object_tree::recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        if elem.borrow().base_type.to_string() == "Opacity" {
            return;
        }

        let old_children = {
            let mut elem = elem.borrow_mut();
            let new_children = Vec::with_capacity(elem.children.len());
            std::mem::replace(&mut elem.children, new_children)
        };

        let has_opacity_binding = |e: &ElementRc| {
            e.borrow().base_type.lookup_property("opacity").property_type != Type::Invalid
                && (e.borrow().bindings.contains_key("opacity")
                    || e.borrow()
                        .property_analysis
                        .borrow()
                        .get("opacity")
                        .map_or(false, |a| a.is_set))
        };

        for mut child in old_children {
            if child.borrow().repeated.is_some() {
                let root_elem = child.borrow().base_type.as_component().root_element.clone();
                if has_opacity_binding(&root_elem) {
                    object_tree::inject_element_as_repeated_element(
                        &child,
                        create_opacity_element(&root_elem, type_register),
                    )
                }
            } else if has_opacity_binding(&child) {
                let new_child = create_opacity_element(&child, type_register);
                new_child.borrow_mut().children.push(child);
                child = new_child;
            }

            elem.borrow_mut().children.push(child);
        }
    });
}

fn create_opacity_element(child: &ElementRc, type_register: &TypeRegister) -> ElementRc {
    let element = Element {
        id: format!("{}-opacity", child.borrow().id),
        base_type: type_register.lookup_element("Opacity").unwrap(),
        enclosing_component: child.borrow().enclosing_component.clone(),
        bindings: std::iter::once((
            "opacity".to_owned(),
            BindingExpression::new_two_way(NamedReference::new(child, "opacity")).into(),
        ))
        .collect(),
        ..Default::default()
    };
    Rc::new(RefCell::new(element))
}
