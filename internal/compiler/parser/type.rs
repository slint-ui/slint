// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Module containing the parsing functions for type names

use super::document::parse_qualified_name;
use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test,Type
/// string
/// [ int ]
/// {a: string, b: int}
/// ```
pub fn parse_type(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::Type);
    match p.nth(0).kind() {
        SyntaxKind::LBrace => parse_type_object(&mut *p),
        SyntaxKind::LBracket => parse_type_array(&mut *p),
        _ => {
            parse_qualified_name(&mut *p);
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,ObjectType
/// {a: string, b: int}
/// {}
/// {a: string}
/// {a: string,}
/// {a: { foo: string, bar: int, }, q: {} }
/// ```
pub fn parse_type_object(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::ObjectType);
    if !p.expect(SyntaxKind::LBrace) {
        return;
    }
    while p.nth(0).kind() != SyntaxKind::RBrace {
        let mut p = p.start_node(SyntaxKind::ObjectTypeMember);
        p.expect(SyntaxKind::Identifier);
        p.expect(SyntaxKind::Colon);
        parse_type(&mut *p);
        if p.peek().kind() == SyntaxKind::Semicolon {
            p.error("Expected ','. Use ',' instead of ';' to separate fields in a struct");
            p.consume();
            continue;
        }
        if !p.test(SyntaxKind::Comma) {
            break;
        }
    }
    p.expect(SyntaxKind::RBrace);
}

#[cfg_attr(test, parser_test)]
/// ```test,ArrayType
/// [int]
/// [[int]]
/// [{a: string, b: [string]}]
/// ```
pub fn parse_type_array(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::ArrayType);
    p.expect(SyntaxKind::LBracket);
    parse_type(&mut *p);
    p.expect(SyntaxKind::RBracket);
}

#[cfg_attr(test, parser_test)]
/// ```test,StructDeclaration
/// struct Foo := { foo: bar, xxx: { aaa: bbb, } }
/// struct Bar := {}
/// struct Foo { foo: bar, xxx: { aaa: bbb, } }
/// struct Bar {}
/// ```
pub fn parse_struct_declaration<P: Parser>(p: &mut P, checkpoint: Option<P::Checkpoint>) -> bool {
    debug_assert_eq!(p.peek().as_str(), "struct");
    let mut p = p.start_node_at(checkpoint, SyntaxKind::StructDeclaration);
    p.consume(); // "struct"
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }

    if p.peek().kind() == SyntaxKind::ColonEqual {
        p.warning("':=' to declare a struct is deprecated. Remove the ':='");
        p.consume();
    }

    parse_type_object(&mut *p);
    true
}

#[cfg_attr(test, parser_test)]
/// ```test,EnumDeclaration
/// enum Foo {}
/// enum Foo { el1 }
/// enum Foo { el1, xxx, yyy }
/// ```
pub fn parse_enum_declaration<P: Parser>(p: &mut P, checkpoint: Option<P::Checkpoint>) -> bool {
    debug_assert_eq!(p.peek().as_str(), "enum");
    let mut p = p.start_node_at(checkpoint, SyntaxKind::EnumDeclaration);
    p.consume(); // "enum"
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }

    if !p.expect(SyntaxKind::LBrace) {
        return false;
    }
    while p.nth(0).kind() != SyntaxKind::RBrace {
        {
            let mut p = p.start_node(SyntaxKind::EnumValue);
            p.expect(SyntaxKind::Identifier);
        }
        if !p.test(SyntaxKind::Comma) {
            break;
        }
    }
    p.expect(SyntaxKind::RBrace);
    true
}

/// ```test,AtRustAttr
/// @rustattr(derive([()]), just some token({()}) ()..)
/// @rustattr()
/// ```
pub fn parse_rustattr(p: &mut impl Parser) -> bool {
    debug_assert_eq!(p.peek().as_str(), "@");
    p.consume(); // "@"
    if p.peek().as_str() != "rust-attr" {
        p.expect(SyntaxKind::AtRustAttr);
    }
    p.consume(); // "rust-attr"
    p.expect(SyntaxKind::LParent);
    {
        let mut p = p.start_node(SyntaxKind::AtRustAttr);
        let mut level = 1;
        loop {
            match p.peek().kind() {
                SyntaxKind::LParent => level += 1,
                SyntaxKind::RParent => {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                }
                SyntaxKind::Eof => {
                    p.error("unmatched parentheses in @rust-attr");
                    return false;
                }
                _ => {}
            }
            p.consume()
        }
    }
    p.expect(SyntaxKind::RParent)
}
