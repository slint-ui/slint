// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The parser functions for elements and things inside them

use super::document::parse_qualified_name;
use super::expressions::parse_expression;
use super::prelude::*;
use super::r#type::parse_type;
use super::statements::parse_statement;

#[cfg_attr(test, parser_test)]
/// ```test,Element
/// Item { }
/// Item { property: value; SubElement { } }
/// Item { if true: Rectangle {} }
/// ```
pub fn parse_element(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Element);
    if !parse_qualified_name(&mut *p) {
        return if p.test(SyntaxKind::LBrace) {
            // recover
            parse_element_content(&mut *p);
            p.expect(SyntaxKind::RBrace)
        } else {
            false
        };
    }

    if !p.expect(SyntaxKind::LBrace) {
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
/// if condition : Sub {}
/// clicked => {}
/// callback foobar;
/// property<int> width;
/// animate someProp { }
/// animate * { }
/// @children
/// double_binding <=> element.property;
/// public pure function foo() {}
/// changed foo => {}
/// ```
pub fn parse_element_content(p: &mut impl Parser) {
    let mut had_parse_error = false;
    loop {
        match p.nth(0).kind() {
            SyntaxKind::RBrace => return,
            SyntaxKind::Eof => return,
            SyntaxKind::Identifier => match p.nth(1).kind() {
                SyntaxKind::Colon => parse_property_binding(&mut *p),
                SyntaxKind::ColonEqual | SyntaxKind::LBrace => {
                    had_parse_error |= !parse_sub_element(&mut *p)
                }
                SyntaxKind::FatArrow | SyntaxKind::LParent if p.peek().as_str() != "if" => {
                    parse_callback_connection(&mut *p)
                }
                SyntaxKind::DoubleArrow => parse_two_way_binding(&mut *p),
                SyntaxKind::Identifier if p.peek().as_str() == "for" => {
                    parse_repeated_element(&mut *p);
                }
                SyntaxKind::Identifier
                    if p.peek().as_str() == "callback"
                        || (p.peek().as_str() == "pure" && p.nth(1).as_str() == "callback") =>
                {
                    parse_callback_declaration(&mut *p);
                }
                SyntaxKind::Identifier
                    if p.peek().as_str() == "function"
                        || (matches!(p.peek().as_str(), "public" | "pure" | "protected")
                            && p.nth(1).as_str() == "function")
                        || (matches!(p.nth(1).as_str(), "public" | "pure" | "protected")
                            && p.nth(2).as_str() == "function") =>
                {
                    parse_function(&mut *p);
                }
                SyntaxKind::Identifier | SyntaxKind::Star if p.peek().as_str() == "animate" => {
                    parse_property_animation(&mut *p);
                }
                SyntaxKind::Identifier if p.peek().as_str() == "changed" => {
                    parse_changed_callback(&mut *p);
                }
                SyntaxKind::LAngle | SyntaxKind::Identifier if p.peek().as_str() == "property" => {
                    parse_property_declaration(&mut *p);
                }
                SyntaxKind::Identifier
                    if p.nth(1).as_str() == "property"
                        && matches!(
                            p.peek().as_str(),
                            "in" | "out" | "in_out" | "in-out" | "private"
                        ) =>
                {
                    parse_property_declaration(&mut *p);
                }
                _ if p.peek().as_str() == "if" => {
                    parse_if_element(&mut *p);
                }
                SyntaxKind::LBracket if p.peek().as_str() == "states" => {
                    parse_states(&mut *p);
                }
                SyntaxKind::LBracket if p.peek().as_str() == "transitions" => {
                    parse_transitions(&mut *p);
                }
                _ => {
                    if p.peek().as_str() == "changed" {
                        // Try to recover some errors
                        parse_changed_callback(&mut *p);
                    } else {
                        p.consume();
                        if !had_parse_error {
                            p.error("Parse error");
                            had_parse_error = true;
                        }
                    }
                }
            },
            SyntaxKind::At => {
                let checkpoint = p.checkpoint();
                p.consume();
                if p.peek().as_str() == "children" {
                    let mut p =
                        p.start_node_at(checkpoint.clone(), SyntaxKind::ChildrenPlaceholder);
                    p.consume()
                } else {
                    p.test(SyntaxKind::Identifier);
                    p.error("Parse error: Expected @children")
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
fn parse_sub_element(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::SubElement);
    if p.nth(1).kind() == SyntaxKind::ColonEqual {
        p.expect(SyntaxKind::Identifier);
        p.expect(SyntaxKind::ColonEqual);
    }
    parse_element(&mut *p)
}

#[cfg_attr(test, parser_test)]
/// ```test,RepeatedElement
/// for xx in mm: Elem { }
/// for [idx] in mm: Elem { }
/// for xx [idx] in foo.bar: Elem { }
/// for _ in (xxx()): blah := Elem { Elem{} }
/// ```
/// Must consume at least one token
fn parse_repeated_element(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "for");
    let mut p = p.start_node(SyntaxKind::RepeatedElement);
    p.expect(SyntaxKind::Identifier); // "for"
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
        drop(p.start_node(SyntaxKind::Expression));
        drop(p.start_node(SyntaxKind::SubElement).start_node(SyntaxKind::Element));
        return;
    }
    p.consume(); // "in"
    parse_expression(&mut *p);
    p.expect(SyntaxKind::Colon);
    parse_sub_element(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test,ConditionalElement
/// if (condition) : Elem { }
/// if (foo ? bar : xx) : Elem { foo:bar; Elem {}}
/// if (true) : foo := Elem {}
/// if true && true : Elem {}
/// ```
/// Must consume at least one token
fn parse_if_element(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "if");
    let mut p = p.start_node(SyntaxKind::ConditionalElement);
    p.expect(SyntaxKind::Identifier); // "if"
    parse_expression(&mut *p);
    if !p.expect(SyntaxKind::Colon) {
        drop(p.start_node(SyntaxKind::SubElement).start_node(SyntaxKind::Element));
        return;
    }
    parse_sub_element(&mut *p);
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
    } else if parse_expression(&mut *p) {
        p.expect(SyntaxKind::Semicolon)
    } else {
        p.test(SyntaxKind::Semicolon);
        false
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
/// clicked => bar ;
/// clicked => { foo; } ;
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
    if p.nth(0).kind() == SyntaxKind::LBrace && p.nth(2).kind() != SyntaxKind::Colon {
        parse_code_block(&mut *p);
        p.test(SyntaxKind::Semicolon);
    } else if parse_expression(&mut *p) {
        p.expect(SyntaxKind::Semicolon);
    } else {
        p.test(SyntaxKind::Semicolon);
    }
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
/// callback foo(foo: int, string, xx: { a: string });
/// pure callback one_arg({ a: string, b: string});
/// callback end_coma(a, b, c,);
/// callback with_return(a, b) -> int;
/// callback with_return2({a: string}) -> { a: string };
/// callback foobar <=> elem.foobar;
/// ```
/// Must consume at least one token
fn parse_callback_declaration(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::CallbackDeclaration);
    if p.peek().as_str() == "pure" {
        p.consume();
    }
    debug_assert_eq!(p.peek().as_str(), "callback");
    p.expect(SyntaxKind::Identifier); // "callback"
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    if p.test(SyntaxKind::LParent) {
        while p.peek().kind() != SyntaxKind::RParent {
            {
                let mut p = p.start_node(SyntaxKind::CallbackDeclarationParameter);
                if p.peek().kind() == SyntaxKind::Identifier && p.nth(1).kind() == SyntaxKind::Colon
                {
                    {
                        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
                        p.expect(SyntaxKind::Identifier);
                    }
                    p.expect(SyntaxKind::Colon);
                }
                parse_type(&mut *p);
            }
            if !p.test(SyntaxKind::Comma) {
                break;
            }
        }
        p.expect(SyntaxKind::RParent);
        if p.test(SyntaxKind::Arrow) {
            let mut p = p.start_node(SyntaxKind::ReturnType);
            parse_type(&mut *p);
        }

        if p.peek().kind() == SyntaxKind::DoubleArrow {
            p.error("When declaring a callback alias, one must omit parentheses. e.g. 'callback foo <=> other.bar;'");
        }
    } else if p.test(SyntaxKind::Arrow) {
        // Force callback with return value to also have parentheses, we could remove this
        // restriction in the future
        p.error("Callback with return value must be declared with parentheses e.g. 'callback foo() -> int;'");
        parse_type(&mut *p);
    }

    if p.peek().kind() == SyntaxKind::DoubleArrow {
        let mut p = p.start_node(SyntaxKind::TwoWayBinding);
        p.expect(SyntaxKind::DoubleArrow);
        parse_expression(&mut *p);
    }

    p.expect(SyntaxKind::Semicolon);
}

#[cfg_attr(test, parser_test)]
/// ```test,PropertyDeclaration
/// in property <int> xxx;
/// property<int> foobar;
/// property<string> text: "Something";
/// property<string> text <=> two.way;
/// property alias <=> two.way;
/// ```
fn parse_property_declaration(p: &mut impl Parser) {
    let checkpoint = p.checkpoint();
    while matches!(p.peek().as_str(), "in" | "out" | "in-out" | "in_out" | "private") {
        p.consume();
    }
    if p.peek().as_str() != "property" {
        p.error("Expected 'property' keyword");
        return;
    }
    let mut p = p.start_node_at(checkpoint, SyntaxKind::PropertyDeclaration);
    p.consume(); // property

    if p.test(SyntaxKind::LAngle) {
        parse_type(&mut *p);
        p.expect(SyntaxKind::RAngle);
    } else if p.nth(0).kind() == SyntaxKind::Identifier
        && p.nth(1).kind() != SyntaxKind::DoubleArrow
    {
        p.error("Missing type. The syntax to declare a property is `property <type> name;`. Only two way bindings can omit the type");
    }

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
    p.expect(SyntaxKind::Identifier); // animate
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
/// ```test,PropertyChangedCallback
/// changed the-property => { x = y; }
/// ```
fn parse_changed_callback(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "changed");
    let mut p = p.start_node(SyntaxKind::PropertyChangedCallback);
    p.expect(SyntaxKind::Identifier); // changed
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    p.expect(SyntaxKind::FatArrow);
    parse_code_block(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test,States
/// states []
/// states [ foo when bar : { x:y; } another_state : { x:z; }]
/// ```
fn parse_states(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "states");
    let mut p = p.start_node(SyntaxKind::States);
    p.expect(SyntaxKind::Identifier); // "states"
    p.expect(SyntaxKind::LBracket);
    while parse_state(&mut *p) {}
    p.expect(SyntaxKind::RBracket);
}

#[cfg_attr(test, parser_test)]
/// ```test,State
/// foo : { x: 1px + 2px; aaa.y: {1px + 2px} }
/// foo when bar == 1:  { color: blue; foo.color: red;   }
/// a when b:  { color: blue; in { animate color { duration: 120s; } }   }
/// a when b:  { out { animate foo.bar { } } foo.bar: 42;  }
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
                if p.nth(1).kind() == SyntaxKind::LBrace
                    && matches!(p.peek().as_str(), "in" | "out" | "in-out" | "in_out")
                {
                    let mut p = p.start_node(SyntaxKind::Transition);
                    p.consume(); // "in", "out" or "in-out"
                    p.expect(SyntaxKind::LBrace);
                    if !parse_transition_inner(&mut *p) {
                        return false;
                    }
                    continue;
                };
                let checkpoint = p.checkpoint();
                if !parse_qualified_name(&mut *p)
                    || !p.expect(SyntaxKind::Colon)
                    || !parse_binding_expression(&mut *p)
                {
                    p.test(SyntaxKind::RBrace);
                    return false;
                }
                let _ = p.start_node_at(checkpoint, SyntaxKind::StatePropertyChange);
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,Transitions
/// transitions []
/// transitions [in checked: {animate x { duration: 88ms; }} out checked: {animate x { duration: 88ms; }} in-out checked: {animate x { duration: 88ms; }}]
/// ```
fn parse_transitions(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "transitions");
    let mut p = p.start_node(SyntaxKind::Transitions);
    p.expect(SyntaxKind::Identifier); // "transitions"
    p.expect(SyntaxKind::LBracket);
    while p.nth(0).kind() != SyntaxKind::RBracket && parse_transition(&mut *p) {}
    p.expect(SyntaxKind::RBracket);
}

#[cfg_attr(test, parser_test)]
/// ```test,Transition
/// in pressed : {}
/// in pressed: { animate x { duration: 88ms; } }
/// out pressed: { animate x { duration: 88ms; } }
/// in-out pressed: { animate x { duration: 88ms; } }
/// ```
fn parse_transition(p: &mut impl Parser) -> bool {
    if !matches!(p.peek().as_str(), "in" | "out" | "in-out" | "in_out") {
        p.error("Expected 'in', 'out', or 'in-out' to declare a transition");
        return false;
    }
    let mut p = p.start_node(SyntaxKind::Transition);
    p.consume(); // "in", "out" or "in-out"
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    p.expect(SyntaxKind::Colon);
    if !p.expect(SyntaxKind::LBrace) {
        return false;
    }
    parse_transition_inner(&mut *p)
}

#[cfg_attr(test, parser_test)]
/// ```test
/// }
/// animate x { duration: 88ms; }  animate foo.bar { } }
/// ```
fn parse_transition_inner(p: &mut impl Parser) -> bool {
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
/// ```test,Function
/// function foo() {}
/// function bar(xx : int) { yy = xx; }
/// function bar(xx : int,) -> int { return 42; }
/// public function aa(x: int, b: {a: int}, c: int) {}
/// protected pure function fff() {}
/// ```
fn parse_function(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::Function);
    if matches!(p.peek().as_str(), "public" | "protected") {
        p.consume();
        if p.peek().as_str() == "pure" {
            p.consume()
        }
    } else if p.peek().as_str() == "pure" {
        p.consume();
        if matches!(p.peek().as_str(), "public" | "protected") {
            p.consume()
        }
    }
    if p.peek().as_str() != "function" {
        p.error("Unexpected identifier");
        p.consume();
        while p.peek().kind == SyntaxKind::Identifier && p.peek().as_str() != "function" {
            p.consume();
        }
    }
    debug_assert_eq!(p.peek().as_str(), "function");
    p.expect(SyntaxKind::Identifier); // "function"
    {
        let mut p = p.start_node(SyntaxKind::DeclaredIdentifier);
        p.expect(SyntaxKind::Identifier);
    }
    if p.expect(SyntaxKind::LParent) {
        while p.peek().kind() != SyntaxKind::RParent {
            let mut p_arg = p.start_node(SyntaxKind::ArgumentDeclaration);
            {
                let mut p = p_arg.start_node(SyntaxKind::DeclaredIdentifier);
                p.expect(SyntaxKind::Identifier);
            }
            p_arg.expect(SyntaxKind::Colon);
            parse_type(&mut *p_arg);
            drop(p_arg);
            if !p.test(SyntaxKind::Comma) {
                break;
            }
        }
        p.expect(SyntaxKind::RParent);
        if p.test(SyntaxKind::Arrow) {
            let mut p = p.start_node(SyntaxKind::ReturnType);
            parse_type(&mut *p);
        }
    }
    parse_code_block(&mut *p);
}
