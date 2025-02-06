// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::DocumentCache;
use i_slint_compiler::expression_tree::Callable;
use i_slint_compiler::langtype::{Function, Type};
use i_slint_compiler::lookup::{LookupObject as _, LookupResult, LookupResultCallable};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken};
use lsp_types::{ParameterInformation, ParameterLabel, SignatureHelp, SignatureInformation};

pub(crate) fn get_signature_help(
    document_cache: &mut DocumentCache,
    token: SyntaxToken,
) -> Option<SignatureHelp> {
    let pos = token.text_range().start();
    let mut node = token.parent();
    let mut result = vec![];

    loop {
        if let Some(node) = syntax_nodes::FunctionCallExpression::new(node.clone()) {
            if let Some(f) = node.Expression().next() {
                let mut active_parameter = None;
                for t in node.children_with_tokens() {
                    if t.text_range().start() > pos {
                        break;
                    }
                    match t.kind() {
                        SyntaxKind::LParent => {
                            active_parameter = Some(0);
                        }
                        SyntaxKind::Comma => {
                            if let Some(active_parameter) = active_parameter.as_mut() {
                                *active_parameter += 1;
                            }
                        }
                        SyntaxKind::RParent => {
                            active_parameter = None;
                        }
                        _ => (),
                    };
                }
                if let Some(si) = signature_info(document_cache, f.into(), active_parameter) {
                    result.push(si);
                }
            }
        }

        match node.parent() {
            Some(n) => node = n,
            None => return make_result(result),
        }
    }
}

fn make_result(result: Vec<SignatureInformation>) -> Option<SignatureHelp> {
    if result.is_empty() {
        return None;
    }
    Some(SignatureHelp { signatures: result, active_signature: None, active_parameter: None })
}

fn signature_info(
    document_cache: &mut DocumentCache,
    func_expr: SyntaxNode,
    active_parameter: Option<u32>,
) -> Option<SignatureInformation> {
    if let Some(sub_expr) = func_expr.child_node(SyntaxKind::Expression) {
        return signature_info(document_cache, sub_expr, active_parameter);
    }
    let qn = func_expr.child_node(SyntaxKind::QualifiedName)?;
    let lr = crate::util::with_lookup_ctx(document_cache, func_expr, |ctx| {
        let mut it = qn
            .children_with_tokens()
            .filter_map(|t| t.into_token())
            .filter(|t| t.kind() == SyntaxKind::Identifier);
        let first_tok = it.next()?;
        let mut expr_it = i_slint_compiler::lookup::global_lookup()
            .lookup(ctx, &i_slint_compiler::parser::normalize_identifier(first_tok.text()))?;
        for cur_tok in it {
            expr_it = expr_it
                .lookup(ctx, &i_slint_compiler::parser::normalize_identifier(cur_tok.text()))?;
        }
        Some(expr_it)
    })?;
    let LookupResult::Callable(callable) = lr? else { return None };

    match callable {
        LookupResultCallable::Callable(Callable::Callback(nr))
        | LookupResultCallable::Callable(Callable::Function(nr)) => {
            signature_from_nr(nr, active_parameter)
        }
        LookupResultCallable::Callable(Callable::Builtin(b)) => {
            Some(signature_from_function_ty(&format!("{b:?}"), &b.ty(), 0, active_parameter))
        }
        LookupResultCallable::Macro(b) => {
            Some(make_signature_info(&format!("{b:?}"), vec!["...".into()], active_parameter))
        }
        LookupResultCallable::MemberFunction { member, .. } => match *member {
            LookupResultCallable::Callable(Callable::Builtin(b)) => {
                Some(signature_from_function_ty(&format!("{b:?}"), &b.ty(), 1, active_parameter))
            }
            LookupResultCallable::Macro(b) => {
                Some(make_signature_info(&format!("{b:?}"), vec!["...".into()], active_parameter))
            }
            _ => None,
        },
    }
}

fn signature_from_nr(
    nr: NamedReference,
    active_parameter: Option<u32>,
) -> Option<SignatureInformation> {
    match nr.ty() {
        Type::Function(f) | Type::Callback(f) => {
            Some(signature_from_function_ty(nr.name(), &f, 0, active_parameter))
        }
        _ => None,
    }
}

