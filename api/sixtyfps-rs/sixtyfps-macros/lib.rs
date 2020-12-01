/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

/*!

*NOTE*: This library is an internal crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead.

*/

extern crate proc_macro;
use proc_macro::{Spacing, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use sixtyfps_compilerlib::parser::SyntaxKind;
use sixtyfps_compilerlib::*;

/// Returns true if the two token are touching. For example the two token `foo`and `-` are touching if
/// it was written like so in the source code: `foo-` but not when written like so `foo -`
fn are_token_touching(token1: proc_macro::Span, token2: proc_macro::Span) -> bool {
    // There is no way with stable API to find out if the token are touching, so do it by
    // extracting the range from the debug representation of the span
    are_token_touching_impl(&format!("{:?}", token1), &format!("{:?}", token2))
}

fn are_token_touching_impl(token1_debug: &str, token2_debug: &str) -> bool {
    // The debug representation of a span look like this: "#0 bytes(6662789..6662794)"
    // we just have to find out if the first number of the range of second span
    // is the same as the second number of the first span
    let is_byte_char = |c: char| c.is_numeric() || c == ':';
    let not_is_byte_char = |c: char| !is_byte_char(c);
    let end_of_token1 = token1_debug
        .trim_end_matches(not_is_byte_char)
        .rsplit(not_is_byte_char)
        .next()
        .map(|x| x.trim_matches(':'));
    let begin_of_token2 = token2_debug
        .trim_end_matches(not_is_byte_char)
        .trim_end_matches(is_byte_char)
        .trim_end_matches(not_is_byte_char)
        .rsplit(not_is_byte_char)
        .next()
        .map(|x| x.trim_matches(':'));
    end_of_token1.zip(begin_of_token2).map(|(a, b)| a != "" && a == b).unwrap_or(false)
}

#[test]
fn are_token_touching_impl_test() {
    assert!(are_token_touching_impl("#0 bytes(6662788..6662789)", "#0 bytes(6662789..6662794)"));
    assert!(!are_token_touching_impl("#0 bytes(6662788..6662789)", "#0 bytes(6662790..6662794)"));
    assert!(!are_token_touching_impl("#0 bytes(6662789..6662794)", "#0 bytes(6662788..6662789)"));
    assert!(!are_token_touching_impl("#0 bytes(6662788..6662789)", "#0 bytes(662789..662794)"));
    assert!(are_token_touching_impl("#0 bytes(123..456)", "#0 bytes(456..789)"));

    // Alternative representation on nightly with a special flag
    assert!(are_token_touching_impl("/foo/bar.rs:12:7: 12:18", "/foo/bar.rs:12:18: 12:19"));
    assert!(are_token_touching_impl("/foo/bar.rs:2:7: 13:18", "/foo/bar.rs:13:18: 14:29"));
    assert!(!are_token_touching_impl("/foo/bar.rs:2:7: 13:18", "/foo/bar.rs:14:18: 14:29"));
    assert!(!are_token_touching_impl("/foo/bar.rs:2:7: 2:8", "/foo/bar.rs:2:18: 2:29"));

    // What happens if the representation change
    assert!(!are_token_touching_impl("hello", "hello"));
    assert!(!are_token_touching_impl("hello42", "hello42"));
}

fn fill_token_vec(stream: impl Iterator<Item = TokenTree>, vec: &mut Vec<parser::Token>) {
    let mut prev_spacing = Spacing::Alone;
    let mut prev_span = proc_macro::Span::call_site();
    for t in stream {
        let span = t.span();
        match t {
            TokenTree::Ident(i) => {
                if let Some(last) = vec.last_mut() {
                    if (last.kind == SyntaxKind::ColorLiteral && last.text.len() == 1)
                        || (last.kind == SyntaxKind::Identifier
                            && are_token_touching(prev_span, span))
                    {
                        last.text = format!("{}{}", last.text, i).into();
                        prev_span = span;
                        continue;
                    }
                }
                vec.push(parser::Token {
                    kind: SyntaxKind::Identifier,
                    text: i.to_string().into(),
                    span: Some(i.span()),
                    ..Default::default()
                });
            }
            TokenTree::Punct(p) => {
                let kind = match p.as_char() {
                    ':' => SyntaxKind::Colon,
                    '=' => {
                        if let Some(last) = vec.last_mut() {
                            let kt = match last.kind {
                                SyntaxKind::Star => Some((SyntaxKind::StarEqual, "*=")),
                                SyntaxKind::Colon => Some((SyntaxKind::ColonEqual, ":=")),
                                SyntaxKind::Plus => Some((SyntaxKind::PlusEqual, "+=")),
                                SyntaxKind::Minus => Some((SyntaxKind::MinusEqual, "-=")),
                                SyntaxKind::Div => Some((SyntaxKind::DivEqual, "/=")),
                                SyntaxKind::LAngle => Some((SyntaxKind::LessEqual, "<=")),
                                SyntaxKind::RAngle => Some((SyntaxKind::GreaterEqual, ">=")),
                                SyntaxKind::Equal => Some((SyntaxKind::EqualEqual, "==")),
                                SyntaxKind::Bang => Some((SyntaxKind::NotEqual, "!=")),
                                _ => None,
                            };
                            if let Some((k, t)) = kt {
                                if prev_spacing == Spacing::Joint {
                                    last.kind = k;
                                    last.text = t.into();
                                    continue;
                                }
                            }
                        }
                        SyntaxKind::Equal
                    }
                    ';' => SyntaxKind::Semicolon,
                    '!' => SyntaxKind::Bang,
                    '.' => SyntaxKind::Dot,
                    '+' => SyntaxKind::Plus,
                    '-' => {
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::Identifier
                                && are_token_touching(prev_span, p.span())
                            {
                                last.text = format!("{}-", last.text).into();
                                prev_span = span;
                                continue;
                            }
                        }
                        SyntaxKind::Minus
                    }
                    '*' => SyntaxKind::Star,
                    '/' => SyntaxKind::Div,
                    '<' => SyntaxKind::LAngle,
                    '>' => {
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::LessEqual && prev_spacing == Spacing::Joint
                            {
                                last.kind = SyntaxKind::DoubleArrow;
                                last.text = "<=>".into();
                                continue;
                            } else if last.kind == SyntaxKind::Equal
                                && prev_spacing == Spacing::Joint
                            {
                                last.kind = SyntaxKind::FatArrow;
                                last.text = "=>".into();
                                continue;
                            }
                        }
                        SyntaxKind::RAngle
                    }
                    '#' => SyntaxKind::ColorLiteral,
                    '?' => SyntaxKind::Question,
                    ',' => SyntaxKind::Comma,
                    '&' => {
                        // Since the '&' alone does not exist or cannot be part of any other token that &&
                        // just consider it as '&&' and skip the joint ones.  FIXME. do that properly
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::AndAnd && prev_spacing == Spacing::Joint {
                                continue;
                            }
                        }
                        SyntaxKind::AndAnd
                    }
                    '|' => {
                        // Since the '|' alone does not exist or cannot be part of any other token that ||
                        // just consider it as '||' and skip the joint ones.  FIXME. do that properly
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::OrOr && prev_spacing == Spacing::Joint {
                                continue;
                            }
                        }
                        SyntaxKind::OrOr
                    }
                    '%' => {
                        // % can only exist after number literal
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::NumberLiteral {
                                last.text = format!("{}%", last.text).into();
                                continue;
                            }
                        }
                        SyntaxKind::Error
                    }
                    '$' => SyntaxKind::Dollar,
                    _ => SyntaxKind::Error,
                };
                prev_spacing = p.spacing();
                vec.push(parser::Token {
                    kind,
                    text: p.to_string().into(),
                    span: Some(p.span()),
                    ..Default::default()
                });
            }
            TokenTree::Literal(l) => {
                let s = l.to_string();
                // Why can't the rust API give me the type of the literal
                let f = s.chars().next().unwrap();
                let kind = if f == '"' {
                    SyntaxKind::StringLiteral
                } else if f.is_digit(10) {
                    if let Some(last) = vec.last_mut() {
                        if (last.kind == SyntaxKind::ColorLiteral && last.text.len() == 1)
                            || (last.kind == SyntaxKind::Identifier
                                && are_token_touching(prev_span, span))
                        {
                            last.text = format!("{}{}", last.text, s).into();
                            prev_span = span;
                            continue;
                        }
                    }
                    SyntaxKind::NumberLiteral
                } else {
                    SyntaxKind::Error
                };
                vec.push(parser::Token {
                    kind,
                    text: s.into(),
                    span: Some(l.span()),
                    ..Default::default()
                });
            }
            TokenTree::Group(g) => {
                use proc_macro::Delimiter::*;
                use SyntaxKind::*;
                let (l, r, sl, sr) = match g.delimiter() {
                    Parenthesis => (LParent, RParent, "(", ")"),
                    Brace => (LBrace, RBrace, "{", "}"),
                    Bracket => (LBracket, RBracket, "[", "]"),
                    None => todo!(),
                };
                vec.push(parser::Token {
                    kind: l,
                    text: sl.into(),
                    span: Some(g.span()), // span_open is not stable
                    ..Default::default()
                });
                fill_token_vec(g.stream().into_iter(), vec);
                vec.push(parser::Token {
                    kind: r,
                    text: sr.into(),
                    span: Some(g.span()), // span_clone is not stable
                    ..Default::default()
                });
            }
        }
        prev_span = span;
    }
}

