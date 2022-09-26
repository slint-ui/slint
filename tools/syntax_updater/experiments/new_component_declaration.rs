// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::Cli;
use i_slint_compiler::{
    langtype::Type,
    object_tree::ElementRc,
    parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode},
};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    debug_assert!(args.new_component_declaration);
    let kind = node.kind();
    if kind == SyntaxKind::Component {
        let is_global =
            node.child_token(SyntaxKind::Identifier).map_or(false, |t| t.text() == "global");
        if !is_global {
            write!(file, "component ")?;
        }
        for n in node.children_with_tokens() {
            if n.kind() == SyntaxKind::ColonEqual {
                // ignore that
            } else {
                crate::visit_node_or_token(n, file, state, args)?;
            }
        }
        return Ok(true);
    }
    Ok(false)
}

/// Called for each node directly under SyntaxNode::Element in a component
///
/// returns true if we should not print this
pub(crate) fn process_component_node(
    element: &syntax_nodes::Element,
    node: &NodeOrToken,
    file: &mut impl Write,
    generated_sub_element: &mut bool,
    state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    let k = node.kind();
    if node.kind() == SyntaxKind::QualifiedName {
        return Ok(true);
    }

    if !*generated_sub_element
        && matches!(
            k,
            SyntaxKind::SubElement | SyntaxKind::ChildrenPlaceholder | SyntaxKind::RBrace
        )
    {
        *generated_sub_element = true;

        if let Some(e) = &state.current_elem {
            add_base_two_way_bindings(e.clone(), args, file)?;
        }

        if let Some(base) = element.QualifiedName() {
            write!(file, "__old_root := \n    ")?;
            crate::visit_node(base.into(), file, state, args)?;
            write!(file, "{{ \n    ")?;
        }
    }

    if k == SyntaxKind::RBrace {
        write!(file, "}}")?;
    }

    Ok(false)
}

fn add_base_two_way_bindings(
    mut e: ElementRc,
    args: &Cli,
    file: &mut impl Write,
) -> std::io::Result<()> {
    loop {
        let base_type = e.borrow().base_type.clone();
        match base_type {
            Type::Component(c) => {
                e = c.root_element.clone();
                for p in &e.borrow().property_declarations {
                    if args.input_output_properties {
                        write!(file, "inout ")?;
                    }
                    write!(file, "property {0} <=> __old_root.{0};\n    ", p.0)?;
                }
            }
            Type::Builtin(b) => {
                for p in &b.properties {
                    if matches!(p.0.as_str(), "x" | "y") {
                        continue;
                    }
                    if matches!(p.0.as_str(), "height" | "width") {
                        write!(file, "{0} <=> __old_root.{0};\n    ", p.0)?;
                        continue;
                    }
                    if args.input_output_properties {
                        write!(file, "inout ")?;
                    }
                    write!(file, "property {0} <=> __old_root.{0};\n    ", p.0)?;
                }
                return Ok(());
            }
            _ => return Ok(()),
        }
    }
}
