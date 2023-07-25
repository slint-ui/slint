// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Remove the rectangles that serves no purposes
//!
//! Rectangles which do not draw anything and have no x or y don't need to be in
//! the item tree, we can just remove them.

use crate::expression_tree::{BindingExpression, Expression};
use crate::langtype::{ElementType, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn optimize_useless_rectangles(root_component: &Rc<Component>) {
    recurse_elem_including_sub_components(root_component, &(), &mut |parent_, _| {
        let mut parent = parent_.borrow_mut();
        let children = std::mem::take(&mut parent.children);

        for elem in children {
            let res = can_optimize(&elem);
            if !res.can_optimize {
                parent.children.push(elem);
                continue;
            }

            let children = std::mem::take(&mut elem.borrow_mut().children);
            for child in &children {
                patch_position(child, res.x.as_ref(), res.y.as_ref());
            }
            parent.children.extend(children);

            let enclosing = parent.enclosing_component.upgrade().unwrap();

            for popup in enclosing.popup_windows.borrow_mut().iter_mut() {
                if Rc::ptr_eq(&popup.parent_element, &elem) {
                    // TODO patch x/y
                    // parent element is use for x/y, and the position of the removed element is 0,0
                    popup.parent_element = parent_.clone();
                }
            }

            enclosing.optimized_elements.borrow_mut().push(elem);
        }
    });
}

/// Used for uniquely name some variables
static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

fn patch_position(
    elem: &ElementRc,
    parent_x: Option<&NamedReference>,
    parent_y: Option<&NamedReference>,
) {
    if elem.borrow().repeated.is_some() {
        return;
    }
    let geo = elem.borrow_mut().geometry_props.take();
    if let Some(mut geo) = geo {
        if let Some(x) = parent_x {
            let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let new_x_name = format!("x-optimized-{id}");
            elem.borrow_mut().property_declarations.insert(
                new_x_name.clone(),
                PropertyDeclaration {
                    property_type: Type::LogicalLength,
                    ..PropertyDeclaration::default()
                },
            );

            elem.borrow_mut().bindings.insert(
                new_x_name.clone(),
                RefCell::new(BindingExpression::from(Expression::BinaryExpression {
                    lhs: Box::new(Expression::PropertyReference(x.clone())),
                    rhs: Box::new(Expression::PropertyReference(geo.x.clone())),
                    op: '+',
                })),
            );

            geo.x = NamedReference::new(elem, &new_x_name);
        }

        if let Some(y) = parent_y {
            let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let new_y_name = format!("y-optimized-{id}");
            elem.borrow_mut().property_declarations.insert(
                new_y_name.clone(),
                PropertyDeclaration {
                    property_type: Type::LogicalLength,
                    ..PropertyDeclaration::default()
                },
            );

            elem.borrow_mut().bindings.insert(
                new_y_name.clone(),
                RefCell::new(BindingExpression::from(Expression::BinaryExpression {
                    lhs: Box::new(Expression::PropertyReference(y.clone())),
                    rhs: Box::new(Expression::PropertyReference(geo.y.clone())),
                    op: '+',
                })),
            );

            geo.y = NamedReference::new(elem, &new_y_name);
        }
        elem.borrow_mut().geometry_props = Some(geo);
    }
}

struct OptimizeResult {
    can_optimize: bool,
    x: Option<NamedReference>,
    y: Option<NamedReference>,
}

impl OptimizeResult {
    fn keep() -> Self {
        Self { can_optimize: false, x: None, y: None }
    }
}

/// Check that this is a element we can optimize
fn can_optimize(elem: &ElementRc) -> OptimizeResult {
    let e = elem.borrow();
    if e.is_flickable_viewport || e.has_popup_child {
        return OptimizeResult::keep();
    };

    if e.child_of_layout {
        // The `LayoutItem` still has reference to this component, so we cannot remove it
        return OptimizeResult::keep();
    }

    let base_type = match &e.base_type {
        ElementType::Builtin(base_type) if base_type.name == "Rectangle" => base_type,
        ElementType::Builtin(base_type) if base_type.native_class.class_name == "Empty" => {
            base_type
        }
        _ => return OptimizeResult::keep(),
    };

    let (x, y) = if let Some(g) = &e.geometry_props {
        let ex = g.x.element();
        let ex = ex.borrow();
        let x = if ex.bindings.get(g.x.name()).is_some() { Some(g.x.clone()) } else { None };

        let ey = g.y.element();
        let ey = ey.borrow();
        let y = if ey.bindings.get(g.y.name()).is_some() { Some(g.y.clone()) } else { None };

        (x, y)
    } else {
        (None, None)
    };

    // Check that no Rectangle property other than height and width are set
    let analysis = e.property_analysis.borrow();
    let can_optimize =
        !e.bindings.keys().chain(analysis.iter().filter(|(_, v)| v.is_set).map(|(k, _)| k)).any(
            |k| {
                !matches!(k.as_str(), "height" | "width")
                    && !e.property_declarations.contains_key(k.as_str())
                    && base_type.properties.contains_key(k.as_str())
            },
        );

    OptimizeResult { can_optimize, x, y }
}
