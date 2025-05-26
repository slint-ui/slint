// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Cli;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    let kind = node.kind();
    if kind == SyntaxKind::Element {
        if state.lookup_change.scope.len() >= 2 {
            let elem = &state.lookup_change.scope[state.lookup_change.scope.len() - 1];
            let parent = &state.lookup_change.scope[state.lookup_change.scope.len() - 2];

            if !is_layout_base(parent) && !is_path(elem) && elem.borrow().is_legacy_syntax {
                let extend = elem.borrow().builtin_type().is_some_and(|b| {
                    b.default_size_binding
                        != i_slint_compiler::langtype::DefaultSizeBinding::ImplicitSize
                });

                let new_props = format!(
                    "{}{}",
                    new_geometry_binding(elem, "x", "width", extend),
                    new_geometry_binding(elem, "y", "height", extend)
                );
                if !new_props.is_empty() {
                    let mut seen_brace = false;
                    for c in node.children_with_tokens() {
                        if seen_brace {
                            if c.kind() == SyntaxKind::Whitespace {
                                crate::visit_node_or_token(c.clone(), file, state, args)?;
                            }
                            write!(file, "{new_props}")?;
                            seen_brace = false;
                        } else if c.kind() == SyntaxKind::LBrace {
                            seen_brace = true;
                        }
                        crate::visit_node_or_token(c, file, state, args)?;
                    }
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn new_geometry_binding(elem: &ElementRc, pos_prop: &str, size_prop: &str, extend: bool) -> String {
    if elem.borrow().lookup_property(pos_prop).property_type != Type::LogicalLength {
        return String::default();
    }
    if elem.borrow().is_binding_set(pos_prop, false) {
        return String::default();
    }
    if extend && !elem.borrow().is_binding_set(size_prop, false) {
        return String::default();
    }
    if let Some(b) = elem.borrow().bindings.get(size_prop) {
        if let Expression::Uncompiled(x) = &b.borrow().expression {
            let s = x.to_string();
            if s.trim() == "100%;" || s.trim() == format!("parent.{size_prop};") {
                return String::default();
            }
        }
    }

    format!("{pos_prop}:0;")
}

fn is_layout_base(elem: &ElementRc) -> bool {
    match &elem.borrow().base_type {
        i_slint_compiler::langtype::ElementType::Builtin(b) => {
            matches!(
                b.name.as_str(),
                "GridLayout" | "HorizontalLayout" | "VerticalLayout" | "Row" | "Path" | "Dialog"
            )
        }
        i_slint_compiler::langtype::ElementType::Component(c) => {
            if c.id == "ListView" {
                return true;
            }
            if let Some(ins) = &*c.child_insertion_point.borrow() {
                is_layout_base(&ins.parent)
            } else {
                is_layout_base(&c.root_element)
            }
        }
        _ => false,
    }
}

fn is_path(elem: &ElementRc) -> bool {
    // Path's children are still considered in the Path for some reason, so just don't touch path
    elem.borrow().base_type.to_string() == "Path"
}