fn extract_include_paths(
    mut stream: proc_macro::token_stream::IntoIter,
) -> (impl Iterator<Item = TokenTree>, Vec<std::path::PathBuf>) {
    let mut include_paths = Vec::new();

    let mut remaining_stream = stream.clone();

    // parse #[include_path="../foo/bar/baz"]
    // ### support multiple occurrences
    match (stream.next(), stream.next()) {
        (Some(TokenTree::Punct(p)), Some(TokenTree::Group(group)))
            if p.as_char() == '#' && group.delimiter() == proc_macro::Delimiter::Bracket =>
        {
            let mut attr_stream = group.stream().into_iter();
            match (attr_stream.next(), attr_stream.next(), attr_stream.next()) {
                (
                    Some(TokenTree::Ident(include_ident)),
                    Some(TokenTree::Punct(equal_punct)),
                    Some(TokenTree::Literal(path)),
                ) if include_ident.to_string() == "include_path"
                    && equal_punct.as_char() == '=' =>
                {
                    let path_with_quotes = path.to_string();
                    let path_with_quotes_stripped = if path_with_quotes.starts_with("r") {
                        let hash_removed = path_with_quotes[1..].trim_matches('#');
                        hash_removed.strip_prefix("\"").unwrap().strip_suffix("\"").unwrap()
                    } else {
                        // FIXME: unescape
                        path_with_quotes.trim_matches('\"')
                    };
                    include_paths.push(path_with_quotes_stripped.into());
                    remaining_stream = stream;
                }
                _ => (),
            }
        }
        _ => (),
    }

    (remaining_stream, include_paths)
}

