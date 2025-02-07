// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Cli;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    _state: &mut crate::State,
    _args: &Cli,
) -> std::io::Result<bool> {
    let kind = node.kind();
    if kind == SyntaxKind::QualifiedName
        && node.parent().is_some_and(|n| n.kind() == SyntaxKind::Expression)
    {
        let q = i_slint_compiler::object_tree::QualifiedTypeName::from_node(node.clone().into())
            .to_string();
        if q == "PointerEventButton.none" {
            for t in node.children_with_tokens() {
                let text = t.into_token().unwrap().to_string();
                write!(file, "{}", if text == "none" { "other" } else { &text })?;
            }
            return Ok(true);
        } else if q.starts_with("Keys.") {
            for t in node.children_with_tokens() {
                let text = t.into_token().unwrap().to_string();
                write!(file, "{}", if text == "Keys" { "Key" } else { &text })?;
            }
            return Ok(true);
        }
    }

    Ok(false)
}
