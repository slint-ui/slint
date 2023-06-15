// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use super::element::{parse_element, parse_element_content};
use super::prelude::*;
use super::r#type::{parse_enum_declaration, parse_rustattr, parse_struct_declaration};

#[cfg_attr(test, parser_test)]
/// ```test,Document
/// component Type { }
/// Type := Base { SubElement { } }
/// Comp := Base {}  Type := Base {}
/// component Q {} Type := Base {} export { Type }
/// import { Base } from "somewhere"; Type := Base {}
/// struct Foo { foo: foo }
/// enum Foo { hello }
/// /* empty */
/// ```
pub fn parse_document(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Document);

    loop {
        if p.test(SyntaxKind::Eof) {
            return true;
        }

        if p.peek().kind() == SyntaxKind::Semicolon {
            p.error("Extra semicolon. Remove this semicolon");
            p.consume();
            continue;
        }

        match p.peek().as_str() {
            "export" => {
                if !parse_export(&mut *p) {
                    break;
                }
            }
            "import" => {
                if !parse_import_specifier(&mut *p) {
                    break;
                }
            }
            "struct" => {
                if !parse_struct_declaration(&mut *p) {
                    break;
                }
            }
            "enum" => {
                if !parse_enum_declaration(&mut *p) {
                    break;
                }
            }
            "@" if p.nth(1).as_str() == "rust-attr" => {
                let mut is_export = false;
                let mut i = 0;
                loop {
                    let value = p.nth(i);
                    if value.as_str() == ")" && p.nth(i + 1).as_str() == "export" {
                        is_export = true;
                        break;
                    } else if (value.as_str() == ")"
                        && p.nth(i + 1).as_str() != "struct"
                        && p.nth(i + 1).as_str() != "export"
                        && p.nth(i + 1).as_str() != ")")
                        || (value.as_str() == ")" && p.nth(i + 1).as_str() == "struct")
                    {
                        break;
                    }
                    i += 1;
                }
                if is_export {
                    let mut p = p.start_node(SyntaxKind::ExportsList);
                    if !parse_rustattr(&mut *p) {
                        break;
                    }
                } else {
                    if !parse_rustattr(&mut *p) {
                        break;
                    }
                }
            }
            _ => {
                if !parse_component(&mut *p) {
                    break;
                }
            }
        }
    }
    // Always consume the whole document
    while !p.test(SyntaxKind::Eof) {
        p.consume()
    }
    false
}

#[cfg_attr(test, parser_test)]
/// ```test,Component
/// Type := Base { }
/// Type := Base { prop: value; }
/// Type := Base { SubElement { } }
/// global Struct := { }
/// global Struct { property<int> xx; }
/// component C { property<int> xx; }
/// component C inherits D { }
/// ```
pub fn parse_component(p: &mut impl Parser) -> bool {
    let simple_component = p.nth(1).kind() == SyntaxKind::ColonEqual;
    let is_global = !simple_component && p.peek().as_str() == "global";
    let is_new_component = !simple_component && p.peek().as_str() == "component";
    if !is_global && !simple_component && !is_new_component {
        p.error(
            "Parse error: expected a top-level item such as a component, a struct, or a global",
        );
        return false;
    }
    let mut p = p.start_node(SyntaxKind::Component);
    if is_global || is_new_component {
        p.consume();
    }
    if !p.start_node(SyntaxKind::DeclaredIdentifier).expect(SyntaxKind::Identifier) {
        drop(p.start_node(SyntaxKind::Element));
        return false;
    }
    if is_global {
        if p.peek().kind() == SyntaxKind::ColonEqual {
            p.warning("':=' to declare a global is deprecated. Remove the ':='");
            p.consume();
        }
    } else if !is_new_component {
        if p.peek().kind() == SyntaxKind::ColonEqual {
            p.warning("':=' to declare a component is deprecated. The new syntax declare components with 'component MyComponent {'. Read the documentation for more info");
        }
        if !p.expect(SyntaxKind::ColonEqual) {
            drop(p.start_node(SyntaxKind::Element));
            return false;
        }
    } else {
        if p.peek().as_str() == "inherits" {
            p.consume();
        } else if p.peek().kind() == SyntaxKind::LBrace {
            let mut p = p.start_node(SyntaxKind::Element);
            p.consume();
            parse_element_content(&mut *p);
            return p.expect(SyntaxKind::RBrace);
        } else {
            p.error("Expected '{' or keyword 'inherits'");
            drop(p.start_node(SyntaxKind::Element));
            return false;
        }
    }

    if is_global && p.peek().kind() == SyntaxKind::LBrace {
        let mut p = p.start_node(SyntaxKind::Element);
        p.consume();
        parse_element_content(&mut *p);
        return p.expect(SyntaxKind::RBrace);
    }

    parse_element(&mut *p)
}

#[cfg_attr(test, parser_test)]
/// ```test,QualifiedName
/// Rectangle
/// MyModule.Rectangle
/// Deeply.Nested.MyModule.Rectangle
/// ```
pub fn parse_qualified_name(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::QualifiedName);
    if !p.expect(SyntaxKind::Identifier) {
        return false;
    }

    loop {
        if p.nth(0).kind() != SyntaxKind::Dot {
            break;
        }
        p.consume();
        p.expect(SyntaxKind::Identifier);
    }

    true
}

