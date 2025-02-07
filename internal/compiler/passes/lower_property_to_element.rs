// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that lowers synthetic properties such as `opacity` and `layer` properties to their corresponding elements.
//! For example `f := Foo { opacity: <some float>; }` is mapped to `Opacity { opacity <=> f.opacity; f := Foo { ... } }`

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::{self, Component, Element, ElementRc};
use crate::typeregister::TypeRegister;
use smol_str::{format_smolstr, SmolStr, ToSmolStr};
use std::rc::Rc;

/// If any element in `component` declares a binding to `property_name`, then a new
/// element of type `element_name` is created, injected as a parent to the element and bindings
/// to property_name and all properties in  extra_properties are mapped.
/// Default value for the property extra_properties is queried with the `default_value_for_extra_properties`
pub(crate) fn lower_property_to_element(
    component: &Rc<Component>,
    property_name: &'static str,
    extra_properties: impl Iterator<Item = &'static str> + Clone,
    default_value_for_extra_properties: Option<&dyn Fn(&ElementRc, &str) -> Expression>,
    element_name: &SmolStr,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    if let Some(b) = component.root_element.borrow().bindings.get(property_name) {
        diag.push_warning(
            format!(
                "The {property_name} property cannot be used on the root element, it will not be applied"
            ),
            &*b.borrow(),
        );
    }

    object_tree::recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        if elem.borrow().base_type.to_smolstr() == *element_name {
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
                        .is_some_and(|a| a.is_set || a.is_linked))
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
                            extra_properties.clone(),
                            default_value_for_extra_properties,
                            element_name,
                            type_register,
                        ),
                    )
                }
            } else if has_property_binding(&child) {
                let new_child = create_property_element(
                    &child,
                    property_name,
                    extra_properties.clone(),
                    default_value_for_extra_properties,
                    element_name,
                    type_register,
                );
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
    property_name: &'static str,
    extra_properties: impl Iterator<Item = &'static str>,
    default_value_for_extra_properties: Option<&dyn Fn(&ElementRc, &str) -> Expression>,
    element_name: &SmolStr,
    type_register: &TypeRegister,
) -> ElementRc {
    let bindings = core::iter::once(property_name)
        .chain(extra_properties)
        .map(|property_name| {
            let mut bind =
                BindingExpression::new_two_way(NamedReference::new(child, property_name.into()));
            if let Some(default_value_for_extra_properties) = default_value_for_extra_properties {
                if !child.borrow().bindings.contains_key(property_name) {
                    bind.expression = default_value_for_extra_properties(child, property_name)
                }
            }
            (property_name.into(), bind.into())
        })
        .collect();

    let element = Element {
        id: format_smolstr!("{}-{}", child.borrow().id, property_name),
        base_type: type_register.lookup_element(element_name).unwrap(),
        enclosing_component: child.borrow().enclosing_component.clone(),
        bindings,
        ..Default::default()
    };
    element.make_rc()
}
