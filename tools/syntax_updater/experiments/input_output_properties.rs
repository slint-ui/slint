// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::Cli;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    _state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    debug_assert!(args.input_output_properties);
    let kind = node.kind();
    if kind == SyntaxKind::PropertyDeclaration
        && node
            .parent()
            .and_then(|n| n.parent())
            .map_or(false, |n| n.kind() == SyntaxKind::Component)
    {
        // check that the first identifier is "property" as opposed to an already converted "inout" token
        if node.child_token(SyntaxKind::Identifier).map_or(false, |t| t.text() == "property") {
            // Consider that all property are inout, because we don't do enough analysis in the syntax_updater to know
            // if they should be private
            write!(file, "inout ")?;
        }
    }
    Ok(false)
}
