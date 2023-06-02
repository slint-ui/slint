// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

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
pub fn parse_struct_declaration(p: &mut impl Parser) -> bool {
    debug_assert_eq!(p.peek().as_str(), "struct");
    let mut p = p.start_node(SyntaxKind::StructDeclaration);
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

pub fn parse_rustattr(p: &mut impl Parser) -> bool {
    let checkpoint = p.checkpoint();
    debug_assert_eq!(p.peek().as_str(), "@");
    p.consume(); // "@"
    if p.peek().as_str() != "rust-attr" {
        p.expect(SyntaxKind::AtRustAttr);
    }
    p.consume(); // "rust-attr"
    p.expect(SyntaxKind::LParent);
    parse_parentheses(&mut *p);
    if p.peek().as_str() == "export" {
        p.consume();
    }
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

fn parse_parentheses(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::AtRustAttr);
    let mut opened = 0;
    let mut closed = 0;
    while closed <= opened {
        if p.peek().kind() == SyntaxKind::LParent {
            opened += 1;
        }
        if p.peek().kind() == SyntaxKind::RParent {
            closed += 1;
        }
        if closed == opened && opened != 0 && closed != 0 && p.peek().kind() != SyntaxKind::RParent
        {
            p.error("Parse error: `)` or `,`");
            return false;
        }
        p.consume();
    }
    if p.peek().as_str() != "struct" && p.peek().as_str() != "export" {
        p.error("Parse error: expected `struct` or `export`");
        return false;
    }
    true
}
