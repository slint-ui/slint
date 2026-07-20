// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Synthesize the navigator back-stack.
//!
//! `from_navigator_node` records the resolved route table on the enclosing
//! element (`Element::navigator_routes`); navigation itself is just assigning
//! the route property. This pass adds, on that same element, a back-stack plus
//! the surface for going back:
//!
//!   navigate(route): push the current route, then switch to `route`
//!   back():          restore and drop the top of the back-stack (no-op if empty)
//!   can-go-back:     true while the back-stack is non-empty
//!
//! It is expressed entirely by lowering onto the existing property/callback and
//! `Array*` builtin machinery, so it needs no new `internal/core` items.
//!
//! Runs after expression resolution (the route models are typed and the route
//! property is a resolved reference by then) and before inlining. The construct
//! is experimental-gated in `from_navigator_node`, so `navigator_routes` is only
//! ever populated under `enable_experimental`; this pass inherits that gate.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Callable, Expression, NamedReference, Unit};
use crate::langtype::{Function, Type};
use crate::object_tree::*;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::rc::Rc;

// Private storage for the back-stack. A single navigator per element is
// supported, matching the navigation convention.
const BACK_STACK: &str = "navigator-back-stack";

pub fn lower_navigator(doc: &Document, diag: &mut BuildDiagnostics) {
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if !elem.borrow().navigator_routes.is_empty() {
                lower_one(elem, diag);
            }
        })
    });
}

