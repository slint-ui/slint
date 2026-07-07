// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that lowers synthetic `drop-shadow-*` and `inner-shadow-*` properties to proper shadow elements.
// At the moment only shadows on `Rectangle` elements are supported.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::BindingExpression;
use crate::{expression_tree::Expression, object_tree::*};
use crate::{expression_tree::NamedReference, typeregister::TypeRegister};
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Copy, Clone)]
enum ShadowKind {
    Drop,
    Inner,
}

impl ShadowKind {
    fn prefix(self) -> &'static str {
        match self {
            ShadowKind::Drop => "drop-shadow-",
            ShadowKind::Inner => "inner-shadow-",
        }
    }

    fn property_list(self) -> &'static [(&'static str, crate::langtype::Type)] {
        match self {
            ShadowKind::Drop => crate::typeregister::RESERVED_DROP_SHADOW_PROPERTIES,
            ShadowKind::Inner => crate::typeregister::RESERVED_INNER_SHADOW_PROPERTIES,
        }
    }
}

// Creates a new BoxShadow element holding the supplied bindings, sized to follow `sibling_element`'s geometry.
fn create_box_shadow_element(
    shadow_property_bindings: HashMap<SmolStr, BindingExpression>,
    sibling_element: &ElementRc,
    kind: ShadowKind,
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

    let prefix = kind.prefix();
    let id_suffix = match kind {
        ShadowKind::Drop => "shadow",
        ShadowKind::Inner => "inner-shadow",
    };

    let mut bindings: crate::object_tree::BindingsMap = shadow_property_bindings
        .into_iter()
        .map(|(shadow_prop_name, expr)| {
            (shadow_prop_name.strip_prefix(prefix).unwrap().into(), expr.into())
        })
        .collect();

    if matches!(kind, ShadowKind::Inner) {
        bindings.insert(
            SmolStr::new_static("inset"),
            RefCell::new(Expression::BoolLiteral(true).into()),
        );
    }

    let mut element = Element {
        id: format_smolstr!("{}-{}", sibling_element.borrow().id, id_suffix),
        base_type: type_register.lookup_builtin_element("BoxShadow").unwrap(),
        enclosing_component: sibling_element.borrow().enclosing_component.clone(),
        bindings,
        ..Default::default()
    };

    for property_name in super::border_radius::BORDER_RADIUS_PROPERTIES {
        let source_property = if sibling_element.borrow().is_binding_set(property_name, true) {
            Some(SmolStr::new_static(property_name))
        } else if sibling_element.borrow().is_binding_set("border-radius", true) {
            Some(SmolStr::new_static("border-radius"))
        } else {
            None
        };

        if let Some(source_property) = source_property {
            let target_property = SmolStr::new_static(property_name);
            element.bindings.insert(
                target_property,
                RefCell::new(
                    Expression::PropertyReference(NamedReference::new(
                        sibling_element,
                        source_property,
                    ))
                    .into(),
                ),
            );
        }
    }

    Some(element)
}

fn prepend_inner_shadow_child(parent: &ElementRc, inner_elem: Element) {
    let inner_rc = ElementRc::new(inner_elem.into());
    inner_rc.borrow_mut().geometry_props = Some(GeometryProps::new(&inner_rc));
    for property_name in ["width", "height"] {
        inner_rc.borrow_mut().bindings.insert(
            property_name.into(),
            RefCell::new(
                Expression::PropertyReference(NamedReference::new(
                    parent,
                    SmolStr::new_static(property_name),
                ))
                .into(),
            ),
        );
    }
    parent.borrow_mut().children.insert(0, inner_rc);
}

// For a repeated element with a drop shadow, the shadow becomes the new root so it renders below the repeated
// element. This is only used for drop shadows; inner shadows on a repeated root are prepended as a child instead.
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
        ShadowKind::Drop,
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

