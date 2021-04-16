/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use super::{lookup_current_element_type, DocumentCache};
use lsp_types::{CompletionItem, CompletionItemKind};
use sixtyfps_compilerlib::diagnostics::Spanned;
use sixtyfps_compilerlib::expression_tree::Expression;
use sixtyfps_compilerlib::langtype::Type;
use sixtyfps_compilerlib::lookup::{LookupCtx, LookupObject};
use sixtyfps_compilerlib::object_tree;
use sixtyfps_compilerlib::parser::{syntax_nodes, SyntaxKind, SyntaxToken};

pub(crate) fn completion_at(
    document_cache: &DocumentCache,
    token: SyntaxToken,
) -> Option<Vec<CompletionItem>> {
    let node = token.parent();
    if let Some(element) = syntax_nodes::Element::new(node.clone()) {
        return resolve_element_scope(element, document_cache);
    } else if let Some(n) = syntax_nodes::Binding::new(node.clone()) {
        if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let all = resolve_element_scope(syntax_nodes::Element::new(n.parent()?)?, document_cache)?;
        return Some(
            all.into_iter().filter(|ce| ce.kind == Some(CompletionItemKind::Property)).collect(),
        );
    } else if let Some(n) = syntax_nodes::TwoWayBinding::new(node.clone()) {
        if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let all = resolve_element_scope(syntax_nodes::Element::new(n.parent()?)?, document_cache)?;
        return Some(
            all.into_iter().filter(|ce| ce.kind == Some(CompletionItemKind::Property)).collect(),
        );
    } else if let Some(n) = syntax_nodes::CallbackConnection::new(node.clone()) {
        if token.kind() != SyntaxKind::Identifier {
            return None;
        }
        let all = resolve_element_scope(syntax_nodes::Element::new(n.parent()?)?, document_cache)?;
        return Some(
            all.into_iter().filter(|ce| ce.kind == Some(CompletionItemKind::Method)).collect(),
        );
    } else if matches!(
        node.kind(),
        SyntaxKind::BindingExpression
            | SyntaxKind::CodeBlock
            | SyntaxKind::ReturnStatement
            | SyntaxKind::Expression
            | SyntaxKind::FunctionCallExpression
            | SyntaxKind::SelfAssignment
            | SyntaxKind::ConditionalExpression
            | SyntaxKind::BinaryExpression
            | SyntaxKind::UnaryOpExpression
            | SyntaxKind::Array
    ) {
        // find context
        let mut n = node.parent()?;
        let (element, prop_name) = loop {
            if let Some(decl) = syntax_nodes::PropertyDeclaration::new(n.clone()) {
                let prop_name =
                    sixtyfps_compilerlib::parser::identifier_text(&decl.DeclaredIdentifier())?;
                let element = syntax_nodes::Element::new(n.parent()?)?;
                break (element, prop_name.to_string());
            }
            match n.kind() {
                SyntaxKind::Binding
                | SyntaxKind::TwoWayBinding
                // FIXME: arguments of the callback
                | SyntaxKind::CallbackConnection => {
                    let prop_name = sixtyfps_compilerlib::parser::identifier_text(&n)?;
                    let element = syntax_nodes::Element::new(n.parent()?)?;
                    break (element, prop_name.to_string());
                }
                SyntaxKind::ConditionalElement | SyntaxKind::RepeatedElement => {
                    let element = syntax_nodes::Element::new(n.parent()?)?;
                    break (element, "$model".to_string());
                }
                SyntaxKind::Element => {
                    // oops: missed it
                    let element = syntax_nodes::Element::new(n)?;
                    break (element, String::new());
                }
                _ => n = n.parent()?,

            }
        };
        return resolve_expression_scope(element, &prop_name, document_cache);
    }
    return None;
}

