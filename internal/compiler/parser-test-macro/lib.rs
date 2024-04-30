// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

extern crate proc_macro;
use core::str::FromStr;
use proc_macro::{Delimiter, TokenStream, TokenTree};

fn error(e: &str) -> String {
    format!("::core::compile_error!{{\"{}\"}}", e)
}

fn generate_test(fn_name: &str, doc: &str, extra_args: usize) -> String {
    if fn_name.is_empty() {
        return error("Could not parse function name");
    }

    if doc.is_empty() {
        return error("doc comments not found");
    }
    let mut kind = None;
    let idx = match doc.find("```test\n") {
        Some(idx) => idx + 8,
        None => match doc.find("```test,") {
            None => return error("test not found"),
            Some(idx) => match doc[idx..].find('\n') {
                None => return error("test not found"),
                Some(newline) => {
                    kind = Some(&doc[idx + 8..idx + newline]);
                    idx + newline + 1
                }
            },
        },
    };
    let doc = &doc[(idx)..];
    let idx = match doc.find("```\n") {
        Some(idx) => idx,
        None => return error("end of test not found"),
    };
    let doc = &doc[..idx];

    let verify = match kind {
        None => String::new(),
        Some(kind) => {
            format!(
                "syntax_nodes::{}::verify(SyntaxNode {{
                    node: rowan::SyntaxNode::new_root(p.builder.finish()),
                    source_file: Default::default(),
                }});",
                kind
            )
        }
    };

    let mut tests = String::new();
    for (i, line) in doc.split('\n').enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let follow_args = ", Default::default()".repeat(extra_args);
        tests += &format!(
            r#"
        #[test] fn parser_test_{fn_name}_{i}()
        {{
            let mut diag = Default::default();
            let mut p = DefaultParser::new("{line}", &mut diag);
            {fn_name}(&mut p{follow_args});
            let has_error = p.diags.has_error();
            //#[cfg(feature = "display-diagnostics")]
            //p.diags.print();
            assert!(!has_error);
            assert_eq!(p.cursor, p.tokens.len());
            {verify}
        }}
        "#,
        )
    }
    tests
}

#[proc_macro_attribute]
pub fn parser_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut result = item.clone(); // The original function

    let mut doc = String::new();
    let mut item = item.into_iter();

    let mut fn_name = String::new();
    let mut extra_args = 0;

    // Extract the doc comment.
    // Bail out once we find a token that does not fit the doc comment pattern
    loop {
        match item.next() {
            Some(TokenTree::Punct(p)) => {
                if p.as_char() != '#' {
                    break;
                }
            }
            Some(TokenTree::Ident(i)) => {
                if i.to_string() == "fn" {
                    fn_name = item.next().map_or_else(String::default, |i| i.to_string());
                }
                break;
            }
            _ => break,
        }
        if let Some(TokenTree::Group(g)) = item.next() {
            if g.delimiter() != proc_macro::Delimiter::Bracket {
                break;
            }
            let mut attr = g.stream().into_iter();
            if let Some(TokenTree::Ident(i)) = attr.next() {
                if i.to_string() != "doc" {
                    break;
                }
            } else {
                break;
            }
            if let Some(TokenTree::Punct(p)) = attr.next() {
                if p.as_char() != '=' {
                    break;
                }
            } else {
                break;
            }
            if let Some(TokenTree::Literal(lit)) = attr.next() {
                let s = lit.to_string();
                // trim the quotes
                doc += &s[1..(s.len() - 1)];
                doc += "\n";
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if fn_name.is_empty() {
        while let Some(tt) = item.next() {
            if tt.to_string() == "fn" {
                fn_name = item.next().map_or_else(String::default, |i| i.to_string());
                break;
            }
        }
    }

    loop {
        match item.next() {
            None => break,
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => {
                let mut had_coma = false;
                for tt in g.stream().into_iter() {
                    match tt {
                        TokenTree::Punct(p) if p.as_char() == ',' => {
                            had_coma = true;
                        }
                        TokenTree::Punct(p) if p.as_char() == ':' && had_coma => {
                            extra_args += 1;
                            had_coma = false;
                        }
                        _ => {}
                    }
                }
            }
            _ => (),
        }
    }

    let test_function = TokenStream::from_str(&generate_test(&fn_name, &doc, extra_args))
        .unwrap_or_else(|e| {
            TokenStream::from_str(&error(&format!("Lex error in generated test: {:?}", e))).unwrap()
        });

    result.extend(test_function);
    result
}
