// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that lowers synthetic `drop-shadow-*` properties to proper shadow elements
// At the moment only shadows on `Rectangle` elements are supported, i.e. the drop shadow
// of a rectangle is a box shadow.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::BindingExpression;
use crate::{expression_tree::Expression, object_tree::*};
use crate::{expression_tree::NamedReference, typeregister::TypeRegister};
use smol_str::{format_smolstr, SmolStr};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Creates a new element for the drop shadow properties that'll be a sibling to the specified
// sibling element.
fn create_box_shadow_element(
    shadow_property_bindings: HashMap<SmolStr, BindingExpression>,
    sibling_element: &ElementRc,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) -> Option<Element> {
    if matches!(sibling_element.borrow().builtin_type(), Some(b) if b.name != "Rectangle") {
        for (shadow_prop_name, shadow_prop_binding) in shadow_property_bindings {
            diag.push_error(
                format!("The {shadow_prop_name} property is only supported on Rectangle elements right now"),
                &shadow_prop_binding,
            );
        }
        return None;
    }

    let mut element = Element {
        id: format_smolstr!("{}-shadow", sibling_element.borrow().id),
        base_type: type_register.lookup_builtin_element("BoxShadow").unwrap(),
        enclosing_component: sibling_element.borrow().enclosing_component.clone(),
        bindings: shadow_property_bindings
            .into_iter()
            .map(|(shadow_prop_name, expr)| {
                (shadow_prop_name.strip_prefix("drop-shadow-").unwrap().into(), expr.into())
            })
            .collect(),
        ..Default::default()
    };

    // FIXME: remove the border-radius manual mapping.
    let border_radius = SmolStr::new_static("border-radius");
    if sibling_element.borrow().bindings.contains_key(&border_radius) {
        element.bindings.insert(
            border_radius.clone(),
            RefCell::new(
                Expression::PropertyReference(NamedReference::new(sibling_element, border_radius))
                    .into(),
            ),
        );
    }

    Some(element)
}

// For a repeated element, this function creates a new element for the drop shadow properties that
// will act as the new root element in the repeater. The former root will become a child.
fn inject_shadow_element_in_repeated_element(
    shadow_property_bindings: HashMap<SmolStr, BindingExpression>,
    repeated_element: &ElementRc,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let element_with_shadow_property =
        &repeated_element.borrow().base_type.as_component().root_element.clone();

    let shadow_element = match create_box_shadow_element(
        shadow_property_bindings,
        element_with_shadow_property,
        type_register,
        diag,
    ) {
        Some(element) => element,
        None => return,
    };

    crate::object_tree::inject_element_as_repeated_element(
        repeated_element,
        Element::make_rc(shadow_element),
    );
}

fn take_shadow_property_bindings(element: &ElementRc) -> HashMap<SmolStr, BindingExpression> {
    crate::typeregister::RESERVED_DROP_SHADOW_PROPERTIES
        .iter()
        .flat_map(|(shadow_property_name, _)| {
            let shadow_property_name = SmolStr::new(shadow_property_name);
            let mut element = element.borrow_mut();
            element.bindings.remove(&shadow_property_name).map(|binding| {
                // Remove the drop-shadow property that was also materialized as a fake property by now.
                element.property_declarations.remove(&shadow_property_name);
                (shadow_property_name, binding.into_inner())
            })
        })
        .collect()
}

pub fn lower_shadow_properties(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    for (shadow_prop_name, shadow_prop_binding) in
        take_shadow_property_bindings(&component.root_element)
    {
        diag.push_warning(
            format!("The {shadow_prop_name} property cannot be used on the root element, the shadow will not be visible"),
            &shadow_prop_binding,
        );
    }

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        // When encountering a repeater where the repeated element has a `drop-shadow` property, we create a new
        // dedicated shadow element and make the previously repeated element a child of that. This ensures rendering
        // underneath while maintaining the hierarchy for the repeater.
        // The geometry properties are aliased using two-way bindings (which may be eliminated in a later pass).

        if elem.borrow().repeated.is_some() {
            let component = elem.borrow().base_type.as_component().clone(); // CHECK if clone can be removed if we change borrow

            let drop_shadow_properties = take_shadow_property_bindings(&component.root_element);
            if !drop_shadow_properties.is_empty() {
                drop(component);
                inject_shadow_element_in_repeated_element(
                    drop_shadow_properties,
                    elem,
                    type_register,
                    diag,
                );
            }
        }

        let old_children = {
            let mut elem = elem.borrow_mut();
            let new_children = Vec::with_capacity(elem.children.len());
            std::mem::replace(&mut elem.children, new_children)
        };

        // When encountering a `drop-shadow` property in a supported element, we create a new dedicated
        // shadow element and insert it *before* the element that had the `drop-shadow` property, to ensure
        // that it is rendered underneath.
        for child in old_children {
            let drop_shadow_properties = take_shadow_property_bindings(&child);
            if !drop_shadow_properties.is_empty() {
                let mut shadow_elem = match create_box_shadow_element(
                    drop_shadow_properties,
                    &child,
                    type_register,
                    diag,
                ) {
                    Some(element) => element,
                    None => {
                        elem.borrow_mut().children.push(child);
                        continue;
                    }
                };

                shadow_elem.geometry_props.clone_from(&child.borrow().geometry_props);
                elem.borrow_mut().children.push(ElementRc::new(shadow_elem.into()));
            }
            elem.borrow_mut().children.push(child);
        }
    });
}