fn take_shadow_property_bindings(
    element: &ElementRc,
    kind: ShadowKind,
) -> HashMap<SmolStr, BindingExpression> {
    kind.property_list()
        .iter()
        .flat_map(|(shadow_property_name, _)| {
            let shadow_property_name = SmolStr::new(shadow_property_name);
            let mut element = element.borrow_mut();
            element.bindings.remove(&shadow_property_name).map(|binding| {
                // Remove the shadow property that was also materialized as a fake property by now.
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
    for kind in [ShadowKind::Drop, ShadowKind::Inner] {
        for (shadow_prop_name, shadow_prop_binding) in
            take_shadow_property_bindings(&component.root_element, kind)
        {
            diag.push_warning(
                format!("The {shadow_prop_name} property cannot be used on the root element, the shadow will not be visible"),
                &shadow_prop_binding,
            );
        }
    }

    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        // Repeater handling: drop shadow becomes the new root (so it renders underneath); inner
        // shadow is prepended as a child of the repeater's root rectangle (so it renders above
        // the background but below the rectangle's original children).
        if elem.borrow().repeated.is_some() {
            // Take both binding sets up front, then release every Rc clone before
            // `inject_element_as_repeated_element`, which asserts the component has strong_count == 2.
            let (drop_shadow_properties, inner_shadow_properties) = {
                let component = elem.borrow().base_type.as_component().clone();
                let drop = take_shadow_property_bindings(&component.root_element, ShadowKind::Drop);
                let inner =
                    take_shadow_property_bindings(&component.root_element, ShadowKind::Inner);
                (drop, inner)
            };

            if !drop_shadow_properties.is_empty() {
                inject_shadow_element_in_repeated_element(
                    drop_shadow_properties,
                    elem,
                    type_register,
                    diag,
                );
                // After injection the original rectangle is a child of the new shadow root.
                // Prepend the inner BoxShadow as a child of that rectangle.
                if !inner_shadow_properties.is_empty() {
                    let rect_child = elem
                        .borrow()
                        .base_type
                        .as_component()
                        .root_element
                        .borrow()
                        .children
                        .first()
                        .cloned();
                    if let Some(rect_child) = rect_child
                        && let Some(inner_elem) = create_box_shadow_element(
                            inner_shadow_properties,
                            &rect_child,
                            ShadowKind::Inner,
                            type_register,
                            diag,
                        )
                    {
                        prepend_inner_shadow_child(&rect_child, inner_elem);
                    }
                }
            } else if !inner_shadow_properties.is_empty() {
                // No drop shadow: prepend inner shadow as a child of the repeater root rectangle.
                let root = elem.borrow().base_type.as_component().root_element.clone();
                if let Some(inner_elem) = create_box_shadow_element(
                    inner_shadow_properties,
                    &root,
                    ShadowKind::Inner,
                    type_register,
                    diag,
                ) {
                    prepend_inner_shadow_child(&root, inner_elem);
                }
            }
        }

        let old_children = {
            let mut elem = elem.borrow_mut();
            let new_children = Vec::with_capacity(elem.children.len());
            std::mem::replace(&mut elem.children, new_children)
        };

        // For each child: drop shadow renders BEFORE (underneath); inner shadow is prepended as
        // the child's first child (above background, below the original child content).
        for child in old_children {
            let drop_shadow_properties = take_shadow_property_bindings(&child, ShadowKind::Drop);
            let inner_shadow_properties = take_shadow_property_bindings(&child, ShadowKind::Inner);

            if !drop_shadow_properties.is_empty()
                && let Some(mut shadow_elem) = create_box_shadow_element(
                    drop_shadow_properties,
                    &child,
                    ShadowKind::Drop,
                    type_register,
                    diag,
                )
            {
                shadow_elem.geometry_props.clone_from(&child.borrow().geometry_props);
                // Sort the shadow with the same z as its element; the stable sort keeps it beneath
                shadow_elem.z_order = child.borrow().z_order.clone();
                elem.borrow_mut().children.push(ElementRc::new(shadow_elem.into()));
            }

            if !inner_shadow_properties.is_empty()
                && let Some(shadow_elem) = create_box_shadow_element(
                    inner_shadow_properties,
                    &child,
                    ShadowKind::Inner,
                    type_register,
                    diag,
                )
            {
                prepend_inner_shadow_child(&child, shadow_elem);
            }

            elem.borrow_mut().children.push(child);
        }
    });
}
