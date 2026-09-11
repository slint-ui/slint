// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Flickable pass
//!
//! The Flickable element is special in the sense that it has a content element
//! which is not exposed. This passes create the content element and fixes all property access
//!
//! It will also initialize proper geometry
//! This pass must be called before the materialize_fake_properties as it going to be generate
//! binding reference to fake properties

use crate::expression_tree::{BindingExpression, Expression, MinMaxOp, NamedReference, Unit};
use crate::langtype::{ElementType, NativeClass, Type};
use crate::layout::{Orientation, is_layout, repeated_element_layout_info};
use crate::object_tree::{Component, Element, ElementRc};
use crate::typeregister::TypeRegister;
use smol_str::{SmolStr, format_smolstr};
use std::rc::Rc;
use std::sync::Arc;

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
            create_content_element(elem, &native_empty);
        },
    )
}

fn create_content_element(flickable: &ElementRc, native_empty: &Arc<NativeClass>) {
    let children = std::mem::take(&mut flickable.borrow_mut().children);
    let is_listview = children
        .iter()
        .find_map(|c| c.borrow().repeated.as_ref().and_then(|r| r.is_listview.clone()));

    if let Some(listview) = &is_listview {
        // Fox Listview, we don't bind the y property to the geometry because for large listview, we want to support coordinate with more precision than f32
        // so the actual geometry is relative to the Flickable instead of the content element
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
            inner_elem.borrow_mut().set_binding(
                "y".into(),
                Expression::BinaryExpression {
                    lhs: Expression::PropertyReference(new_y.clone()).into(),
                    rhs: Expression::PropertyReference(listview.content_y.clone()).into(),
                    op: '-',
                }
                .into(),
            );
            inner_elem.borrow_mut().geometry_props.as_mut().unwrap().y = new_y;
        }
    }

    let content = Element::make_rc(Element {
        id: format_smolstr!("{}-content", flickable.borrow().id),
        base_type: ElementType::Native(native_empty.clone()),
        children,
        enclosing_component: flickable.borrow().enclosing_component.clone(),
        is_flickable_content: true,
        ..Element::default()
    });
    let element_type = flickable.borrow().base_type.clone();
    for prop in element_type.as_builtin().properties.keys() {
        // bind the content's property to the flickable property, such as:  `width <=> parent.content-width`
        if let Some(content_prop) = prop.strip_prefix("content-") {
            if is_listview.is_some() && matches!(content_prop, "y" | "height") {
                //don't bind content-y for ListView because the layout is handled by the runtime
                continue;
            }
            content.borrow_mut().set_binding(
                content_prop.into(),
                BindingExpression::new_two_way(NamedReference::new(flickable, prop.clone()).into()),
            );
        }
    }
    content
        .borrow()
        .property_analysis
        .borrow_mut()
        .entry("y".into())
        .or_default()
        .is_set_externally = true;
    content
        .borrow()
        .property_analysis
        .borrow_mut()
        .entry("x".into())
        .or_default()
        .is_set_externally = true;

    let enclosing_component = flickable.borrow().enclosing_component.upgrade().unwrap();
    for insertion_point in enclosing_component.child_insertion_points.borrow_mut().values_mut() {
        if std::rc::Rc::ptr_eq(&insertion_point.parent, flickable) {
            insertion_point.parent = content.clone()
        }
    }

    flickable.borrow_mut().children.push(content);
}

