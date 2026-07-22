// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore radiogroup

//! This pass lowers `RadioGroup` (a builtin pseudo-element) and its
//! `RadioButton` children to the style's `RadioGroupImpl` / `RadioButtonImpl`.
//!
//! It must run before inlining because the lowered tree references components
//! that themselves need to be inlined.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Callable, Expression, NamedReference, Unit};
use crate::langtype::{ElementType, Type};
use crate::object_tree::*;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub async fn lower_radiogroup(
    doc: &Document,
    type_loader: &mut crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    // Collect before lowering: lowering rewrites base_type, which would hide
    // other RadioGroup elements from builtin_type() (an instance and the style
    // wrapper both match). Dedup a sub-component root visited more than once.
    let mut seen = HashSet::new();
    let mut radio_groups = Vec::new();
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if matches!(&elem.borrow().builtin_type(), Some(b) if b.name == "RadioGroup")
                && seen.insert(Rc::as_ptr(elem))
            {
                radio_groups.push(elem.clone());
            }
        })
    });
    if radio_groups.is_empty() {
        return;
    }

    let mut ignore = BuildDiagnostics::default();
    let radio_group_impl = type_loader
        .import_component("std-widgets-impl.slint", "RadioGroupImpl", &mut ignore)
        .await
        .expect("RadioGroupImpl should be in std-widgets-impl.slint");
    let radio_button_impl = type_loader
        .import_component("std-widgets-impl.slint", "RadioButtonImpl", &mut ignore)
        .await
        .expect("RadioButtonImpl should be in std-widgets-impl.slint");

    for elem in &radio_groups {
        process_radiogroup(
            elem,
            ElementType::Component(radio_group_impl.clone()),
            ElementType::Component(radio_button_impl.clone()),
            diag,
        );
    }
}

fn process_radiogroup(
    elem: &ElementRc,
    radio_group_impl: ElementType,
    radio_button_impl: ElementType,
    diag: &mut BuildDiagnostics,
) {
    // Borrow the children read-only for validation; do not take them out of
    // the element yet, so that any early-return error path leaves the element
    // intact for downstream passes (LSP/live-preview keep going past errors).
    let children = elem.borrow().children.clone();

    for child in &children {
        if !matches!(&child.borrow().base_type, ElementType::Builtin(b) if b.name == "RadioButton")
        {
            diag.push_error(
                format!(
                    "Only RadioButton is allowed inside RadioGroup, found {}",
                    child.borrow().base_type
                ),
                &*child.borrow(),
            );
            elem.borrow_mut().base_type = radio_group_impl;
            return;
        }
    }

    // `if` is not allowed inside RadioGroup: there is no obvious way to keep
    // selection state stable as conditions flip on and off. Static-only or a
    // single `for` are the only shapes accepted.
    if let Some(if_child) = children
        .iter()
        .find(|c| c.borrow().repeated.as_ref().is_some_and(|r| r.is_conditional_element))
    {
        diag.push_error("`if` is not allowed inside RadioGroup".into(), &*if_child.borrow());
        elem.borrow_mut().base_type = radio_group_impl;
        return;
    }

    // A `for` loop must be the only child: each for-instance uses the
    // repeater's running index, which would collide with the source-position
    // index used by static siblings. The filter explicitly excludes
    // conditional repeaters even though those are already rejected above,
    // so a future relaxation of the `if` rule can't make this message fire
    // on the wrong shape.
    if let Some(for_child) = children
        .iter()
        .find(|c| c.borrow().repeated.as_ref().is_some_and(|r| !r.is_conditional_element))
        && children.len() > 1
    {
        diag.push_error(
            "A `for` loop must be the only child of RadioGroup".into(),
            &*for_child.borrow(),
        );
        elem.borrow_mut().base_type = radio_group_impl;
        return;
    }

    elem.borrow_mut().base_type = radio_group_impl;

    let count_expr = match children.first().and_then(|c| c.borrow().repeated.clone()) {
        Some(rep) if children.len() == 1 => Expression::FunctionCall {
            function: Callable::Builtin(crate::expression_tree::BuiltinFunction::ArrayLength),
            arguments: vec![rep.model],
            source_location: None,
        },
        _ => Expression::NumberLiteral(children.len() as f64, Unit::None),
    };
    elem.borrow_mut()
        .bindings
        .insert(SmolStr::new_static("item-count"), RefCell::new(count_expr.into()));

    for (position, child) in children.iter().enumerate() {
        let item_index_expr = match &child.borrow().repeated {
            Some(_) => Expression::RepeaterIndexReference { element: Rc::downgrade(child) },
            None => Expression::NumberLiteral(position as f64, Unit::None),
        };
        wire_radio_button(elem, child, &radio_button_impl, item_index_expr);
    }
}

