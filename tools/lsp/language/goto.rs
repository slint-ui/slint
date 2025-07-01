// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::{
    self,
    token_info::{token_info, TokenInfo},
};
use crate::util;
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::parser::{SyntaxNode, SyntaxToken};
use lsp_types::{GotoDefinitionResponse, LocationLink, Position, Range};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub fn goto_definition(
    document_cache: &mut common::DocumentCache,
    token: SyntaxToken,
) -> Option<GotoDefinitionResponse> {
    let token_info = token_info(document_cache, token.clone())?;
    match token_info {
        TokenInfo::Type(ty) => goto_type(&ty),
        TokenInfo::ElementType(el) => {
            if let ElementType::Component(c) = el {
                goto_node(&c.root_element.borrow().debug.first()?.node)
            } else {
                None
            }
        }
        TokenInfo::ElementRc(el) => goto_node(&el.borrow().debug.first()?.node),
        TokenInfo::NamedReference(nr) => {
            let mut el = nr.element();
            loop {
                if let Some(x) = el.borrow().property_declarations.get(nr.name()) {
                    return goto_node(x.node.as_ref()?);
                }
                let base = el.borrow().base_type.clone();
                if let ElementType::Component(c) = base {
                    el = c.root_element.clone();
                } else {
                    return None;
                }
            }
        }
        TokenInfo::EnumerationValue(v) => {
            // FIXME: this goes to the enum definition instead of the value definition.
            goto_node(v.enumeration.node.as_ref()?)
        }
        TokenInfo::FileName(f) | TokenInfo::Image(f) => {
            if let Some(doc) = document_cache.get_document_by_path(&f) {
                let doc_node = doc.node.clone()?;
                goto_node(&doc_node)
            } else if f.is_file() || cfg!(test) {
                // WASM will never get here, but that is fine: Slintpad can not open images anyway;-)
                return Some(GotoDefinitionResponse::Link(vec![LocationLink {
                    origin_selection_range: Some(util::token_to_lsp_range(&token)),
                    target_uri: lsp_types::Url::from_file_path(&f).ok()?,
                    target_range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    target_selection_range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                }]));
            } else {
                None
            }
        }
        TokenInfo::LocalProperty(x) => goto_node(&x),
        TokenInfo::LocalCallback(x) => goto_node(&x),
        TokenInfo::LocalFunction(x) => goto_node(&x),
        TokenInfo::IncompleteNamedReference(mut element_type, prop_name) => {
            while let ElementType::Component(com) = element_type {
                if let Some(p) = com.root_element.borrow().property_declarations.get(&prop_name) {
                    return goto_node(p.node.as_ref()?);
                }
                element_type = com.root_element.borrow().base_type.clone();
            }
            None
        }
    }
}

fn goto_type(ty: &Type) -> Option<GotoDefinitionResponse> {
    match ty {
        Type::Struct(s) if s.node.is_some() => {
            goto_node(s.node.as_ref().unwrap().parent().as_ref()?)
        }
        Type::Enumeration(e) => goto_node(e.node.as_ref()?),
        _ => None,
    }
}

fn goto_node(node: &SyntaxNode) -> Option<GotoDefinitionResponse> {
    let (target_uri, range) = crate::util::node_to_url_and_lsp_range(node)?;
    let range = Range::new(range.start, range.start); // Shrink range to a position:-)
    Some(GotoDefinitionResponse::Link(vec![LocationLink {
        origin_selection_range: None,
        target_uri,
        target_range: range,
        target_selection_range: range,
    }]))
}

#[cfg(test)]
use i_slint_compiler::parser::TextSize;