fn signature_from_function_ty(
    name: &str,
    f: &Function,
    skip: usize,
    active_parameter: Option<u32>,
) -> SignatureInformation {
    make_signature_info(
        name,
        f.args
            .iter()
            .zip(f.arg_names.iter().chain(std::iter::repeat(&Default::default())))
            .skip(skip)
            .filter(|(x, _)| *x != &Type::ElementReference)
            .map(
                |(ty, name)| {
                    if !name.is_empty() {
                        format!("{name}: {ty}")
                    } else {
                        ty.to_string()
                    }
                },
            )
            .collect(),
        active_parameter,
    )
}

fn make_signature_info(
    name: &str,
    args: Vec<String>,
    active_parameter: Option<u32>,
) -> SignatureInformation {
    SignatureInformation {
        label: format!("{}({})", name, args.join(", ")),
        documentation: None,
        parameters: Some(
            args.into_iter()
                .map(|x| ParameterInformation {
                    label: ParameterLabel::Simple(x),
                    documentation: None,
                })
                .collect(),
        ),
        active_parameter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Given a source text containing the unicode emoji `🔺`, the emoji will be removed and then an signature help request will be done as if the cursor was there
    fn query_signature_help(file: &str) -> Option<SignatureHelp> {
        const CURSOR_EMOJI: char = '🔺';
        let offset = (file.find(CURSOR_EMOJI).unwrap() as u32).into();
        let source = file.replace(CURSOR_EMOJI, "");
        let (mut dc, uri, _) = crate::language::test::loaded_document_cache(source);

        let doc = dc.get_document(&uri).unwrap();
        let token = crate::language::token_at_offset(doc.node.as_ref().unwrap(), offset)?;
        get_signature_help(&mut dc, token)
    }

    #[test]
    fn builtin_function_and_member_fn() {
        let source = r#"
import { StandardTableView } from "std-widgets.slint";
export component Abc {
    table := StandardTableView {}
    function do_something(a: int, b: int) -> int {
        table.accessible-action-set-value(a.sqrt(🔺
        45
    }
}
    "#;
        let sh = query_signature_help(source).unwrap();
        assert_eq!(
            sh,
            SignatureHelp {
                signatures: vec![
                    SignatureInformation {
                        label: "Sqrt()".into(),
                        documentation: None,
                        parameters: Some(vec![]),
                        active_parameter: Some(0),
                    },
                    make_signature_info(
                        "accessible-action-set-value",
                        vec!["string".into()],
                        Some(0)
                    ),
                ],
                active_signature: None,
                active_parameter: None
            }
        );
    }

    #[test]
    fn arg_pos() {
        let source = r#"
export component Abc {
    function abc(a: {hi: string, ho: int}, b: int, c  :  string) -> int { b }
    function do_something(a: int, b: int) -> int {
        pow(42, abc({hi: "hello", ho: int}, a.pow(🔺
    }
}
    "#;
        let sh = query_signature_help(source).unwrap();
        assert_eq!(
            sh,
            SignatureHelp {
                signatures: vec![
                    make_signature_info("Pow", vec!["float".into()], Some(0),),
                    make_signature_info(
                        "abc",
                        vec![
                            "a: { hi: string,ho: int,}".into(),
                            "b: int".into(),
                            "c: string".into()
                        ],
                        Some(1)
                    ),
                    make_signature_info("Pow", vec!["float".into(), "float".into()], Some(1)),
                ],
                active_signature: None,
                active_parameter: None
            }
        );
    }

    #[test]
    fn callback_with_names() {
        let source = r#"
        import { StandardTableView } from "std-widgets.slint";
        export component Abc {
            table := StandardTableView {}
            function do_something(a: int, b: int) -> int {
                table.current_row_changed(🔺)
            }
        }

    "#;
        let sh = query_signature_help(source).unwrap();
        assert_eq!(
            sh,
            SignatureHelp {
                signatures: vec![make_signature_info(
                    "current-row-changed",
                    vec!["current-row: int".into()],
                    Some(0),
                )],
                active_signature: None,
                active_parameter: None
            }
        );
    }

    #[test]
    fn macros() {
        let source = r#"
        export component Abc {
            function do_something(a: int, b: int) -> int {
                debug(0, 1, a.mod(b.clamp(0,🔺 )), 3, 4)
            }
        }

    "#;
        let sh = query_signature_help(source).unwrap();
        assert_eq!(
            sh,
            SignatureHelp {
                signatures: vec![
                    make_signature_info("Clamp", vec!["...".into()], Some(1)),
                    make_signature_info("Mod", vec!["...".into()], Some(0)),
                    make_signature_info("Debug", vec!["...".into()], Some(2)),
                ],
                active_signature: None,
                active_parameter: None
            }
        );
    }
}
