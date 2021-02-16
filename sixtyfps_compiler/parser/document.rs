/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use super::expressions::parse_expression;
use super::prelude::*;
use super::r#type::{parse_struct_declaration, parse_type};
use super::statements::parse_statement;

#[cfg_attr(test, parser_test)]
/// ```test,Document
/// Type := Base { }
/// Type := Base { SubElement { } }
/// Comp := Base {}  Type := Base {}
/// Type := Base {} export { Type }
/// import { Base } from "somewhere"; Type := Base {}
/// struct Foo := { foo: foo }
/// ```
pub fn parse_document(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Document);

    loop {
        match p.peek().as_str() {
            "export" => {
                if !parse_export(&mut *p) {
                    return false;
                }
            }
            "import" => {
                if !parse_import_specifier(&mut *p) {
                    return false;
                }
            }
            "struct" => {
                if !parse_struct_declaration(&mut *p) {
                    return false;
                }
            }
            _ => {
                if !parse_component(&mut *p) {
                    return false;
                }
            }
        }

        if p.nth(0).kind() == SyntaxKind::Eof {
            return true;
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,Component
/// Type := Base { }
/// Type := Base { prop: value; }
/// Type := Base { SubElement { } }
/// global Struct := { property<int> xx; }
/// ```
pub fn parse_component(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Component);
    let is_global = p.peek().as_str() == "global";
    if is_global {
        p.consume();
    }
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        if !p.expect(SyntaxKind::Identifier) {
            return false;
        }
    }
    if !p.expect(SyntaxKind::ColonEqual) {
        return false;
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
/// ```test,Element
/// Item { }
/// Item { property: value; SubElement { } }
/// ```
pub fn parse_element(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Element);
    if !(parse_qualified_name(&mut *p) && p.expect(SyntaxKind::LBrace)) {
        return false;
    }

    parse_element_content(&mut *p);

    p.expect(SyntaxKind::RBrace)
}

#[cfg_attr(test, parser_test)]
/// ```test
/// property1: value; property2: value;
/// sub := Sub { }
/// for xx in model: Sub {}
/// if (condition) : Sub {}
/// clicked => {}
/// callback foobar;
/// property<int> width;
/// animate someProp { }
/// animate * { }
/// @children
/// double_binding <=> element.property;
/// ```
fn parse_element_content(p: &mut impl Parser) {
    let mut had_parse_error = false;
    loop {
        match p.nth(0).kind() {
            SyntaxKind::RBrace => return,
            SyntaxKind::Eof => return,
            SyntaxKind::Identifier => match p.nth(1).kind() {
                SyntaxKind::Colon => parse_property_binding(&mut *p),
                SyntaxKind::ColonEqual | SyntaxKind::LBrace => parse_sub_element(&mut *p),
                SyntaxKind::FatArrow | SyntaxKind::LParent if p.peek().as_str() != "if" => {
                    parse_callback_connection(&mut *p)
                }
                SyntaxKind::DoubleArrow => parse_two_way_binding(&mut *p),
                SyntaxKind::Identifier if p.peek().as_str() == "for" => {
                    parse_repeated_element(&mut *p);
                }
                SyntaxKind::Identifier if p.peek().as_str() == "callback" => {
                    parse_callback_declaration(&mut *p);
                }
                SyntaxKind::Identifier | SyntaxKind::Star if p.peek().as_str() == "animate" => {
                    parse_property_animation(&mut *p);
                }
                SyntaxKind::LAngle if p.peek().as_str() == "property" => {
                    parse_property_declaration(&mut *p);
                }
                SyntaxKind::LParent if p.peek().as_str() == "if" => {
                    parse_if_element(&mut *p);
                }
                SyntaxKind::LBracket if p.peek().as_str() == "states" => {
                    parse_states(&mut *p);
                }
                SyntaxKind::LBracket if p.peek().as_str() == "transitions" => {
                    parse_transitions(&mut *p);
                }
                _ => {
                    p.consume();
                    if !had_parse_error {
                        p.error("Parse error");
                        had_parse_error = true;
                    }
                }
            },
            SyntaxKind::At => {
                let checkpoint = p.checkpoint();
                p.consume();
                if p.peek().as_str() == "children" {
                    {
                        let _ =
                            p.start_node_at(checkpoint.clone(), SyntaxKind::ChildrenPlaceholder);
                    }
                    p.consume()
                } else {
                    p.consume();
                    p.error("Unexpected identifier. Expected @children.")
                }
            }
            _ => {
                if !had_parse_error {
                    p.error("Parse error");
                    had_parse_error = true;
                }
                p.consume();
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,SubElement
/// Bar {}
/// foo := Bar {}
/// Bar { x : y ; }
/// ```
/// Must consume at least one token
fn parse_sub_element(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::SubElement);
    if p.nth(1).kind() == SyntaxKind::ColonEqual {
        assert!(p.expect(SyntaxKind::Identifier));
        p.expect(SyntaxKind::ColonEqual);
    }
    parse_element(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test,RepeatedElement
/// for xx in mm: Elem { }
/// for [idx] in mm: Elem { }
/// for xx [idx] in foo.bar: Elem { }
/// ```
/// Must consume at least one token
fn parse_repeated_element(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "for");
    let mut p = p.start_node(SyntaxKind::RepeatedElement);
    p.consume(); // "for"
    if p.nth(0).kind() == SyntaxKind::Identifier {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    if p.nth(0).kind() == SyntaxKind::LBracket {
        let mut p = p.start_node(SyntaxKind::RepeatedIndex);
        p.expect(SyntaxKind::LBracket);
        p.expect(SyntaxKind::Identifier);
        p.expect(SyntaxKind::RBracket);
    }
    if p.peek().as_str() != "in" {
        p.error("Invalid 'for' syntax: there should be a 'in' token");
        return;
    }
    p.consume(); // "in"
    parse_expression(&mut *p);
    p.expect(SyntaxKind::Colon);
    parse_element(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test,ConditionalElement
/// if (condition) : Elem { }
/// if (foo ? bar : xx) : Elem { foo:bar; Elem {}}
/// ```
/// Must consume at least one token
fn parse_if_element(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "if");
    let mut p = p.start_node(SyntaxKind::ConditionalElement);
    p.consume(); // "if"
    if !p.expect(SyntaxKind::LParent) {
        return;
    }
    parse_expression(&mut *p);
    if !p.expect(SyntaxKind::RParent) || !p.expect(SyntaxKind::Colon) {
        return;
    }
    parse_element(&mut *p);
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
/// ```test,Binding
/// foo: bar;
/// foo: {}
/// ```
fn parse_property_binding(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::Binding);
    p.consume();
    p.expect(SyntaxKind::Colon);
    parse_binding_expression(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test,BindingExpression
/// {  }
/// expression ;
/// {expression }
/// {object: 42};
/// ```
fn parse_binding_expression(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::BindingExpression);
    if p.nth(0).kind() == SyntaxKind::LBrace && p.nth(2).kind() != SyntaxKind::Colon {
        parse_code_block(&mut *p);
        p.test(SyntaxKind::Semicolon);
        true
    } else {
        parse_expression(&mut *p) && p.expect(SyntaxKind::Semicolon)
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,CodeBlock
/// {  }
/// { expression }
/// { expression ; expression }
/// { expression ; expression ; }
/// { ;;;; }
/// ```
pub fn parse_code_block(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::CodeBlock);
    p.expect(SyntaxKind::LBrace); // Or assert?

    while p.nth(0).kind() != SyntaxKind::RBrace {
        if !parse_statement(&mut *p) {
            break;
        }
    }
    p.expect(SyntaxKind::RBrace);
}

#[cfg_attr(test, parser_test)]
/// ```test,CallbackConnection
/// clicked => {}
/// clicked() => { foo; }
/// mouse_move(x, y) => {}
/// mouse_move(x, y, ) => { bar; goo; }
/// ```
fn parse_callback_connection(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::CallbackConnection);
    p.consume(); // the identifier
    if p.test(SyntaxKind::LParent) {
        while p.peek().kind() != SyntaxKind::RParent {
            {
                let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
                p.expect(SyntaxKind::Identifier);
            }
            if !p.test(SyntaxKind::Comma) {
                break;
            }
        }
        p.expect(SyntaxKind::RParent);
    }
    p.expect(SyntaxKind::FatArrow);
    parse_code_block(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test,TwoWayBinding
/// foo <=> bar;
/// foo <=> bar.xxx;
/// ```
fn parse_two_way_binding(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::TwoWayBinding);
    p.consume(); // the identifier
    p.expect(SyntaxKind::DoubleArrow);
    parse_expression(&mut *p);
    p.expect(SyntaxKind::Semicolon);
}

#[cfg_attr(test, parser_test)]
/// ```test,CallbackDeclaration
/// callback foobar;
/// callback my_callback();
/// callback foo(int, string);
/// callback one_arg({ a: string, b: string});
/// callback end_coma(a, b, c,);
/// callback with_return(a, b) -> int;
/// callback with_return2({a: string}) -> { a: string };
/// ```
/// Must consume at least one token
fn parse_callback_declaration(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "callback");
    let mut p = p.start_node(SyntaxKind::CallbackDeclaration);
    p.consume(); // "callback"
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    if p.test(SyntaxKind::LParent) {
        while p.peek().kind() != SyntaxKind::RParent {
            parse_type(&mut *p);
            if !p.test(SyntaxKind::Comma) {
                break;
            }
        }
        p.expect(SyntaxKind::RParent);
        if p.test(SyntaxKind::Arrow) {
            let mut p = p.start_node(SyntaxKind::ReturnType);
            parse_type(&mut *p);
        }
    } else if p.test(SyntaxKind::Arrow) {
        // Force callback with return value to also have parentheses, we could remove this
        // restriction in the future
        p.error("Callback with return value must be declared with parentheses e.g. 'callback foo() -> int;'");
        parse_type(&mut *p);
    }
    p.expect(SyntaxKind::Semicolon);
}

#[cfg_attr(test, parser_test)]
/// ```test,PropertyDeclaration
/// property<int> foobar;
/// property<string> text: "Something";
/// property<string> text <=> two.way;
/// ```
fn parse_property_declaration(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "property");
    let mut p = p.start_node(SyntaxKind::PropertyDeclaration);
    p.consume(); // property
    p.expect(SyntaxKind::LAngle);
    parse_type(&mut *p);
    p.expect(SyntaxKind::RAngle);
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    match p.nth(0).kind() {
        SyntaxKind::Colon => {
            p.consume();
            parse_binding_expression(&mut *p);
        }
        SyntaxKind::DoubleArrow => {
            let mut p = p.start_node(SyntaxKind::TwoWayBinding);
            p.consume();
            parse_expression(&mut *p);
            p.expect(SyntaxKind::Semicolon);
        }
        _ => {
            p.expect(SyntaxKind::Semicolon);
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,PropertyAnimation
/// animate x { duration: 1000; }
/// animate x, foo.y {  }
/// animate * {  }
/// ```
fn parse_property_animation(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "animate");
    let mut p = p.start_node(SyntaxKind::PropertyAnimation);
    p.consume(); // animate
    if p.nth(0).kind() == SyntaxKind::Star {
        p.consume();
    } else {
        parse_qualified_name(&mut *p);
        while p.nth(0).kind() == SyntaxKind::Comma {
            p.consume();
            parse_qualified_name(&mut *p);
        }
    };
    p.expect(SyntaxKind::LBrace);

    loop {
        match p.nth(0).kind() {
            SyntaxKind::RBrace => {
                p.consume();
                return;
            }
            SyntaxKind::Eof => return,
            SyntaxKind::Identifier => match p.nth(1).kind() {
                SyntaxKind::Colon => parse_property_binding(&mut *p),
                _ => {
                    p.consume();
                    p.error("Only bindings are allowed in animations");
                }
            },
            _ => {
                p.consume();
                p.error("Only bindings are allowed in animations");
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,States
/// states []
/// states [ foo when bar : { x:y; } another_state : { x:z; }]
/// ```
fn parse_states(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "states");
    let mut p = p.start_node(SyntaxKind::States);
    p.consume(); // "states"
    p.expect(SyntaxKind::LBracket);
    while parse_state(&mut *p) {}
    p.expect(SyntaxKind::RBracket);
}

#[cfg_attr(test, parser_test)]
/// ```test,State
/// foo : { x: 1px + 2px; aaa.y: {1px + 2px} }
/// foo when bar == 1:  { color: blue; foo.color: red;   }
/// ```
fn parse_state(p: &mut impl Parser) -> bool {
    if p.nth(0).kind() != SyntaxKind::Identifier {
        return false;
    }
    let mut p = p.start_node(SyntaxKind::State);
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    if p.peek().as_str() == "when" {
        p.consume();
        parse_expression(&mut *p);
    }
    p.expect(SyntaxKind::Colon);
    if !p.expect(SyntaxKind::LBrace) {
        return false;
    }

    loop {
        match p.nth(0).kind() {
            SyntaxKind::RBrace => {
                p.consume();
                return true;
            }
            SyntaxKind::Eof => return false,
            _ => {
                let mut p = p.start_node(SyntaxKind::StatePropertyChange);
                if !parse_qualified_name(&mut *p)
                    || !p.expect(SyntaxKind::Colon)
                    || !parse_binding_expression(&mut *p)
                {
                    return false;
                }
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,Transitions
/// transitions []
/// transitions [in checked: {animate x { duration: 88ms; }} out checked: {animate x { duration: 88ms; }}]
/// ```
fn parse_transitions(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "transitions");
    let mut p = p.start_node(SyntaxKind::Transitions);
    p.consume(); // "transitions"
    p.expect(SyntaxKind::LBracket);
    while p.nth(0).kind() != SyntaxKind::RBracket && parse_transition(&mut *p) {}
    p.expect(SyntaxKind::RBracket);
}

#[cfg_attr(test, parser_test)]
/// ```test,Transition
/// in pressed : {}
/// in pressed: { animate x { duration: 88ms; } }
/// out pressed: { animate x { duration: 88ms; } }
/// ```
fn parse_transition(p: &mut impl Parser) -> bool {
    if !matches!(p.peek().as_str(), "in" | "out") {
        p.error("Expected 'in' or 'out' to declare a transition");
        return false;
    }
    let mut p = p.start_node(SyntaxKind::Transition);
    p.consume(); // "in" or "out"
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    p.expect(SyntaxKind::Colon);
    if !p.expect(SyntaxKind::LBrace) {
        return false;
    }

    loop {
        match p.nth(0).kind() {
            SyntaxKind::RBrace => {
                p.consume();
                return true;
            }
            SyntaxKind::Eof => return false,
            SyntaxKind::Identifier if p.peek().as_str() == "animate" => {
                parse_property_animation(&mut *p);
            }
            _ => {
                p.consume();
                p.error("Expected 'animate'");
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,ExportsList
/// export { Type }
/// export { Type, AnotherType }
/// export { Type as Foo, AnotherType }
/// export Foo := Item { }
/// export struct Foo := { foo: bar }
/// ```
fn parse_export(p: &mut impl Parser) -> bool {
    debug_assert_eq!(p.peek().as_str(), "export");
    let mut p = p.start_node(SyntaxKind::ExportsList);
    p.consume(); // "export"
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
/// ```
fn parse_import_specifier(p: &mut impl Parser) -> bool {
    debug_assert_eq!(p.peek().as_str(), "import");
    let mut p = p.start_node(SyntaxKind::ImportSpecifier);
    p.consume(); // "import"
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
/// { }
/// { Type as Alias1, Type as AnotherAlias }
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
