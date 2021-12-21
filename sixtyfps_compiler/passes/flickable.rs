// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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
use crate::langtype::{NativeClass, Type};
use crate::object_tree::{Component, Element, ElementRc};
use crate::typeregister::TypeRegister;

pub fn is_flickable_element(element: &ElementRc) -> bool {
    matches!(&element.borrow().base_type, Type::Builtin(n) if n.name == "Flickable")
}

pub fn handle_flickable(root_component: &Rc<Component>, tr: &TypeRegister) {
    let mut native_rect = tr.lookup("Rectangle").as_builtin().native_class.clone();
    while let Some(p) = native_rect.parent.clone() {
        native_rect = p;
    }

    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &(),
        &mut |elem: &ElementRc, _| {
            if !is_flickable_element(elem) {
                return;
            }

            fixup_geometry(elem);
            create_viewport_element(elem, &native_rect);
        },
    )
}

fn create_viewport_element(flickable_elem: &ElementRc, native_rect: &Rc<NativeClass>) {
    let mut flickable = flickable_elem.borrow_mut();
    let flickable = &mut *flickable;
    let viewport = Rc::new(RefCell::new(Element {
        id: format!("{}-viewport", flickable.id),
        base_type: Type::Native(native_rect.clone()),
        children: std::mem::take(&mut flickable.children),
        enclosing_component: flickable.enclosing_component.clone(),
        is_flickable_viewport: true,
        ..Element::default()
    }));
    for (prop, info) in &flickable.base_type.as_builtin().properties {
        if let Some(vp_prop) = prop.strip_prefix("viewport-") {
            let nr = NamedReference::new(&viewport, vp_prop);
            flickable.property_declarations.insert(prop.to_owned(), info.ty.clone().into());
            match flickable.bindings.entry(prop.to_owned()) {
                std::collections::btree_map::Entry::Occupied(entry) => {
                    entry.into_mut().get_mut().two_way_bindings.push(nr);
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(BindingExpression::new_two_way(nr).into());
                }
            }
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
    flickable.children.push(viewport);
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
fn is_layout(base_type: &Type) -> bool {
    if let Type::Builtin(be) = base_type {
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