#[test]
fn test_goto_definition() {
    fn first_link(def: &GotoDefinitionResponse) -> &LocationLink {
        let GotoDefinitionResponse::Link(link) = def else { panic!("not a single link {def:?}") };
        link.first().unwrap()
    }

    let source = r#"
import { Button } from "std-widgets.slint";
component Abc {
    in property <string> hello;
}
export component Test {
    abc := Abc {
        hello: "foo";
        changed hello => {}
    }
    btn := Button {
        text: abc.hello;
    }
    rec := Rectangle { }

    img := Image {
        source: @image-url("test.png")
    }
}"#;

    let (mut dc, uri, _) = crate::language::test::loaded_document_cache(source.into());
    let doc = dc.get_document(&uri).unwrap().node.clone().unwrap();

    // Jump to the definition of Abc
    let offset: TextSize = (source.find("abc := Abc").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset + TextSize::new(8)).unwrap();
    assert_eq!(token.text(), "Abc");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 2);

    // Jump to the definition of abc
    let offset: TextSize = (source.find("text: abc.hello").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset + TextSize::new(7)).unwrap();
    assert_eq!(token.text(), "abc");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 6);

    // Jump to the definition of hello
    let offset: TextSize = (source.find("text: abc.hello").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset + TextSize::new(12)).unwrap();
    assert_eq!(token.text(), "hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 3);

    // Also jump to the definition of hello
    let offset = (source.find("hello: \"foo\"").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset).unwrap();
    assert_eq!(token.text(), "hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 3);

    // Rectangle is builtin and not accessible
    let offset: TextSize = (source.find("rec := ").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset + TextSize::new(8)).unwrap();
    assert_eq!(token.text(), "Rectangle");
    assert!(goto_definition(&mut dc, token).is_none());

    // Button is builtin and not accessible
    let offset: TextSize = (source.find("btn := ").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset + TextSize::new(9)).unwrap();
    assert_eq!(token.text(), "Button");
    assert!(goto_definition(&mut dc, token).is_none());
    let offset = (source.find("text: abc.hello").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset).unwrap();
    assert_eq!(token.text(), "text");
    assert!(goto_definition(&mut dc, token).is_none());

    // Jump from a changed callback
    let offset: TextSize = (source.find("changed hello").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset + TextSize::new(12)).unwrap();
    assert_eq!(token.text(), "hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri);
    assert_eq!(link.target_range.start.line, 3);

    // Jump to test.png image url
    let offset: TextSize = (source.find("\"test.png\"").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc, offset + TextSize::new(1)).unwrap();

    assert_eq!(token.text(), "\"test.png\"");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, uri.join("test.png").unwrap());
    assert_eq!(link.target_range.start.line, 0);
    assert_eq!(link.target_range.start.character, 0);
}

#[test]
fn test_goto_definition_multi_files() {
    fn first_link(def: &GotoDefinitionResponse) -> &LocationLink {
        let GotoDefinitionResponse::Link(link) = def else { panic!("not a single link {def:?}") };
        link.first().unwrap()
    }

    let source1 = r#"
    export component Hello {
        in-out property <int> the_prop;
    }
    export struct AStruct {
        f: int
    }
    export component Another {
        callback xx;
    }
    "#;
    let (mut dc, url1, diags) = crate::language::test::loaded_document_cache(source1.into());
    for (u, ds) in diags {
        assert_eq!(ds, vec![], "errors in {u}");
    }
    let url2 = url1.join("../file2.slint").unwrap();
    let source2 = format!(
        r#"
        import {{ Hello }} from "{url1}";
        export {{ Another as A }} from "{url1}";
        export component Foo {{ h := Hello {{ the_prop: 42; }} }}
    "#,
        url1 = url1.to_file_path().unwrap().display()
    );
    let (extra_files, diag) = spin_on::spin_on(crate::language::reload_document_impl(
        None,
        source2.clone(),
        url2.clone(),
        Some(43),
        &mut dc,
    ));
    let diag = crate::language::convert_diagnostics(&extra_files, diag);
    for (u, ds) in diag {
        assert_eq!(ds, vec![], "errors in {u}");
    }

    let doc2 = dc.get_document(&url2).unwrap().node.clone().unwrap();

    let offset: TextSize = (source2.find("h := Hello").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc2, offset + TextSize::new(8)).unwrap();
    assert_eq!(token.text(), "Hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 1);

    let offset = (source2.find("the_prop: 42").unwrap() as u32).into();
    let token = crate::language::token_at_offset(&doc2, offset).unwrap();
    assert_eq!(token.text(), "the_prop");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 2);

    let offset = (source2.find("Hello } from ").unwrap() as u32).into();
    // check the string literal
    let token = crate::language::token_at_offset(&doc2, offset + TextSize::new(20)).unwrap();
    assert_eq!(token.kind(), i_slint_compiler::parser::SyntaxKind::StringLiteral);
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 0);
    // check the identifier
    let token = crate::language::token_at_offset(&doc2, offset).unwrap();
    assert_eq!(token.text(), "Hello");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 1);

    let offset = (source2.find("Another as A } from ").unwrap() as u32).into();
    // check the string literal
    let token = crate::language::token_at_offset(&doc2, offset + TextSize::new(25)).unwrap();
    assert_eq!(token.kind(), i_slint_compiler::parser::SyntaxKind::StringLiteral);
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 0);
    // check the identifier
    let token = crate::language::token_at_offset(&doc2, offset).unwrap();
    assert_eq!(token.text(), "Another");
    let def = goto_definition(&mut dc, token).unwrap();
    let link = first_link(&def);
    assert_eq!(link.target_uri, url1);
    assert_eq!(link.target_range.start.line, 7);
}
