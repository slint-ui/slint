// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Cli;
use i_slint_compiler::langtype::Type;

use i_slint_compiler::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode};
use std::io::Write;

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
    _args: &Cli,
) -> std::io::Result<bool> {
    if let Some(s) = syntax_nodes::CallbackDeclaration::new(node.clone()) {
        if state.current_elem.as_ref().is_some_and(|e| e.borrow().is_legacy_syntax)
            && s.child_text(SyntaxKind::Identifier).as_deref() != Some("pure")
        {
            if s.ReturnType().is_some() {
                write!(file, "pure ")?;
            } else if let Some(twb) = s.TwoWayBinding() {
                let nr = super::lookup_changes::with_lookup_ctx(state, |lookup_ctx| {
                    lookup_ctx.property_type = Type::InferredCallback;
                    let r = i_slint_compiler::passes::resolving::resolve_two_way_binding(
                        twb, lookup_ctx,
                    );
                    r
                })
                .flatten();

                if let Some(nr) = nr {
                    let lk = nr.element().borrow().lookup_property(nr.name());
                    if lk.declared_pure == Some(true) {
                        write!(file, "pure ")?;
                    } else if let Type::Callback(callback) = lk.property_type {
                        if !matches!(callback.return_type, Type::Void) {
                            write!(file, "pure ")?;
                        }
                    }
                }
            }
        }
    } else if let Some(s) = syntax_nodes::Function::new(node.clone()) {
        if state.current_elem.as_ref().is_some_and(|e| e.borrow().is_legacy_syntax)
            && s.ReturnType().is_some()
        {
            let (mut pure, mut public) = (false, false);
            for t in s
                .children_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .filter(|t| t.kind() == SyntaxKind::Identifier)
            {
                match t.text() {
                    "pure" => pure = true,
                    "public" => public = true,
                    _ => (),
                }
            }
            if !pure && public {
                write!(file, "pure ")?;
            }
        }
    }
    Ok(false)
}
