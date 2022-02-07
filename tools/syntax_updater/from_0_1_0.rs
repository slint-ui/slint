// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::io::Write;

use i_slint_compiler::parser::{syntax_nodes, SyntaxNode};

/// Rename `loop-count` to `iteration-count` in `PropertyAnimation`s
pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
) -> std::io::Result<bool> {
    if let Some(binding) = syntax_nodes::Binding::new(node.clone()) {
        let property_name = state.property_name.as_deref().unwrap_or_default();
        if (property_name == "loop-count" || property_name == "loop_count")
            && has_parent_anim(&binding.clone().into())
        {
            let text = node.text().to_string();
            let text = text.replace("loop-count", "iteration-count");
            let text = text.replace("loop_count", "iteration-count");
            file.write_all(text.as_bytes())?;
            return Ok(true);
        }
    };
    Ok(false)
}

fn has_parent_anim(node: &SyntaxNode) -> bool {
    let node = node.parent();
    if let Some(node) = node {
        syntax_nodes::PropertyAnimation::new(node).is_some()
    } else {
        false
    }
}
