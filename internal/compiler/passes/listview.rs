// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Implements a ListView's `ensure-row-visible` function with a call into the ListView's repeater.

use crate::builtin_functions::BuiltinFunction;
use crate::diagnostics::BuildDiagnostics;
use crate::element_type::ElementType;
use crate::expression_tree::{Callable, Expression};
use crate::langtype::Type;
use crate::object_tree::{Component, DEFAULT_SLOT_NAME, Element, ElementRc};
use crate::typeregister;
use std::rc::Rc;

/// Reports a `ListView` with `ensure-row-visible` whose children don't end up directly in a `Flickable`.
pub fn check_children_in_flickable(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    let root = component.root_element.borrow();
    if component.id != "ListView" || !Element::declares_ensure_row_visible(&root) {
        return;
    }
    if matches!(children_type(&ElementType::Component(component.clone())), ElementType::Builtin(b) if b.name == "Flickable")
    {
        return;
    }
    let message = "Place '@children' directly in a 'Flickable' so that 'ensure-row-visible' can scroll this 'ListView'";
    match component.child_insertion_points.borrow().get(DEFAULT_SLOT_NAME) {
        Some(children) => diag.push_error(message.into(), &children.node),
        None => match &component.node {
            Some(node) => diag.push_error(message.into(), &node.DeclaredIdentifier()),
            None => diag.push_error(message.into(), &*root),
        },
    }
    if let Some(binding) = root.bindings.binding_cell_including_synthetic("ensure-row-visible") {
        diag.push_note("'ensure-row-visible' is declared here".into(), &*binding.borrow());
    }
}

/// The type of the element that the children of an element of type `ty` end up in,
/// like `ElementType::lookup_type_for_child_element`.
fn children_type(ty: &ElementType) -> ElementType {
    let ElementType::Component(component) = ty else { return ty.clone() };
    let parent = component
        .child_insertion_points
        .borrow()
        .get(DEFAULT_SLOT_NAME)
        .map_or_else(|| component.root_element.clone(), |children| children.parent.clone());
    let base_type = parent.borrow().base_type.clone();
    children_type(&base_type)
}

pub fn lower_ensure_row_visible(root_component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(
        root_component,
        &None,
        &mut |element_rc: &ElementRc, parent: &Option<ElementRc>| {
            let element = element_rc.borrow();
            let Some(listview) = element.repeated.as_ref().and_then(|r| r.is_listview.as_ref())
            else {
                return Some(element_rc.clone());
            };
            let Some(nr) = &listview.ensure_row_visible else {
                return Some(element_rc.clone());
            };
            let listview_elem = nr.element();
            let listview_elem = listview_elem.borrow();
            let Some(binding) = listview_elem.bindings.binding_cell_including_synthetic(nr.name())
            else {
                return Some(element_rc.clone());
            };
            // The ensure-row-visible is already routed to the correct function so we don't have to do it again
            if matches!(
                &binding.borrow().expression,
                Expression::FunctionCall {
                    function: Callable::Builtin(BuiltinFunction::ListViewEnsureRowVisible),
                    ..
                }
            ) {
                return Some(element_rc.clone());
            }
            if !matches!(&binding.borrow().expression, Expression::CodeBlock(v) if v.is_empty()) {
                diag.push_error(
                    "Remove the body of 'ensure-row-visible', the compiler provides it for a 'ListView'".into(),
                    &*binding.borrow(),
                );
                return Some(element_rc.clone());
            }
            let Some(flickable) = parent
                .as_ref()
                .filter(|p| p.borrow().builtin_type().is_some_and(|b| b.name == "Flickable"))
            else {
                // See `check_children_in_flickable`.
                return Some(element_rc.clone());
            };

            binding.borrow_mut().expression = Expression::FunctionCall {
                function: Callable::Builtin(BuiltinFunction::ListViewEnsureRowVisible),
                arguments: vec![
                    Expression::ElementReference(Rc::downgrade(element_rc)),
                    Expression::ElementReference(Rc::downgrade(flickable)),
                    Expression::FunctionParameterReference { index: 0, ty: Type::Int32 },
                    Expression::FunctionParameterReference {
                        index: 1,
                        ty: Type::Enumeration(typeregister::BUILTIN.enums.ScrollBehavior.clone()),
                    },
                ],
                source_location: None,
            };
            Some(element_rc.clone())
        },
    );
}
