// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_compiler::{
    expression_tree::Expression,
    langtype::Type,
    lookup::{LookupCtx, LookupObject},
    parser::{SyntaxKind, SyntaxNode},
};
use std::{io::Write, rc::Rc};

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
) -> std::io::Result<bool> {
    if node.kind() == SyntaxKind::QualifiedName
        && node.parent().map_or(false, |n| n.kind() == SyntaxKind::Expression)
    {
        let mut it = node
            .children_with_tokens()
            .filter(|n| n.kind() == SyntaxKind::Identifier)
            .filter_map(|n| n.into_token());

        let first = match it.next() {
            Some(first) => first,
            None => return Ok(false),
        };
        let first_str = i_slint_compiler::parser::normalize_identifier(first.text());
        if matches!(first_str.as_str(), "root" | "self" | "parent") {
            return Ok(false);
        }

        with_lookup_ctx(state, |ctx| -> std::io::Result<()> {
            ctx.current_token = Some(first.clone().into());
            let global_lookup = i_slint_compiler::lookup::global_lookup();
            if let Some(i_slint_compiler::lookup::LookupResult::Expression {
                expression: Expression::PropertyReference(nr) | Expression::CallbackReference(nr),
                ..
            }) = global_lookup.lookup(ctx, &first_str)
            {
                let element = nr.element();
                if state
                    .current_component
                    .as_ref()
                    .map_or(false, |c| Rc::ptr_eq(&element, &c.root_element))
                {
                    // We could decide to also prefix all root properties with root
                    //write!(file, "root.")?;
                } else if state.current_elem.as_ref().map_or(false, |e| Rc::ptr_eq(&element, e)) {
                    write!(file, "self.")?;
                }
            };
            Ok(())
        })
        .transpose()?;
        return Ok(false);
    }
    Ok(false)
}

fn with_lookup_ctx<R>(state: &crate::State, f: impl FnOnce(&mut LookupCtx) -> R) -> Option<R> {
    let mut build_diagnostics = Default::default();
    let tr = &state.current_doc.as_ref()?.local_registry;
    let mut lookup_context = LookupCtx::empty_context(tr, &mut build_diagnostics);

    let ty = state
        .current_elem
        .as_ref()
        .zip(state.property_name.as_ref())
        .map_or(Type::Invalid, |(e, n)| e.borrow().lookup_property(&n).property_type);

    let scope = state
        .current_component
        .as_ref()
        .map(|c| c.root_element.clone())
        .into_iter()
        .chain(state.current_elem.clone().into_iter())
        .collect::<Vec<_>>();

    lookup_context.property_name = state.property_name.as_ref().map(String::as_str);
    lookup_context.property_type = ty;
    lookup_context.component_scope = &scope;
    Some(f(&mut lookup_context))
}
