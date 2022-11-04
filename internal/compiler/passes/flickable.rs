// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Flickable pass
//!
//! The Flickable element is special in the sense that it has a viewport
//! which is not exposed. This passes create the viewport and fixes all property access
//!
//! It will also initialize proper geometry
//! This pass must be called before the materialize_fake_properties as it going to be generate
//! binding reference to fake properties

use std::cell::RefCell;
use std::rc::Rc;

use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::langtype::{ElementType, NativeClass};
use crate::object_tree::{Component, Element, ElementRc};
use crate::typeregister::TypeRegister;

pub fn is_flickable_element(element: &ElementRc) -> bool {
    matches!(&element.borrow().base_type, ElementType::Builtin(n) if n.name == "Flickable")
}

pub fn handle_flickable(root_component: &Rc<Component>, tr: &TypeRegister) {
    let mut native_empty = tr.empty_type().as_builtin().native_class.clone();
    while let Some(p) = native_empty.parent.clone() {
        native_empty = p;
    }

    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &(),
        &mut |elem: &ElementRc, _| {
            if !is_flickable_element(elem) {
                return;
            }

            fixup_geometry(elem);
            create_viewport_element(elem, &native_empty);
        },
    )
}

fn create_viewport_element(flickable: &ElementRc, native_empty: &Rc<NativeClass>) {
    let children = std::mem::take(&mut flickable.borrow_mut().children);
    let viewport = Rc::new(RefCell::new(Element {
        id: format!("{}-viewport", flickable.borrow().id),
        base_type: ElementType::Native(native_empty.clone()),
        children,
        enclosing_component: flickable.borrow().enclosing_component.clone(),
        is_flickable_viewport: true,
        ..Element::default()
    }));
    let element_type = flickable.borrow().base_type.clone();
    for (prop, info) in &element_type.as_builtin().properties {
        if let Some(vp_prop) = prop.strip_prefix("viewport-") {
            // materialize the viewport properties
            flickable
                .borrow_mut()
                .property_declarations
                .insert(prop.to_owned(), info.ty.clone().into());
            // bind the viewport's property to the flickable property, such as:  `width <=> parent.viewport-width`
            viewport.borrow_mut().bindings.insert(
                vp_prop.to_owned(),
                BindingExpression::new_two_way(NamedReference::new(flickable, prop)).into(),
            );
        }
    }
    viewport
        .borrow()
        .property_analysis
        .borrow_mut()
        .entry("y".into())
        .or_default()
        .is_set_externally = true;
    viewport
        .borrow()
        .property_analysis
        .borrow_mut()
        .entry("x".into())
        .or_default()
        .is_set_externally = true;
    flickable.borrow_mut().children.push(viewport);
}

fn fixup_geometry(flickable_elem: &ElementRc) {
    let forward_minmax_of = |prop: &str, op: char| {
        set_binding_if_not_explicit(flickable_elem, prop, || {
            flickable_elem
                .borrow()
                .children
                .iter()
                .filter(|x| is_layout(&x.borrow().base_type))
                // FIXME: we should ideally add runtime code to merge layout info of all elements that are repeated (#407)
                .filter(|x| x.borrow().repeated.is_none())
                .map(|x| Expression::PropertyReference(NamedReference::new(x, prop)))
                .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, op))
        })
    };

    if !flickable_elem.borrow().bindings.contains_key("height") {
        forward_minmax_of("max-height", '<');
        forward_minmax_of("preferred-height", '<');
    }
    if !flickable_elem.borrow().bindings.contains_key("width") {
        forward_minmax_of("max-width", '<');
        forward_minmax_of("preferred-width", '<');
    }
    set_binding_if_not_explicit(flickable_elem, "viewport-width", || {
        Some(
            flickable_elem
                .borrow()
                .children
                .iter()
                .filter(|x| is_layout(&x.borrow().base_type))
                // FIXME: (#407)
                .filter(|x| x.borrow().repeated.is_none())
                .map(|x| Expression::PropertyReference(NamedReference::new(x, "min-width")))
                .fold(
                    Expression::PropertyReference(NamedReference::new(flickable_elem, "width")),
                    |lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, '>'),
                ),
        )
    });
    set_binding_if_not_explicit(flickable_elem, "viewport-height", || {
        Some(
            flickable_elem
                .borrow()
                .children
                .iter()
                .filter(|x| is_layout(&x.borrow().base_type))
                // FIXME: (#407)
                .filter(|x| x.borrow().repeated.is_none())
                .map(|x| Expression::PropertyReference(NamedReference::new(x, "min-height")))
                .fold(
                    Expression::PropertyReference(NamedReference::new(flickable_elem, "height")),
                    |lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, '>'),
                ),
        )
    });
}

/// Return true if this type is a layout that has constraints
fn is_layout(base_type: &ElementType) -> bool {
    if let ElementType::Builtin(be) = base_type {
        match be.name.as_str() {
            "GridLayout" | "HorizontalLayout" | "VerticalLayout" => true,
            "PathLayout" => false,
            _ => false,
        }
    } else {
        false
    }
}

/// Set the property binding on the given element to the given expression (computed lazily).
/// The parameter to the lazily calculation is the element's children
fn set_binding_if_not_explicit(
    elem: &ElementRc,
    property: &str,
    expression: impl FnOnce() -> Option<Expression>,
) {
    // we can't use `set_binding_if_not_set` directly because `expression()` may borrow `elem`
    if elem.borrow().bindings.get(property).map_or(true, |b| !b.borrow().has_binding()) {
        if let Some(e) = expression() {
            elem.borrow_mut().set_binding_if_not_set(property.into(), || e);
        }
    }
}
