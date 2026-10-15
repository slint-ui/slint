// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Implements a ListView's `ensure-row-visible` function with a call into the ListView's repeater.

use crate::builtin_functions::BuiltinFunction;
use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Callable, Expression};
use crate::langtype::Type;
use crate::object_tree::{Component, Element, ElementRc};
use crate::parser::{SyntaxKind, SyntaxToken};
use crate::typeregister;
use std::rc::Rc;

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
                diag.push_error(
                    "Place the 'for' of a 'ListView' with 'ensure-row-visible' directly in a 'Flickable'".into(),
                    &for_keyword(&element),
                );
                diag.push_note("'ensure-row-visible' is declared here".into(), &*binding.borrow());
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

fn for_keyword(repeated: &Element) -> Option<SyntaxToken> {
    let node = &repeated.debug.first()?.node;
    std::iter::successors(node.parent(), |n| n.parent())
        .find(|n| n.kind() == SyntaxKind::RepeatedElement)?
        .child_token(SyntaxKind::Identifier)
}
