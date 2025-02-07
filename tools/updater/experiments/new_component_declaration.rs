// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Cli;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    let kind = node.kind();
    if kind == SyntaxKind::Component && node.child_token(SyntaxKind::ColonEqual).is_some() {
        let is_global =
            node.child_token(SyntaxKind::Identifier).is_some_and(|t| t.text() == "global");
        if !is_global {
            write!(file, "component ")?;
        }
        for n in node.children_with_tokens() {
            if n.kind() == SyntaxKind::ColonEqual {
                if !is_global {
                    let t = n.as_token().unwrap();
                    if t.prev_token().is_some_and(|t| t.kind() != SyntaxKind::Whitespace) {
                        write!(file, " ")?;
                    }
                    write!(file, "inherits")?;
                    if t.next_token().is_some_and(|t| t.kind() != SyntaxKind::Whitespace) {
                        write!(file, " ")?;
                    }
                }
            } else {
                crate::visit_node_or_token(n, file, state, args)?;
            }
        }
        return Ok(true);
    } else if kind == SyntaxKind::StructDeclaration
        && node.child_token(SyntaxKind::ColonEqual).is_some()
    {
        for n in node.children_with_tokens() {
            if n.kind() == SyntaxKind::ColonEqual {
                // remove the ':=' in structs
            } else {
                crate::visit_node_or_token(n, file, state, args)?;
            }
        }
        return Ok(true);
    }
    Ok(false)
}
