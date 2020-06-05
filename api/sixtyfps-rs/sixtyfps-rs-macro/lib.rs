extern crate proc_macro;
use proc_macro::{Spacing, TokenStream};
use quote::ToTokens;
use sixtyfps_compilerlib::*;

fn fill_token_vec(stream: TokenStream, vec: &mut Vec<parser::Token>) {
    let mut prev_spacing = Spacing::Alone;
    for t in stream {
        use parser::SyntaxKind;
        use proc_macro::TokenTree;

        match t {
            TokenTree::Ident(i) => {
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
                            if last.kind == SyntaxKind::Colon && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::ColonEqual;
                                last.text = ":=".into();
                                continue;
                            }
                            if last.kind == SyntaxKind::Plus && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::PlusEqual;
                                last.text = "+=".into();
                                continue;
                            }
                            if last.kind == SyntaxKind::Minus && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::MinusEqual;
                                last.text = "-=".into();
                                continue;
                            }
                            if last.kind == SyntaxKind::Div && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::DivEqual;
                                last.text = "/=".into();
                                continue;
                            }
                            if last.kind == SyntaxKind::Star && prev_spacing == Spacing::Joint {
                                last.kind = SyntaxKind::StarEqual;
                                last.text = "*=".into();
                                continue;
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
                    Bracket => todo!(),
                    None => todo!(),
                };
                vec.push(parser::Token {
                    kind: l,
                    text: sl.into(),
                    span: Some(g.span()), // span_open is not stable
                    ..Default::default()
                });
                fill_token_vec(g.stream(), vec);
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

#[proc_macro]
pub fn sixtyfps(stream: TokenStream) -> TokenStream {
    let mut tokens = vec![];
    fill_token_vec(stream, &mut tokens);

    let (syntax_node, mut diag) = parser::parse_tokens(tokens.clone());

    if let Ok(cargo_manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        diag.current_path = cargo_manifest.into();
        diag.current_path.push("Cargo.toml");
    }

    //println!("{:#?}", syntax_node);
    let mut tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    run_passes(&tree, &mut diag, &mut tr);
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
