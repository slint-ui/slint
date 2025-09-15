// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore punct

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

extern crate proc_macro;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::SyntaxKind;
use i_slint_compiler::*;
use proc_macro::{Spacing, TokenStream, TokenTree};
use quote::quote;
use std::path::PathBuf;

/// Returns true if the two token are touching. For example the two token `foo`and `-` are touching if
/// it was written like so in the source code: `foo-` but not when written like so `foo -`
///
/// Returns None if we couldn't detect whether they are touching  (eg, our heuristics don't work with rust-analyzer)
#[rustversion::since(1.88)]
fn are_token_touching(token1: proc_macro::Span, token2: proc_macro::Span) -> Option<bool> {
    let t1 = token1.end();
    let t2 = token2.start();
    let t1_column = t1.column();
    if t1_column == 1 && t1.line() == 1 && t2.end().line() == 1 && t2.end().column() == 1 {
        // If everything is 1, this means that Span::line and Span::column are not working properly
        // (eg, rust-analyzer)
        return None;
    }
    Some(t1.line() == t2.line() && t1_column == t2.column())
}
#[rustversion::before(1.88)]
fn are_token_touching(token1: proc_macro::Span, token2: proc_macro::Span) -> Option<bool> {
    // There is no way with stable API to find out if the token are touching, so do it by
    // extracting the range from the debug representation of the span
    are_token_touching_impl(&format!("{token1:?}"), &format!("{token2:?}"))
}

#[rustversion::before(1.88)]
fn are_token_touching_impl(token1_debug: &str, token2_debug: &str) -> Option<bool> {
    // The debug representation of a span look like this: "#0 bytes(6662789..6662794)"
    // we just have to find out if the first number of the range of second span
    // is the same as the second number of the first span
    let is_byte_char = |c: char| c.is_numeric() || c == ':';
    let not_is_byte_char = |c: char| !is_byte_char(c);
    let end_of_token1 = token1_debug
        .trim_end_matches(not_is_byte_char)
        .rsplit(not_is_byte_char)
        .next()?
        .trim_matches(':');
    let begin_of_token2 = token2_debug
        .trim_end_matches(not_is_byte_char)
        .strip_suffix(is_byte_char)?
        .trim_end_matches(is_byte_char)
        .trim_end_matches(not_is_byte_char)
        .rsplit(not_is_byte_char)
        .next()?
        .trim_matches(':');
    (!begin_of_token2.is_empty()).then_some(end_of_token1 == begin_of_token2)
}

