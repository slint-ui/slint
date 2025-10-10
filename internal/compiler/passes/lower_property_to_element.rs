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

/// If any element in `component` declares a binding to any of `property_names`, then a new
/// element of type `element_name` is created, injected as a parent to the element and bindings
/// to all properties in property_names and extra_properties are mapped.
/// Default value for the property extra_properties is queried with the `default_value_for_extra_properties`
pub(crate) fn lower_property_to_element(
    component: &Rc<Component>,
    property_names: impl Iterator<Item = &'static str> + Clone,
    extra_properties: impl Iterator<Item = &'static str> + Clone,
    default_value_for_extra_properties: Option<&dyn Fn(&ElementRc, &str) -> Option<Expression>>,
    element_name: &SmolStr,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    for property_name in property_names.clone() {
        if let Some(b) = component.root_element.borrow().bindings.get(property_name) {
            diag.push_warning(
                format!(
                    "The {property_name} property cannot be used on the root element, it will not be applied"
                ),
                &*b.borrow(),
            );
        }
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
            property_names.clone().any(|property_name| {
                e.borrow().base_type.lookup_property(property_name).property_type != Type::Invalid
                    && (e.borrow().bindings.contains_key(property_name)
                        || e.borrow()
                            .property_analysis
                            .borrow()
                            .get(property_name)
                            .is_some_and(|a| a.is_set || a.is_linked))
            })
        };

        for mut child in old_children {
            if child.borrow().repeated.is_some() {
                let root_elem = child.borrow().base_type.as_component().root_element.clone();
                if has_property_binding(&root_elem) {
                    object_tree::inject_element_as_repeated_element(
                        &child,
                        create_property_element(
                            &root_elem,
                            property_names.clone().chain(extra_properties.clone()),
                            default_value_for_extra_properties,
                            element_name,
                            type_register,
                        ),
                    )
                }
            } else if has_property_binding(&child) {
                let new_child = create_property_element(
                    &child,
                    property_names.clone().chain(extra_properties.clone()),
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
    properties: impl Iterator<Item = &'static str>,
    default_value_for_extra_properties: Option<&dyn Fn(&ElementRc, &str) -> Option<Expression>>,
    element_name: &SmolStr,
    type_register: &TypeRegister,
) -> ElementRc {
    let bindings = properties
        .map(|property_name| {
            let mut bind =
                BindingExpression::new_two_way(NamedReference::new(child, property_name.into()));
            if let Some(default_value_for_extra_properties) = default_value_for_extra_properties {
                if !child.borrow().bindings.contains_key(property_name) {
                    if let Some(e) = default_value_for_extra_properties(child, property_name) {
                        bind.expression = e;
                    }
                }
            }
            (property_name.into(), bind.into())
        })
        .collect();

    let element = Element {
        id: format_smolstr!("{}-{}", child.borrow().id, element_name),
        base_type: type_register.lookup_element(element_name).unwrap(),
        enclosing_component: child.borrow().enclosing_component.clone(),
        bindings,
        ..Default::default()
    };
    element.make_rc()
}

/// Wrapper around lower_property_to_element for the Transform element
pub fn lower_transform_properties(
    component: &Rc<Component>,
    tr: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let transform_origin = crate::typeregister::transform_origin_property();

    lower_property_to_element(
        component,
        crate::typeregister::RESERVED_TRANSFORM_PROPERTIES.iter().map(|(prop_name, _)| *prop_name),
        std::iter::once(transform_origin.0),
        Some(&|e, prop| {
            let prop_div_2 = |prop: &str| Expression::BinaryExpression {
                lhs: Expression::PropertyReference(NamedReference::new(e, prop.into())).into(),
                op: '/',
                rhs: Expression::NumberLiteral(2., Default::default()).into(),
            };

            match prop {
                "transform-origin" => Some(Expression::Struct {
                    ty: transform_origin.1.clone(),
                    values: [
                        (SmolStr::new_static("x"), prop_div_2("width")),
                        (SmolStr::new_static("y"), prop_div_2("height")),
                    ]
                    .into_iter()
                    .collect(),
                }),
                "transform-scale-x" | "transform-scale-y" => {
                    if e.borrow().is_binding_set("transform-scale", true) {
                        Some(Expression::PropertyReference(NamedReference::new(
                            e,
                            SmolStr::new_static("transform-scale"),
                        )))
                    } else {
                        Some(Expression::NumberLiteral(1., Default::default()))
                    }
                }
                "transform-scale" => None,
                "transform-rotation" => Some(Expression::NumberLiteral(0., Default::default())),
                _ => unreachable!(),
            }
        }),
        &SmolStr::new_static("Transform"),
        tr,
        diag,
    );
}
