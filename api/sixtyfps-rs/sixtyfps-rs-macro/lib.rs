extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use sixtyfps_compiler::*;

fn fill_token_vec(stream: TokenStream, vec: &mut Vec<parser::Token>) {
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
                    '=' => SyntaxKind::Equal,
                    ';' => SyntaxKind::Semicolon,
                    _ => SyntaxKind::Error,
                };
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

    let (syntax_node, mut diag) = parser::parse_tokens(tokens);
    //println!("{:#?}", syntax_node);
    let tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &tr);
    //println!("{:#?}", tree);
    if !diag.inner.is_empty() {
        let diags: Vec<_> = diag
            .into_iter()
            .map(|diagnostics::CompilerDiagnostic { message, span }| {
                quote::quote_spanned!(span.span.unwrap().into() => compile_error!{ #message })
            })
            .collect();
        return quote!(#(#diags)*).into();
    }

    let l = lower::LoweredComponent::lower(&*tree.root_component);
    generator::generate(&l);

    quote!(
        #[derive(Default)]
        struct SuperSimple;
        impl SuperSimple {
            fn run(&self) {
                println!("Hello world");
            }

        }
    )
    .into()
}