fn resolve_element_scope(
    element: syntax_nodes::Element,
    document_cache: &DocumentCache,
) -> Option<Vec<CompletionItem>> {
    let global_tr = document_cache.documents.global_type_registry.borrow();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.documents.get_document(sf.path()))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);
    let element_type = lookup_current_element_type((*element).clone(), tr).unwrap_or_default();
    Some(
        element_type
            .property_list()
            .into_iter()
            .map(|(k, t)| {
                let mut c = CompletionItem::new_simple(k, t.to_string());
                c.kind = Some(if matches!(t, Type::Callback { .. }) {
                    CompletionItemKind::Method
                } else {
                    CompletionItemKind::Property
                });
                c
            })
            .chain(element.PropertyDeclaration().map(|pr| {
                let mut c = CompletionItem::new_simple(
                    sixtyfps_compilerlib::parser::identifier_text(&pr.DeclaredIdentifier())
                        .unwrap_or_default(),
                    pr.Type().text().into(),
                );
                c.kind = Some(CompletionItemKind::Property);
                c
            }))
            .chain(element.CallbackDeclaration().map(|cd| {
                let mut c = CompletionItem::new_simple(
                    sixtyfps_compilerlib::parser::identifier_text(&cd.DeclaredIdentifier())
                        .unwrap_or_default(),
                    "callback".into(),
                );
                c.kind = Some(CompletionItemKind::Method);
                c
            }))
            .chain(tr.all_types().into_iter().filter_map(|(k, t)| {
                if !matches!(t, Type::Component(_) | Type::Builtin(_)) {
                    return None;
                } else {
                    let mut c = CompletionItem::new_simple(k, "element".into());
                    c.kind = Some(CompletionItemKind::Class);
                    Some(c)
                }
            }))
            .collect(),
    )
}

fn resolve_expression_scope(
    element: syntax_nodes::Element,
    prop_name: &str,
    document_cache: &DocumentCache,
) -> Option<Vec<CompletionItem>> {
    let global_tr = document_cache.documents.global_type_registry.borrow();
    let tr = element
        .source_file()
        .and_then(|sf| document_cache.documents.get_document(sf.path()))
        .map(|doc| &doc.local_registry)
        .unwrap_or(&global_tr);
    let ty = element
        .PropertyDeclaration()
        .find_map(|p| {
            (sixtyfps_compilerlib::parser::identifier_text(&p.DeclaredIdentifier())? == prop_name)
                .then(|| p)
        })
        .map(|p| object_tree::type_from_node(p.Type(), &mut Default::default(), tr));
    let ty = ty.or_else(|| {
        lookup_current_element_type((*element).clone(), tr)
            .map(|el_ty| el_ty.lookup_property(&prop_name).property_type)
    });

    // FIXME: we need also to fill in the repeated element
    let component = {
        let mut n = element.parent()?;
        loop {
            if let Some(component) = syntax_nodes::Component::new(n.clone()) {
                break component;
            }
            n = n.parent()?;
        }
    };
    let component_name =
        sixtyfps_compilerlib::parser::identifier_text(&component.DeclaredIdentifier())?;
    let component = tr.lookup(&component_name);
    let scope =
        if let Type::Component(c) = component { vec![c.root_element.clone()] } else { Vec::new() };

    let mut build_diagnostics = Default::default();
    let mut lookup_context = LookupCtx::empty_context(tr, &mut build_diagnostics);
    lookup_context.property_name = Some(&prop_name);
    lookup_context.property_type = ty.unwrap_or_default();
    lookup_context.component_scope = &scope;

    let mut r = Vec::new();
    let global = sixtyfps_compilerlib::lookup::global_lookup();
    global.for_each_entry(&lookup_context, &mut |str, expr| -> Option<()> {
        let mut c = CompletionItem::new_simple(str.to_string(), expr.ty().to_string());
        c.kind = match expr {
            Expression::BoolLiteral(_) => Some(CompletionItemKind::Constant),
            Expression::CallbackReference(_) => Some(CompletionItemKind::Method),
            Expression::PropertyReference(_) => Some(CompletionItemKind::Property),
            Expression::BuiltinFunctionReference(_) => Some(CompletionItemKind::Function),
            Expression::BuiltinMacroReference(..) => Some(CompletionItemKind::Function),
            Expression::ElementReference(_) => Some(CompletionItemKind::Class),
            Expression::RepeaterIndexReference { .. } => Some(CompletionItemKind::Variable),
            Expression::RepeaterModelReference { .. } => Some(CompletionItemKind::Variable),
            Expression::FunctionParameterReference { .. } => Some(CompletionItemKind::Variable),
            Expression::Cast { .. } => Some(CompletionItemKind::Constant),
            Expression::EasingCurve(_) => Some(CompletionItemKind::Constant),
            Expression::EnumerationValue(ev) => Some(if ev.value == usize::MAX {
                CompletionItemKind::Enum
            } else {
                CompletionItemKind::EnumMember
            }),
            _ => None,
        };
        r.push(c);
        None
    });
    Some(r)
}