/// The scalar `scalar_prop` (e.g. `max-height`) of a `is_layout` child `x` of the
/// Flickable, for use in `fixup_geometry`'s folds.
///
/// A repeated (`if`/`for`) child has no such property of its own — it exists as N
/// runtime instances — so its merged `LayoutInfo` (issue #407) is synthesized
/// instead and the matching `struct_field` (`max`, `preferred`, or `min`) is read
/// back out of it.
///
/// A repeater with no live instance merges to `LayoutInfo::default()`, whose
/// `preferred` is zero. The folds below take the *smallest* preferred size, so
/// that zero would collapse the Flickable: an `if` whose condition is false would
/// shrink it to nothing. Zero means "no preference" here, so it is folded as the
/// identity instead. `max` needs no such care — it defaults to `Coord::MAX`,
/// already the identity of its own fold.
fn layout_child_scalar(
    x: &ElementRc,
    orientation: Orientation,
    scalar_prop: &'static str,
    struct_field: &'static str,
) -> Expression {
    if x.borrow().repeated.is_none() {
        return Expression::PropertyReference(NamedReference::new(
            x,
            SmolStr::new_static(scalar_prop),
        ));
    }
    let merged = Expression::StructFieldAccess {
        base: Box::new(repeated_element_layout_info(x, orientation)),
        name: SmolStr::new_static(struct_field),
    };
    if struct_field != "preferred" {
        return merged;
    }
    let name = SmolStr::new_static("repeated_preferred");
    let read = || Expression::ReadLocalVariable { name: name.clone(), ty: Type::LogicalLength };
    Expression::CodeBlock(
        [
            Expression::StoreLocalVariable { name: name.clone(), value: Box::new(merged) },
            Expression::Condition {
                condition: Box::new(Expression::BinaryExpression {
                    lhs: Box::new(read()),
                    rhs: Box::new(Expression::NumberLiteral(0., Unit::Px)),
                    op: '=',
                }),
                true_expr: Box::new(Expression::NumberLiteral(f32::MAX as f64, Unit::Px)),
                false_expr: Box::new(read()),
            },
        ]
        .into(),
    )
}

fn fixup_geometry(flickable_elem: &ElementRc) {
    // A ListView's repeater only materializes the currently-visible instances
    // (`ensure_updated_listview`), so folding it here would make the constraint
    // change as the user scrolls. Any other repeated child is fully
    // materialized before layout runs and is safe to merge.
    let is_mergeable = |x: &&ElementRc| {
        is_layout(&x.borrow().base_type)
            && x.borrow().repeated.as_ref().is_none_or(|r| r.is_listview.is_none())
    };

    let forward_minmax_of =
        |prop: &'static str, struct_field: &'static str, orientation: Orientation, op: MinMaxOp| {
            set_binding_if_not_explicit(flickable_elem, prop, || {
                flickable_elem
                    .borrow()
                    .children
                    .iter()
                    .filter(is_mergeable)
                    .map(|x| layout_child_scalar(x, orientation, prop, struct_field))
                    .reduce(|lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, op))
            })
        };

    if !flickable_elem.borrow().is_binding_set("height", false) {
        forward_minmax_of("max-height", "max", Orientation::Vertical, MinMaxOp::Min);
        forward_minmax_of("preferred-height", "preferred", Orientation::Vertical, MinMaxOp::Min);
    }
    if !flickable_elem.borrow().is_binding_set("width", false) {
        forward_minmax_of("max-width", "max", Orientation::Horizontal, MinMaxOp::Min);
        forward_minmax_of("preferred-width", "preferred", Orientation::Horizontal, MinMaxOp::Min);
    }
    set_binding_if_not_explicit(flickable_elem, "content-width", || {
        Some(
            flickable_elem
                .borrow()
                .children
                .iter()
                .filter(is_mergeable)
                .map(|x| layout_child_scalar(x, Orientation::Horizontal, "min-width", "min"))
                .fold(
                    Expression::PropertyReference(NamedReference::new(
                        flickable_elem,
                        SmolStr::new_static("width"),
                    )),
                    |lhs, rhs| crate::builtin_macros::min_max_expression(lhs, rhs, MinMaxOp::Max),
                ),
        )
    });
    set_binding_if_not_explicit(flickable_elem, "content-height", || {
        Some(
            flickable_elem
                .borrow()
                .children
                .iter()
                .filter(is_mergeable)
                .map(|x| layout_child_scalar(x, Orientation::Vertical, "min-height", "min"))
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

/// Set the property binding on the given element to the given expression (computed lazily).
/// The parameter to the lazily calculation is the element's children
fn set_binding_if_not_explicit(
    elem: &ElementRc,
    property: &str,
    expression: impl FnOnce() -> Option<Expression>,
) {
    // we can't use `set_binding_if_not_set` directly because `expression()` may borrow `elem`
    //
    // Be careful to check both that a binding exists and that the BindingExpression actually has a
    // binding by using is_binding_set instead of binding().is_none().
    // Otherwise an animation on the property would prevent setting the binding, even if the binding
    // is not set, but just animated.
    if !elem.borrow().is_binding_set(property, false)
        && let Some(e) = expression()
    {
        elem.borrow_mut().set_binding_if_not_set(property.into(), || e);
    }
}