/// This macro allows you to use the `.60` design markup language inline in Rust code. Within the braces of the macro
/// you can use place `.60` code and the named exported components will be available for instantiation.
///
/// [The documentation of the `sixtyfps`](./index.html) crate contains more information about the language specification and
/// how to use the generated code.
#[proc_macro]
pub fn sixtyfps(stream: TokenStream) -> TokenStream {
    let token_iter = stream.into_iter();

    let (token_iter, include_paths) = extract_include_paths(token_iter);

    let mut tokens = vec![];
    fill_token_vec(token_iter, &mut tokens);

    let (syntax_node, mut diag) = parser::parse_tokens(tokens.clone());
    if diag.has_error() {
        diag.map_offsets_to_span(&tokens);
        return diag.into_token_stream().into();
    }

    let source_file = if let Some(cargo_manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let mut path: std::path::PathBuf = cargo_manifest.into();
        path.push("Cargo.toml");
        diag.current_path = std::rc::Rc::new(path);
        Some(diag.current_path.clone())
    } else {
        None
    };

    let syntax_node = parser::SyntaxNodeWithSourceFile { node: syntax_node, source_file };

    //println!("{:#?}", syntax_node);
    let compiler_config = CompilerConfiguration { include_paths, ..Default::default() };
    let (root_component, mut diag) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));
    //println!("{:#?}", tree);
    if diag.has_error() {
        return report_diagnostics(diag, &tokens);
    }

    let mut result = generator::rust::generate(&root_component, &mut diag);

    // Make sure to recompile if any of the external files changes
    let reload = diag
        .files()
        .filter(|path| path.is_absolute() && !path.ends_with("Cargo.toml"))
        .filter_map(|p| p.to_str())
        .map(|p| quote! {const _ : &'static [u8] = ::core::include_bytes!(#p);});
    result.as_mut().map(|x| {
        x.extend(reload);
        x.extend(quote! {const _ : Option<&'static str> = ::core::option_env!("SIXTYFPS_STYLE");});
    });

    let diags = report_diagnostics(diag, &tokens);
    result.map_or(diags, |r| r.into())
}

fn report_diagnostics(
    diag: diagnostics::BuildDiagnostics,
    span_map: &[parser::Token],
) -> TokenStream {
    let mut result = TokenStream::new();
    let mut needs_error = diag.has_error();
    for mut file_diag in diag.into_iter() {
        if file_diag.source.is_none() {
            file_diag.map_offsets_to_span(span_map);
            needs_error &= !file_diag.has_error();
            result.extend(TokenStream::from(file_diag.into_token_stream()))
        } else {
            file_diag.print();
        }
    }
    if needs_error {
        result.extend(TokenStream::from(quote!(compile_error! { "Error occured" })))
    }
    result
}
