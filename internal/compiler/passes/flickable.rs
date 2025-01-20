// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Flickable pass
//!
//! The Flickable element is special in the sense that it has a viewport
//! which is not exposed. This passes create the viewport and fixes all property access
//!
//! It will also initialize proper geometry
//! This pass must be called before the materialize_fake_properties as it going to be generate
//! binding reference to fake properties

use crate::expression_tree::{BindingExpression, Expression, MinMaxOp, NamedReference};
use crate::langtype::{ElementType, NativeClass, Type};
use crate::object_tree::{Component, Element, ElementRc};
use crate::typeregister::TypeRegister;
use core::cell::RefCell;
use smol_str::{format_smolstr, SmolStr};
use std::rc::Rc;

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
    let is_listview = children
        .iter()
        .any(|c| c.borrow().repeated.as_ref().is_some_and(|r| r.is_listview.is_some()));

    if is_listview {
        // Fox Listview, we don't bind the y property to the geometry because for large listview, we want to support coordinate with more precision than f32
        // so the actual geometry is relative to the Flickable instead of the viewport
        // We still assign a binding to the y property in case it is read by someone
        for c in &children {
            if c.borrow().repeated.is_none() {
                // Normally should not happen, listview should only have one children, and it should be repeated
                continue;
            }
            let ElementType::Component(base) = c.borrow().base_type.clone() else { continue };
            let inner_elem = &base.root_element;
            let new_y = crate::layout::create_new_prop(
                inner_elem,
                SmolStr::new_static("actual-y"),
                Type::LogicalLength,
            );
            new_y.mark_as_set();
            inner_elem.borrow_mut().bindings.insert(
                "y".into(),
                RefCell::new(
                    Expression::BinaryExpression {
                        lhs: Expression::PropertyReference(new_y.clone()).into(),
                        rhs: Expression::PropertyReference(NamedReference::new(
                            flickable,
                            SmolStr::new_static("viewport-y"),
                        ))
                        .into(),
                        op: '-',
                    }
                    .into(),
                ),
            );
            inner_elem.borrow_mut().geometry_props.as_mut().unwrap().y = new_y;
        }
    }

    let viewport = Element::make_rc(Element {
        id: format_smolstr!("{}-viewport", flickable.borrow().id),
        base_type: ElementType::Native(native_empty.clone()),
        children,
        enclosing_component: flickable.borrow().enclosing_component.clone(),
        is_flickable_viewport: true,
        ..Element::default()
    });
    let element_type = flickable.borrow().base_type.clone();
    for prop in element_type.as_builtin().properties.keys() {
        // bind the viewport's property to the flickable property, such as:  `width <=> parent.viewport-width`
        if let Some(vp_prop) = prop.strip_prefix("viewport-") {
            if is_listview && matches!(vp_prop, "y" | "height") {
                //don't bind viewport-y for ListView because the layout is handled by the runtime
                continue;
            }
            viewport.borrow_mut().bindings.insert(
                vp_prop.into(),
                BindingExpression::new_two_way(NamedReference::new(flickable, prop.clone())).into(),
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

    let enclosing_component = flickable.borrow().enclosing_component.upgrade().unwrap();
    if let Some((insertion_point, _, _)) =
        &mut *enclosing_component.child_insertion_point.borrow_mut()
    {
        if std::rc::Rc::ptr_eq(insertion_point, flickable) {
            *insertion_point = viewport.clone()
        }
    }

    flickable.borrow_mut().children.push(viewport);
}

fn fixup_geometry(flickable_elem: &ElementRc) {
    let forward_minmax_of = |prop: &'static str, op: MinMaxOp| {
        set_binding_if_not_explicit(flickable_elem, prop, || {
            flickable_elem
                .borrow()
                .children
                .iter()
                .filter(|x| is_layout(&x.borrow().base_type))
                // FIXME: we should ideally add runtime code to merge layout info of all elements that are repeated (#407)
                .filter(|x| x.borrow().repeated.is_none())
                .map(|x| {
                    Expression::PropertyReference(NamedReference::new(x, SmolStr::new_static(prop)))
                })
                .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, op))
        })
    };

    if !flickable_elem.borrow().bindings.contains_key("height") {
        forward_minmax_of("max-height", MinMaxOp::Min);
        forward_minmax_of("preferred-height", MinMaxOp::Min);
    }
    if !flickable_elem.borrow().bindings.contains_key("width") {
        forward_minmax_of("max-width", MinMaxOp::Min);
        forward_minmax_of("preferred-width", MinMaxOp::Min);
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
                .map(|x| {
                    Expression::PropertyReference(NamedReference::new(
                        x,
                        SmolStr::new_static("min-width"),
                    ))
                })
                .fold(
                    Expression::PropertyReference(NamedReference::new(
                        flickable_elem,
                        SmolStr::new_static("width"),
                    )),
                    |lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max),
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
                .map(|x| {
                    Expression::PropertyReference(NamedReference::new(
                        x,
                        SmolStr::new_static("min-height"),
                    ))
                })
                .fold(
                    Expression::PropertyReference(NamedReference::new(
                        flickable_elem,
                        SmolStr::new_static("height"),
                    )),
                    |lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max),
                ),
        )
    });
}

/// Return true if this type is a layout that has constraints
fn is_layout(base_type: &ElementType) -> bool {
    if let ElementType::Builtin(be) = base_type {
        matches!(be.name.as_str(), "GridLayout" | "HorizontalLayout" | "VerticalLayout")
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