fn wire_radio_button(
    group: &ElementRc,
    child: &ElementRc,
    radio_button_impl: &ElementType,
    item_index_expr: Expression,
) {
    child.borrow_mut().base_type = radio_button_impl.clone();

    child
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static("item-index"), RefCell::new(item_index_expr.into()));

    child.borrow_mut().bindings.insert(
        SmolStr::new_static("group-enabled"),
        RefCell::new(
            Expression::PropertyReference(NamedReference::new(group, "enabled".into())).into(),
        ),
    );

    // row / col for the parent GridLayout — stack vertically (column 0,
    // increasing rows) for vertical orientation, otherwise stack horizontally.
    let orientation_vertical = crate::typeregister::BUILTIN
        .with(|e| e.enums.Orientation.clone())
        .try_value_from_string("vertical")
        .unwrap();
    let is_vertical = Expression::BinaryExpression {
        lhs: Expression::PropertyReference(NamedReference::new(group, "orientation".into())).into(),
        rhs: Expression::EnumerationValue(orientation_vertical).into(),
        op: '=',
    };
    let item_index_ref =
        || Expression::PropertyReference(NamedReference::new(child, "item-index".into()));
    let row_expr = Expression::Condition {
        condition: is_vertical.clone().into(),
        true_expr: item_index_ref().into(),
        false_expr: Expression::NumberLiteral(0.0, Unit::None).into(),
    };
    let col_expr = Expression::Condition {
        condition: is_vertical.into(),
        true_expr: Expression::NumberLiteral(0.0, Unit::None).into(),
        false_expr: item_index_ref().into(),
    };
    child.borrow_mut().bindings.insert(SmolStr::new_static("row"), RefCell::new(row_expr.into()));
    child.borrow_mut().bindings.insert(SmolStr::new_static("col"), RefCell::new(col_expr.into()));

    // Bind `group-current-index` rather than `checked` directly: the latter
    // is `in-out` so users can toggle it from outside, and an imperative
    // write would replace the binding and break the radio-group invariant.
    // The impl base's `changed group-current-index` handler is responsible
    // for syncing `checked`.
    let group_current_index_expr =
        Expression::PropertyReference(NamedReference::new(group, "current-index".into()));
    child.borrow_mut().bindings.insert(
        SmolStr::new_static("group-current-index"),
        RefCell::new(group_current_index_expr.into()),
    );

    let select_call = Expression::FunctionCall {
        function: Callable::Function(NamedReference::new(group, "select".into())),
        arguments: vec![
            Expression::PropertyReference(NamedReference::new(child, "item-index".into())),
            Expression::PropertyReference(NamedReference::new(child, "text".into())),
        ],
        source_location: None,
    };
    child
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static("group-select"), RefCell::new(select_call.into()));

    let focus_call = Expression::FunctionCall {
        function: Callable::Function(NamedReference::new(group, "on-focus-change".into())),
        arguments: vec![Expression::FunctionParameterReference { index: 0, ty: Type::Bool }],
        source_location: None,
    };
    child
        .borrow_mut()
        .bindings
        .insert(SmolStr::new_static("focus-change"), RefCell::new(focus_call.into()));
}
