// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

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
            node.child_token(SyntaxKind::Identifier).map_or(false, |t| t.text() == "global");
        if !is_global {
            write!(file, "component ")?;
        }
        for n in node.children_with_tokens() {
            if n.kind() == SyntaxKind::ColonEqual {
                if !is_global {
                    let t = n.as_token().unwrap();
                    if t.prev_token().map_or(false, |t| t.kind() != SyntaxKind::Whitespace) {
                        write!(file, " ")?;
                    }
                    write!(file, "inherits")?;
                    if t.next_token().map_or(false, |t| t.kind() != SyntaxKind::Whitespace) {
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
