/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use super::document::parse_qualified_name;
use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test,Expression
/// something
/// "something"
/// 0.3
/// 42
/// 42px
/// #aabbcc
/// (something)
/// @image-url("something")
/// some_id.some_property
/// function_call()
/// function_call(hello, world)
/// cond ? first : second
/// call_cond() ? first : second
/// (nested()) ? (ok) : (other.ko)
/// 4 + 4
/// 4 + 8 * 7 / 5 + 3 - 7 - 7 * 8
/// -0.3px + 0.3px - 3.pt+3pt
/// aa == cc && bb && (xxx || fff) && 3 + aaa == bbb
/// [array]
/// {object:42}
/// ```
pub fn parse_expression(p: &mut impl Parser) {
    parse_expression_helper(p, OperatorPrecedence::Default)
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
enum OperatorPrecedence {
    /// ` ?: `
    Default,
    /// `||`, `&&`
    Logical,
    /// `==` `!=` `>=` `<=` `<` `>`
    Equality,
    /// `+ -`
    Add,
    /// `* /`
    Mul,
    Unary,
    Bang,
}

fn parse_expression_helper(p: &mut impl Parser, precedence: OperatorPrecedence) {
    let mut p = p.start_node(SyntaxKind::Expression);
    let checkpoint = p.checkpoint();
    match p.nth(0).kind() {
        SyntaxKind::Identifier => {
            if p.nth(1).kind() == SyntaxKind::Bang {
                parse_bang_expression(&mut *p)
            } else {
                parse_qualified_name(&mut *p);
            }
        }
        SyntaxKind::StringLiteral => {
            if p.nth(0).as_str().ends_with('{') {
                parse_template_string(&mut *p)
            } else {
                p.consume()
            }
        }
        SyntaxKind::NumberLiteral => p.consume(),
        SyntaxKind::ColorLiteral => p.consume(),
        SyntaxKind::LParent => {
            p.consume();
            parse_expression(&mut *p);
            p.expect(SyntaxKind::RParent);
        }
        SyntaxKind::LBracket => parse_array(&mut *p),
        SyntaxKind::LBrace => parse_object_notation(&mut *p),
        SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::Bang => {
            let mut p = p.start_node(SyntaxKind::UnaryOpExpression);
            p.consume();
            parse_expression_helper(&mut *p, OperatorPrecedence::Unary);
        }
        SyntaxKind::At => {
            parse_at_keyword(&mut *p);
        }
        _ => {
            p.error("invalid expression");
            return;
        }
    }

    if p.nth(0).kind() == SyntaxKind::LParent {
        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::FunctionCallExpression);
        parse_function_arguments(&mut *p);
    }

    if precedence >= OperatorPrecedence::Mul {
        return;
    }

    while matches!(p.nth(0).kind(), SyntaxKind::Star | SyntaxKind::Div) {
        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::BinaryExpression);
        p.consume();
        parse_expression_helper(&mut *p, OperatorPrecedence::Mul);
    }

    if precedence >= OperatorPrecedence::Add {
        return;
    }

    while matches!(p.nth(0).kind(), SyntaxKind::Plus | SyntaxKind::Minus) {
        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::BinaryExpression);
        p.consume();
        parse_expression_helper(&mut *p, OperatorPrecedence::Add);
    }

    if precedence > OperatorPrecedence::Equality {
        return;
    }

    if matches!(
        p.nth(0).kind(),
        SyntaxKind::LessEqual
            | SyntaxKind::GreaterEqual
            | SyntaxKind::EqualEqual
            | SyntaxKind::NotEqual
            | SyntaxKind::LAngle
            | SyntaxKind::RAngle
    ) {
        if precedence == OperatorPrecedence::Equality {
            p.error("Use parentheses to disambiguate equality expression on the same level");
        }

        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::BinaryExpression);
        p.consume();
        parse_expression_helper(&mut *p, OperatorPrecedence::Equality);
    }

    if precedence >= OperatorPrecedence::Logical {
        return;
    }

    let mut prev_logical_op = None;
    while matches!(p.nth(0).kind(), SyntaxKind::AndAnd | SyntaxKind::OrOr) {
        if let Some(prev) = prev_logical_op {
            if prev != p.nth(0).kind() {
                p.error("Use parentheses to disambiguate between && and ||");
                prev_logical_op = None;
            }
        } else {
            prev_logical_op = Some(p.nth(0).kind());
        }

        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::BinaryExpression);
        p.consume();
        parse_expression_helper(&mut *p, OperatorPrecedence::Logical);
    }

    if p.nth(0).kind() == SyntaxKind::Question {
        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint, SyntaxKind::ConditionalExpression);
        p.consume();
        parse_expression(&mut *p);
        p.expect(SyntaxKind::Colon);
        parse_expression(&mut *p);
    }
}

