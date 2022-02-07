// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::io::Write;

use i_slint_compiler::object_tree;
use i_slint_compiler::parser::{syntax_nodes, SyntaxNode};

/// Remove colspan, rowspan, col and row for items not in a GridLayout
pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
) -> std::io::Result<bool> {
    if let Some(binding) = syntax_nodes::Binding::new(node.clone()) {
        match state.property_name.as_deref().unwrap_or_default() {
            "colspan" | "rowspan" | "row" | "col" => {
                if let Some(elem) =
                    find_parent_element(binding.into()).and_then(|x| find_parent_element(x.into()))
                {
                    let elem_name = elem
                        .QualifiedName()
                        .map(|qn| object_tree::QualifiedTypeName::from_node(qn).to_string())
                        .unwrap_or_default();
                    if !matches!(elem_name.as_str(), "GridLayout" | "Row") {
                        write!(file, "/* {} // REMOVED BY THE SYNTAX UPDATER */", **node)?;
                        return Ok(true);
                    }
                }
            }
            _ => {}
        }
    };
    Ok(false)
}

fn find_parent_element(mut node: SyntaxNode) -> Option<syntax_nodes::Element> {
    loop {
        node = node.parent()?;
        if let Some(elem) = syntax_nodes::Element::new(node.clone()) {
            break Some(elem);
        }
    }
}
