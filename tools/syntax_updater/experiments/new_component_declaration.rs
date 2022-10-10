// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::Cli;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    debug_assert!(args.new_component_declaration);
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
                    write!(file, " inherits ")?;
                }
            } else {
                crate::visit_node_or_token(n, file, state, args)?;
            }
        }
        return Ok(true);
    }
    Ok(false)
}