#[rustversion::before(1.88)]
#[test]
fn are_token_touching_impl_test() {
    assert!(are_token_touching_impl("#0 bytes(6662788..6662789)", "#0 bytes(6662789..6662794)")
        .unwrap());
    assert!(!are_token_touching_impl("#0 bytes(6662788..6662789)", "#0 bytes(6662790..6662794)")
        .unwrap());
    assert!(!are_token_touching_impl("#0 bytes(6662789..6662794)", "#0 bytes(6662788..6662789)")
        .unwrap());
    assert!(
        !are_token_touching_impl("#0 bytes(6662788..6662789)", "#0 bytes(662789..662794)").unwrap()
    );
    assert!(are_token_touching_impl("#0 bytes(123..456)", "#0 bytes(456..789)").unwrap());

    // Alternative representation on nightly with a special flag
    assert!(are_token_touching_impl("/foo/bar.rs:12:7: 12:18", "/foo/bar.rs:12:18: 12:19").unwrap());
    assert!(are_token_touching_impl("/foo/bar.rs:2:7: 13:18", "/foo/bar.rs:13:18: 14:29").unwrap());
    assert!(!are_token_touching_impl("/foo/bar.rs:2:7: 13:18", "/foo/bar.rs:14:18: 14:29").unwrap());
    assert!(!are_token_touching_impl("/foo/bar.rs:2:7: 2:8", "/foo/bar.rs:2:18: 2:29").unwrap());

    // What happens if the representation change
    assert!(are_token_touching_impl("hello", "hello").is_none());
    assert!(are_token_touching_impl("hello42", "hello42").is_none());

    // rust-analyzer just has indices that means nothing
    assert!(are_token_touching_impl("55", "56").is_none());
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
                            && are_token_touching(prev_span, span)
                                .unwrap_or_else(|| last.text.ends_with('-')))
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
                    '.' => {
                        // `4..log` is lexed as `4 . . log` in rust, but should be `4. . log` in slint
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::NumberLiteral
                                && are_token_touching(prev_span, p.span()).unwrap_or(false)
                                && !last.text.contains('.')
                                && !last.text.ends_with(char::is_alphabetic)
                            {
                                last.text = format!("{}.", last.text).into();
                                prev_span = span;
                                continue;
                            }
                        }
                        SyntaxKind::Dot
                    }
                    '+' => SyntaxKind::Plus,
                    '-' => {
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::Identifier
                                && are_token_touching(prev_span, p.span()).unwrap_or(true)
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
                            } else if last.kind == SyntaxKind::Minus
                                && prev_spacing == Spacing::Joint
                            {
                                last.kind = SyntaxKind::Arrow;
                                last.text = "->".into();
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
                        // just consider it as '||' and skip the joint ones.
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::Pipe && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::OrOr;
                                continue;
                            }
                        }
                        SyntaxKind::Pipe
                    }
                    '%' => {
                        // handle % as a unit
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::NumberLiteral {
                                last.text = format!("{}%", last.text).into();
                                continue;
                            }
                        }
                        SyntaxKind::Percent
                    }
                    '$' => SyntaxKind::Dollar,
                    '@' => SyntaxKind::At,
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
                } else if f.is_ascii_digit() {
                    if let Some(last) = vec.last_mut() {
                        if (last.kind == SyntaxKind::ColorLiteral && last.text.len() == 1)
                            || (last.kind == SyntaxKind::Identifier
                                && are_token_touching(prev_span, span)
                                    .unwrap_or_else(|| last.text.ends_with('-')))
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

fn extract_path(literal: proc_macro::Literal) -> std::path::PathBuf {
    let path_with_quotes = literal.to_string();
    let path_with_quotes_stripped = if let Some(p) = path_with_quotes.strip_prefix('r') {
        let hash_removed = p.trim_matches('#');
        hash_removed.strip_prefix('\"').unwrap().strip_suffix('\"').unwrap()
    } else {
        // FIXME: unescape
        path_with_quotes.trim_matches('\"')
    };
    path_with_quotes_stripped.into()
}

fn extract_compiler_config(
    mut stream: proc_macro::token_stream::IntoIter,
    compiler_config: &mut CompilerConfiguration,
) -> impl Iterator<Item = TokenTree> {
    let mut remaining_stream;
    loop {
        remaining_stream = stream.clone();
        match (stream.next(), stream.next()) {
            (Some(TokenTree::Punct(p)), Some(TokenTree::Group(group)))
                if p.as_char() == '#' && group.delimiter() == proc_macro::Delimiter::Bracket =>
            {
                let mut attr_stream = group.stream().into_iter();
                match attr_stream.next() {
                    Some(TokenTree::Ident(include_ident))
                        if include_ident.to_string() == "include_path" =>
                    {
                        match (attr_stream.next(), attr_stream.next()) {
                            (
                                Some(TokenTree::Punct(equal_punct)),
                                Some(TokenTree::Literal(path)),
                            ) if equal_punct.as_char() == '=' => {
                                compiler_config.include_paths.push(extract_path(path));
                            }
                            _ => break,
                        }
                    }
                    Some(TokenTree::Ident(library_ident))
                        if library_ident.to_string() == "library_path" =>
                    {
                        match (attr_stream.next(), attr_stream.next(), attr_stream.next()) {
                            (
                                Some(TokenTree::Group(group)),
                                Some(TokenTree::Punct(equal_punct)),
                                Some(TokenTree::Literal(path)),
                            ) if group.delimiter() == proc_macro::Delimiter::Parenthesis
                                && equal_punct.as_char() == '=' =>
                            {
                                let library_name = group.stream().into_iter().next().unwrap();
                                compiler_config
                                    .library_paths
                                    .insert(library_name.to_string(), extract_path(path));
                            }
                            _ => break,
                        }
                    }
                    Some(TokenTree::Ident(style_ident)) if style_ident.to_string() == "style" => {
                        match (attr_stream.next(), attr_stream.next()) {
                            (
                                Some(TokenTree::Punct(equal_punct)),
                                Some(TokenTree::Literal(requested_style)),
                            ) if equal_punct.as_char() == '=' => {
                                compiler_config.style = requested_style
                                    .to_string()
                                    .strip_prefix('\"')
                                    .unwrap()
                                    .strip_suffix('\"')
                                    .unwrap()
                                    .to_string()
                                    .into();
                            }
                            _ => break,
                        }
                    }
                    _ => break,
                }
            }
            _ => break,
        }
    }
    remaining_stream
}

/// This macro allows you to use the Slint design markup language inline in Rust code. Within the braces of the macro
/// you can use place Slint code and the named exported components will be available for instantiation.
///
/// For the documentation about the syntax of the language, see
#[doc = concat!("[The Slint Language Documentation](https://slint.dev/releases/", env!("CARGO_PKG_VERSION"), "/docs/slint)")]
///
/// When Rust 1.88 or later is used, the paths for loading images with `@image-url` and importing `.slint` files
/// are relative to the `.rs` file that contains the macro.
/// For compatibility with older rust version, the files are also searched in the manifest directory that contains `Cargo.toml`.
///
/// ### Limitations
///
/// Within `.slint` files, you can interpolate string literals using `\{...}` syntax.
/// This is not possible in this macro as this wouldn't parse as a Rust string.
#[proc_macro]
pub fn slint(stream: TokenStream) -> TokenStream {
    let token_iter = stream.into_iter();

    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Rust);

    let token_iter = extract_compiler_config(token_iter, &mut compiler_config);

    let mut tokens = vec![];
    fill_token_vec(token_iter, &mut tokens);

    #[rustversion::since(1.88)]
    fn local_file(tokens: &[parser::Token]) -> Option<PathBuf> {
        tokens.first()?.span?.local_file()
    }
    #[rustversion::before(1.88)]
    fn local_file(_: &[parser::Token]) -> Option<PathBuf> {
        None
    }

    let source_file = if let Some(path) = local_file(&tokens) {
        diagnostics::SourceFileInner::from_path_only(path)
    } else if let Some(cargo_manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let mut path: std::path::PathBuf = cargo_manifest.into();
        path.push("Cargo.toml");
        diagnostics::SourceFileInner::from_path_only(path)
    } else {
        diagnostics::SourceFileInner::from_path_only(Default::default())
    };
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parser::parse_tokens(tokens.clone(), source_file, &mut diag);
    if diag.has_errors() {
        return diag.report_macro_diagnostic(&tokens);
    }

    //println!("{syntax_node:#?}");
    compiler_config.translation_domain = std::env::var("CARGO_PKG_NAME").ok();
    let (root_component, diag, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diag, compiler_config));
    //println!("{tree:#?}");
    if diag.has_errors() {
        return diag.report_macro_diagnostic(&tokens);
    }

    let mut result = generator::rust::generate(&root_component, &loader.compiler_config)
        .unwrap_or_else(|e| {
            let e_str = e.to_string();
            quote!(compile_error!(#e_str))
        });

    // Make sure to recompile if any of the external files changes
    let reload = diag
        .all_loaded_files
        .iter()
        .filter(|path| path.is_absolute() && !path.ends_with("Cargo.toml"))
        .filter_map(|p| p.to_str())
        .map(|p| quote! {const _ : &'static [u8] = ::core::include_bytes!(#p);});

    result.extend(reload);
    result.extend(quote! {const _ : ::core::option::Option<&'static str> = ::core::option_env!("SLINT_STYLE");});

    let mut result = TokenStream::from(result);
    if !diag.is_empty() {
        result.extend(diag.report_macro_diagnostic(&tokens));
    }
    result
}
