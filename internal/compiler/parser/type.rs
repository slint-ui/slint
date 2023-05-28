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

#[cfg_attr(test, parser_test)]
/// ```test,AtPragma
/// @pragma(serde) struct Bar { foot : int}
/// ```
pub fn parse_pragma(p: &mut impl Parser) -> bool {
    debug_assert_eq!(p.peek().as_str(), "@");
    let mut p = p.start_node(SyntaxKind::AtPragma);
    p.consume(); // "@"
    if p.peek().as_str() != "pragma" {
        p.expect(SyntaxKind::AtPragma);
    }
    p.consume(); // "pragma"
    p.expect(SyntaxKind::LParent);
    while !p.test(SyntaxKind::RParent) {
        if !parse_deriven(&mut *p) {
            break;
        }
        p.test(SyntaxKind::Comma);
    }
    p.consume();
    if p.peek().as_str() == "export" {
        p.consume();
    }
    parse_struct_declaration(&mut *p);
    true
}

macro_rules! deriven_key {
    () => {
        ["serde"]
    };
}

pub fn parse_deriven(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Deriven);
    p.peek();
    if deriven_key!().contains(&p.nth(0).as_str()) {
        p.consume();
        true
    } else {
        p.consume();
        p.test(SyntaxKind::Identifier);
        p.error("Expected 'deriven' feature like `serde` after '@pragma'");
        false
    }
}
