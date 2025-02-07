// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Cli;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    if let Some(s) = syntax_nodes::State::new(node.clone()) {
        if let Some(element) = state.current_elem.clone() {
            let state_id = i_slint_compiler::parser::normalize_identifier(
                s.DeclaredIdentifier().to_string().as_str(),
            );
            if !element.borrow().transitions.is_empty() {
                for c in node.children_with_tokens() {
                    if c.kind() == SyntaxKind::RBrace {
                        let whitespace = c
                            .as_token()
                            .and_then(|t| t.prev_token())
                            .filter(|t| t.kind() == SyntaxKind::Whitespace);
                        for t in element.borrow().transitions.clone() {
                            if t.state_id == state_id
                                && t.node
                                    .parent()
                                    .is_some_and(|p| p.kind() == SyntaxKind::Transitions)
                            {
                                for c in t.node.children_with_tokens() {
                                    if !matches!(
                                        c.kind(),
                                        SyntaxKind::DeclaredIdentifier | SyntaxKind::Colon,
                                    ) {
                                        crate::visit_node_or_token(c, file, state, args)?;
                                    }
                                }
                                if let Some(ws) = whitespace.as_ref() {
                                    write!(file, "{ws}")?
                                }
                            }
                        }
                    }
                    crate::visit_node_or_token(c, file, state, args)?;
                }
                return Ok(true);
            }
        }
    }
    if node.kind() == SyntaxKind::Transitions {
        if let Some(element) = state.current_elem.clone() {
            if !element.borrow().transitions.is_empty() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
