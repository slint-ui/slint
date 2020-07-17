extern crate proc_macro;
use proc_macro::{Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use sixtyfps_compilerlib::*;

fn fill_token_vec(stream: impl Iterator<Item = TokenTree>, vec: &mut Vec<parser::Token>) {
    let mut prev_spacing = Spacing::Alone;
    for t in stream {
        use parser::SyntaxKind;

        match t {
            TokenTree::Ident(i) => {
                if let Some(last) = vec.last_mut() {
                    if last.kind == SyntaxKind::ColorLiteral && last.text.len() == 1 {
                        last.text = format!("#{}", i).into();
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
                    '-' => SyntaxKind::Minus,
                    '*' => SyntaxKind::Star,
                    '/' => SyntaxKind::Div,
                    '<' => SyntaxKind::LAngle,
                    '>' => {
                        if let Some(last) = vec.last_mut() {
                            if last.kind == SyntaxKind::Equal && prev_spacing == Spacing::Joint {
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
                        if last.kind == SyntaxKind::ColorLiteral && last.text.len() == 1 {
                            last.text = format!("#{}", s).into();
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
                    let path_with_quotes_stripped = path_with_quotes.trim_matches('\"');
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

#[proc_macro]
pub fn sixtyfps(stream: TokenStream) -> TokenStream {
    let token_iter = stream.into_iter();

    let (token_iter, include_paths) = extract_include_paths(token_iter);

    let mut tokens = vec![];
    fill_token_vec(token_iter, &mut tokens);

    let (syntax_node, mut diag) = parser::parse_tokens(tokens.clone());

    if let Some(cargo_manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
        diag.current_path = cargo_manifest.into();
        diag.current_path.push("Cargo.toml");
    }

    //println!("{:#?}", syntax_node);
    let compiler_config =
        CompilerConfiguration { include_paths: &include_paths, ..Default::default() };
    let (tree, mut diag) = compile_syntax_node(syntax_node, diag, &compiler_config);
    //println!("{:#?}", tree);
    if diag.has_error() {
        diag.map_offsets_to_span(&tokens);
        return diag.into_token_stream().into();
    }

    let result = generator::rust::generate(&tree.root_component, &mut diag);

    result
        .unwrap_or_else(|| {
            diag.map_offsets_to_span(&tokens);
            diag.into_token_stream()
        })
        .into()
}