#[cfg_attr(test, parser_test)]
/// ```test
/// @image-url("/foo/bar.png")
/// ```
fn parse_at_keyword(p: &mut impl Parser) {
    let checkpoint = p.checkpoint();
    p.expect(SyntaxKind::At);
    match p.peek().as_str() {
        "image-url" | "image_url" => {
            let mut p = p.start_node_at(checkpoint, SyntaxKind::AtImageUrl);
            p.consume(); // "image-url"
            p.expect(SyntaxKind::LParent);
            p.expect(SyntaxKind::StringLiteral);
            p.expect(SyntaxKind::RParent);
        }
        _ => {
            p.error("Expected 'image-url' after '@'");
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,BangExpression
/// foo!bar
/// foo!(bar)
/// foo!("bar")
/// foo ! "bar"
/// foo ! plop ! bar
/// foo ! (plop ! bar)
/// ```
fn parse_bang_expression(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::BangExpression);
    p.expect(SyntaxKind::Identifier); // Or assert?
    p.expect(SyntaxKind::Bang); // Or assert?
    parse_expression_helper(&mut *p, OperatorPrecedence::Bang);
}

#[cfg_attr(test, parser_test)]
/// ```test,Array
/// [ a, b, c , d]
/// []
/// [a,]
/// [ [], [] ]
/// ```
fn parse_array(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::Array);
    p.expect(SyntaxKind::LBracket);

    while p.nth(0).kind() != SyntaxKind::RBracket {
        parse_expression(&mut *p);
        if !p.test(SyntaxKind::Comma) {
            break;
        }
    }
    p.expect(SyntaxKind::RBracket);
}

#[cfg_attr(test, parser_test)]
/// ```test,ObjectLiteral
/// {}
/// {a:b}
/// { a: "foo" , }
/// {a:b, c: 4 + 4, d: [a,] }
/// ```
fn parse_object_notation(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::ObjectLiteral);
    p.expect(SyntaxKind::LBrace);

    while p.nth(0).kind() != SyntaxKind::RBrace {
        let mut p = p.start_node(SyntaxKind::ObjectMember);
        p.expect(SyntaxKind::Identifier);
        p.expect(SyntaxKind::Colon);
        parse_expression(&mut *p);
        if !p.test(SyntaxKind::Comma) {
            break;
        }
    }
    p.expect(SyntaxKind::RBrace);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// ()
/// (foo)
/// (foo, bar, foo)
/// (foo, bar(), xx+xx,)
/// ```
fn parse_function_arguments(p: &mut impl Parser) {
    p.expect(SyntaxKind::LParent);

    while p.nth(0).kind() != SyntaxKind::RParent {
        parse_expression(&mut *p);
        if !p.test(SyntaxKind::Comma) {
            break;
        }
    }
    p.expect(SyntaxKind::RParent);
}

#[cfg_attr(test, parser_test)]
/// ```test,StringTemplate
/// "foo\{bar}"
/// "foo\{4 + 5}foo"
/// ```
fn parse_template_string(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::StringTemplate);
    debug_assert!(p.nth(0).as_str().ends_with("\\{"));
    {
        let mut p = p.start_node(SyntaxKind::Expression);
        p.consume();
    }
    loop {
        parse_expression(&mut *p);
        let peek = p.peek();
        if peek.kind != SyntaxKind::StringLiteral || !peek.as_str().starts_with('}') {
            p.error("Error while parsing string template")
        }
        let mut p = p.start_node(SyntaxKind::Expression);
        let cont = peek.as_str().ends_with('{');
        p.consume();
        if !cont {
            break;
        }
    }
}
