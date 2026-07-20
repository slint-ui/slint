// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Fill in the navigator's public member bodies and add its private back-stack.
//!
//! Runs after the route table and member signatures are set up by `from_navigator_node`.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Callable, Expression, NamedReference, Unit};
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::rc::Rc;

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

    elem.borrow_mut().bindings.insert(
        SmolStr::new_static(BACK_STACK),
        RefCell::new(Expression::Array { element_ty: route_ty.clone(), values: vec![] }.into()),
    );

    let back_stack = || Expression::PropertyReference(NamedReference::new(elem, BACK_STACK.into()));
    let length = || Expression::FunctionCall {
        function: Callable::Builtin(BuiltinFunction::ArrayLength),
        arguments: vec![back_stack()],
        source_location: None,
    };
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

    let navigate_body = Expression::CodeBlock(vec![
        Expression::FunctionCall {
            function: Callable::Builtin(BuiltinFunction::ArrayPush),
            arguments: vec![back_stack(), route_ref.clone()],
            source_location: None,
        },
        assign_route(Expression::FunctionParameterReference { index: 0, ty: route_ty.clone() }),
    ]);

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

    let route_values: Vec<Expression> = elem
        .borrow()
        .navigator_routes
        .iter()
        .filter_map(|r| match r.component.borrow().repeated.as_ref().map(|rep| rep.model.clone()) {
            Some(Expression::BinaryExpression { rhs, .. }) => Some(*rhs),
            _ => None,
        })
        .collect();

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
    e.bindings.insert(SmolStr::new_static("navigate"), RefCell::new(navigate_body.into()));
    e.bindings.insert(SmolStr::new_static("back"), RefCell::new(back_body.into()));
    e.bindings.insert(SmolStr::new_static("can-go-back"), RefCell::new(can_go_back.into()));
    e.bindings.insert(
        SmolStr::new_static("current-route-index"),
        RefCell::new(current_route_index.into()),
    );
    e.bindings
        .insert(SmolStr::new_static("navigate-index"), RefCell::new(navigate_index_body.into()));
}