fn lower_one(elem: &ElementRc, diag: &mut BuildDiagnostics) {
    // The route property and its enum type both come from a route case's model:
    // resolve turned each case into `<route-property> == Route.X`.
    let model = elem
        .borrow()
        .navigator_routes
        .first()
        .and_then(|r| r.component.borrow().repeated.as_ref().map(|rep| rep.model.clone()));
    let Some(Expression::BinaryExpression { lhs: route_ref, .. }) = model else {
        return;
    };
    let route_ref = *route_ref;
    let route_ty = route_ref.ty();
    if !matches!(route_ty, Type::Enumeration(_)) {
        return;
    }
    // navigate()/back() write the route back, so it must be an assignable
    // property reference. The convention uses `in-out property <Route>`.
    if !matches!(route_ref, Expression::PropertyReference(_)) {
        diag.push_error(
            "the navigator route must be a writable property to support navigate() and back()"
                .into(),
            &*elem.borrow(),
        );
        return;
    }

    elem.borrow_mut().property_declarations.insert(
        SmolStr::new_static(BACK_STACK),
        PropertyDeclaration {
            property_type: Type::Array(Rc::new(route_ty.clone())),
            visibility: PropertyVisibility::Private,
            ..Default::default()
        },
    );
    // Initialize to an empty array: an unbound model property is a null model
    // whose push/remove are silently no-ops.
    elem.borrow_mut().bindings.insert(
        SmolStr::new_static(BACK_STACK),
        RefCell::new(
            Expression::Array { element_ty: route_ty.clone(), values: vec![] }.into(),
        ),
    );

    let back_stack = || Expression::PropertyReference(NamedReference::new(elem, BACK_STACK.into()));
    let length = || Expression::FunctionCall {
        function: Callable::Builtin(BuiltinFunction::ArrayLength),
        arguments: vec![back_stack()],
        source_location: None,
    };
    // Index of the top of the stack: `length - 1`.
    let top_index = || Expression::BinaryExpression {
        lhs: Box::new(length()),
        rhs: Box::new(Expression::NumberLiteral(1., Unit::None)),
        op: '-',
    };
    let non_empty = || Expression::BinaryExpression {
        lhs: Box::new(length()),
        rhs: Box::new(Expression::NumberLiteral(0., Unit::None)),
        op: '>',
    };
    let assign_route = |rhs: Expression| Expression::SelfAssignment {
        lhs: Box::new(route_ref.clone()),
        rhs: Box::new(rhs),
        op: '=',
        node: None,
    };

    // navigate(route): remember the current route, then switch to the argument.
    let navigate_body = Expression::CodeBlock(vec![
        Expression::FunctionCall {
            function: Callable::Builtin(BuiltinFunction::ArrayPush),
            arguments: vec![back_stack(), route_ref.clone()],
            source_location: None,
        },
        assign_route(Expression::FunctionParameterReference { index: 0, ty: route_ty.clone() }),
    ]);

    // back(): restore the top route, then drop it. The restore reads the top
    // before the remove, so both use the pre-pop length.
    let back_body = Expression::Condition {
        condition: Box::new(non_empty()),
        true_expr: Box::new(Expression::CodeBlock(vec![
            assign_route(Expression::ArrayIndex {
                array: Box::new(back_stack()),
                index: Box::new(top_index()),
            }),
            Expression::FunctionCall {
                function: Callable::Builtin(BuiltinFunction::ArrayRemove),
                arguments: vec![back_stack(), top_index()],
                source_location: None,
            },
        ])),
        false_expr: Box::new(Expression::CodeBlock(vec![])),
    };

    let can_go_back = non_empty();

    // Int-index adapter that relies on `current_index: int` /
    // `index_changed(int)` rather than the route enum. Each route case's
    // resolved model is `route_ref == Route.X`, so its rhs is that route's enum
    // value; collected in declaration order these map ordinal <-> route.
    let route_values: Vec<Expression> = elem
        .borrow()
        .navigator_routes
        .iter()
        .filter_map(|r| {
            match r.component.borrow().repeated.as_ref().map(|rep| rep.model.clone()) {
                Some(Expression::BinaryExpression { rhs, .. }) => Some(*rhs),
                _ => None,
            }
        })
        .collect();

    // current-route-index: ordinal of the current route, else -1. Lowered as an
    // if-chain `current == route0 ? 0 : current == route1 ? 1 : ... : -1`.
    let current_route_index = route_values.iter().enumerate().rev().fold(
        Expression::NumberLiteral(-1., Unit::None),
        |otherwise, (i, route_value)| Expression::Condition {
            condition: Box::new(Expression::BinaryExpression {
                lhs: Box::new(route_ref.clone()),
                rhs: Box::new(route_value.clone()),
                op: '=',
            }),
            true_expr: Box::new(Expression::NumberLiteral(i as f64, Unit::None)),
            false_expr: Box::new(otherwise),
        },
    );

    // navigate-index(index): navigate to the route at that ordinal using the same
    // push-then-set logic as navigate(); an out-of-range index is a no-op.
    let index_param = || Expression::FunctionParameterReference { index: 0, ty: Type::Int32 };
    let navigate_index_body = route_values.iter().enumerate().rev().fold(
        Expression::CodeBlock(vec![]),
        |otherwise, (i, route_value)| Expression::Condition {
            condition: Box::new(Expression::BinaryExpression {
                lhs: Box::new(index_param()),
                rhs: Box::new(Expression::NumberLiteral(i as f64, Unit::None)),
                op: '=',
            }),
            true_expr: Box::new(Expression::CodeBlock(vec![
                Expression::FunctionCall {
                    function: Callable::Builtin(BuiltinFunction::ArrayPush),
                    arguments: vec![back_stack(), route_ref.clone()],
                    source_location: None,
                },
                assign_route(route_value.clone()),
            ])),
            false_expr: Box::new(otherwise),
        },
    );

    let mut e = elem.borrow_mut();
    e.property_declarations.insert(
        SmolStr::new_static("navigate"),
        PropertyDeclaration {
            property_type: Type::Function(Rc::new(Function {
                return_type: Type::Void,
                args: vec![route_ty.clone()],
                arg_names: vec![SmolStr::new_static("route")],
            })),
            visibility: PropertyVisibility::Public,
            pure: Some(false),
            // Synthesized after check_public_api; opt into the public surface so
            // callers can invoke it and remove_unused_properties keeps it.
            expose_in_public_api: true,
            ..Default::default()
        },
    );
    e.bindings.insert(SmolStr::new_static("navigate"), RefCell::new(navigate_body.into()));

    e.property_declarations.insert(
        SmolStr::new_static("back"),
        PropertyDeclaration {
            property_type: Type::Function(Rc::new(Function {
                return_type: Type::Void,
                args: vec![],
                arg_names: vec![],
            })),
            visibility: PropertyVisibility::Public,
            pure: Some(false),
            expose_in_public_api: true,
            ..Default::default()
        },
    );
    e.bindings.insert(SmolStr::new_static("back"), RefCell::new(back_body.into()));

    e.property_declarations.insert(
        SmolStr::new_static("can-go-back"),
        PropertyDeclaration {
            property_type: Type::Bool,
            visibility: PropertyVisibility::Output,
            expose_in_public_api: true,
            ..Default::default()
        },
    );
    e.bindings.insert(SmolStr::new_static("can-go-back"), RefCell::new(can_go_back.into()));

    e.property_declarations.insert(
        SmolStr::new_static("current-route-index"),
        PropertyDeclaration {
            property_type: Type::Int32,
            visibility: PropertyVisibility::Output,
            expose_in_public_api: true,
            ..Default::default()
        },
    );
    e.bindings.insert(
        SmolStr::new_static("current-route-index"),
        RefCell::new(current_route_index.into()),
    );

    e.property_declarations.insert(
        SmolStr::new_static("navigate-index"),
        PropertyDeclaration {
            property_type: Type::Function(Rc::new(Function {
                return_type: Type::Void,
                args: vec![Type::Int32],
                arg_names: vec![SmolStr::new_static("index")],
            })),
            visibility: PropertyVisibility::Public,
            pure: Some(false),
            expose_in_public_api: true,
            ..Default::default()
        },
    );
    e.bindings
        .insert(SmolStr::new_static("navigate-index"), RefCell::new(navigate_index_body.into()));
}