#[cfg_attr(test, parser_test)]
/// ```test,ExportsList
/// export { Type }
/// export { Type, AnotherType }
/// export { Type as Foo, AnotherType }
/// export Foo := Item { }
/// export struct Foo := { foo: bar }
/// export enum Foo { bar }
/// export * from "foo";
/// ```
fn parse_export(p: &mut impl Parser) -> bool {
    debug_assert_eq!(p.peek().as_str(), "export");
    let mut p = p.start_node(SyntaxKind::ExportsList);
    p.expect(SyntaxKind::Identifier); // "export"
    if p.test(SyntaxKind::LBrace) {
        loop {
            parse_export_specifier(&mut *p);
            match p.nth(0).kind() {
                SyntaxKind::RBrace => {
                    p.consume();
                    return true;
                }
                SyntaxKind::Eof => {
                    p.error("Expected comma");
                    return false;
                }
                SyntaxKind::Comma => {
                    p.consume();
                }
                _ => {
                    p.consume();
                    p.error("Expected comma")
                }
            }
        }
    } else if p.peek().as_str() == "struct" {
        parse_struct_declaration(&mut *p)
    } else if p.peek().as_str() == "enum" {
        parse_enum_declaration(&mut *p)
    } else if p.peek().kind == SyntaxKind::Star {
        let mut p = p.start_node(SyntaxKind::ExportModule);
        p.consume(); // *
        if p.peek().as_str() != "from" {
            p.error("Expected from keyword for export statement");
            return false;
        }
        p.consume();
        let peek = p.peek();
        if peek.kind != SyntaxKind::StringLiteral
            || !peek.as_str().starts_with('"')
            || !peek.as_str().ends_with('"')
        {
            p.error("Expected plain string literal");
            return false;
        }
        p.consume();
        p.expect(SyntaxKind::Semicolon)
    } else {
        parse_component(&mut *p)
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,ExportSpecifier
/// Type
/// Type as Something
/// ```
fn parse_export_specifier(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::ExportSpecifier);
    {
        let mut p = p.start_node(SyntaxKind::ExportIdentifier);
        if !p.expect(SyntaxKind::Identifier) {
            return false;
        }
    }
    if p.peek().as_str() == "as" {
        p.consume();
        let mut p = p.start_node(SyntaxKind::ExportName);
        if !p.expect(SyntaxKind::Identifier) {
            return false;
        }
    }

    true
}

#[cfg_attr(test, parser_test)]
/// ```test,ImportSpecifier
/// import { Type1, Type2 } from "somewhere";
/// import "something.ttf";
/// ```
fn parse_import_specifier(p: &mut impl Parser) -> bool {
    debug_assert_eq!(p.peek().as_str(), "import");
    let mut p = p.start_node(SyntaxKind::ImportSpecifier);
    p.expect(SyntaxKind::Identifier); // "import"
    if p.peek().kind != SyntaxKind::StringLiteral {
        if !parse_import_identifier_list(&mut *p) {
            return false;
        }
        if p.peek().as_str() != "from" {
            p.error("Expected from keyword for import statement");
            return false;
        }
        if !p.expect(SyntaxKind::Identifier) {
            return false;
        }
    }
    let peek = p.peek();
    if peek.kind != SyntaxKind::StringLiteral
        || !peek.as_str().starts_with('"')
        || !peek.as_str().ends_with('"')
    {
        p.error("Expected plain string literal");
        return false;
    }
    p.consume();
    p.expect(SyntaxKind::Semicolon)
}

#[cfg_attr(test, parser_test)]
/// ```test,ImportIdentifierList
/// { Type1 }
/// { Type2, Type3 }
/// { Type as Alias1, Type as AnotherAlias }
/// {}
/// ```
fn parse_import_identifier_list(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::ImportIdentifierList);
    if !p.expect(SyntaxKind::LBrace) {
        return false;
    }
    if p.test(SyntaxKind::RBrace) {
        return true;
    }
    loop {
        parse_import_identifier(&mut *p);
        match p.nth(0).kind() {
            SyntaxKind::RBrace => {
                p.consume();
                return true;
            }
            SyntaxKind::Eof => return false,
            SyntaxKind::Comma => {
                p.consume();
            }
            _ => {
                p.consume();
                p.error("Expected comma")
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,ImportIdentifier
/// Type
/// Type as Alias1
/// ```
fn parse_import_identifier(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::ImportIdentifier);
    {
        let mut p = p.start_node(SyntaxKind::ExternalName);
        if !p.expect(SyntaxKind::Identifier) {
            return false;
        }
    }
    if p.nth(0).kind() == SyntaxKind::Identifier && p.peek().as_str() == "as" {
        p.consume();
        let mut p = p.start_node(SyntaxKind::InternalName);
        if !p.expect(SyntaxKind::Identifier) {
            return false;
        }
    }
    true
}
