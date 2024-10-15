// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
/// (something).something
/// @image-url("something")
/// @image_url("something")
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
/// array[index]
/// {object:42}
/// "foo".bar.something().something.xx({a: 1.foo}.a)
/// ```
pub fn parse_expression(p: &mut impl Parser) -> bool {
    p.peek(); // consume the whitespace so they aren't part of the Expression node
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
}

fn parse_expression_helper(p: &mut impl Parser, precedence: OperatorPrecedence) -> bool {
    let mut p = p.start_node(SyntaxKind::Expression);
    let checkpoint = p.checkpoint();
    match p.nth(0).kind() {
        SyntaxKind::Identifier => {
            parse_qualified_name(&mut *p);
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
            return false;
        }
    }

    loop {
        match p.nth(0).kind() {
            SyntaxKind::Dot => {
                {
                    let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
                }
                let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::MemberAccess);
                p.consume(); // '.'
                if !p.expect(SyntaxKind::Identifier) {
                    return false;
                }
            }
            SyntaxKind::LParent => {
                {
                    let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
                }
                let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::FunctionCallExpression);
                parse_function_arguments(&mut *p);
            }
            SyntaxKind::LBracket => {
                {
                    let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
                }
                let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::IndexExpression);
                p.expect(SyntaxKind::LBracket);
                parse_expression(&mut *p);
                p.expect(SyntaxKind::RBracket);
            }
            _ => break,
        }
    }

    if precedence >= OperatorPrecedence::Mul {
        return true;
    }

    while matches!(p.nth(0).kind(), SyntaxKind::Star | SyntaxKind::Div) {
        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::BinaryExpression);
        p.consume();
        parse_expression_helper(&mut *p, OperatorPrecedence::Mul);
    }

    if p.nth(0).kind() == SyntaxKind::Percent {
        p.error("Unexpected '%'. For the unit, it should be attached to the number. If you're looking for the modulo operator, use the 'Math.mod(x, y)' function");
        p.consume();
        return false;
    }

    if precedence >= OperatorPrecedence::Add {
        return true;
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
        return true;
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
        return true;
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
    true
}

#[cfg_attr(test, parser_test)]
/// ```test
/// @image-url("/foo/bar.png")
/// @linear-gradient(0deg, blue, red)
/// @tr("foo", bar)
/// ```
fn parse_at_keyword(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().kind(), SyntaxKind::At);
    match p.nth(1).as_str() {
        "image-url" | "image_url" => {
            parse_image_url(p);
        }
        "linear-gradient" | "linear_gradient" => {
            parse_gradient(p);
        }
        "radial-gradient" | "radial_gradient" => {
            parse_gradient(p);
        }
        "tr" => {
            parse_tr(p);
        }
        _ => {
            p.consume();
            p.test(SyntaxKind::Identifier); // consume the identifier, so that autocomplete works
            p.error("Expected 'image-url', 'tr', 'linear-gradient' or 'radial-gradient' after '@'");
        }
    }
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
        p.expect(SyntaxKind::StringLiteral);
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

#[cfg_attr(test, parser_test)]
/// ```test,AtGradient
/// @linear-gradient(#e66465, #9198e5)
/// @linear-gradient(0.25turn, #3f87a6, #ebf8e1, #f69d3c)
/// @linear-gradient(to left, #333, #333 50%, #eee 75%, #333 75%)
/// @linear-gradient(217deg, rgba(255,0,0,0.8), rgba(255,0,0,0) 70.71%)
/// @linear_gradient(217deg, rgba(255,0,0,0.8), rgba(255,0,0,0) 70.71%)
/// @radial-gradient(circle, #e66465, blue 50%, #9198e5)
/// ```
fn parse_gradient(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::AtGradient);
    p.expect(SyntaxKind::At);
    debug_assert!(p.peek().as_str().ends_with("gradient"));
    p.expect(SyntaxKind::Identifier); //eg "linear-gradient"

    p.expect(SyntaxKind::LParent);

    while !p.test(SyntaxKind::RParent) {
        if !parse_expression(&mut *p) {
            return;
        }
        p.test(SyntaxKind::Comma);
    }
}

#[cfg_attr(test, parser_test)]
/// ```test,AtTr
/// @tr("foo")
/// @tr("foo{0}", bar(42))
/// @tr("context" => "ccc{}", 0)
/// @tr("xxx" => "ccc{n}" | "ddd{}" % 42, 45)
/// ```
fn parse_tr(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::AtTr);
    p.expect(SyntaxKind::At);
    debug_assert_eq!(p.peek().as_str(), "tr");
    p.expect(SyntaxKind::Identifier); //"tr"
    p.expect(SyntaxKind::LParent);

    let checkpoint = p.checkpoint();

    fn consume_literal(p: &mut impl Parser) -> bool {
        let peek = p.peek();
        if peek.kind() != SyntaxKind::StringLiteral
            || !peek.as_str().starts_with('"')
            || !peek.as_str().ends_with('"')
        {
            p.error("Expected plain string literal");
            return false;
        }
        p.expect(SyntaxKind::StringLiteral)
    }

    if !consume_literal(&mut *p) {
        return;
    }

    if p.test(SyntaxKind::FatArrow) {
        drop(p.start_node_at(checkpoint, SyntaxKind::TrContext));
        if !consume_literal(&mut *p) {
            return;
        }
    }

    if p.peek().kind() == SyntaxKind::Pipe {
        let mut p = p.start_node(SyntaxKind::TrPlural);
        p.consume();
        if !consume_literal(&mut *p) || !p.expect(SyntaxKind::Percent) {
            let _ = p.start_node(SyntaxKind::Expression);
            return;
        }
        parse_expression(&mut *p);
    }

    while p.test(SyntaxKind::Comma) {
        if !parse_expression(&mut *p) {
            break;
        }
    }
    p.expect(SyntaxKind::RParent);
}

#[cfg_attr(test, parser_test)]
/// ```test,AtImageUrl
/// @image-url("foo.png")
/// @image-url("foo.png",)
/// @image-url("foo.png", nine-slice(1 2 3 4))
/// @image-url("foo.png", nine-slice(1))
/// ```
fn parse_image_url(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::AtImageUrl);
    p.consume(); // "@"
    p.consume(); // "image-url"
    if !(p.expect(SyntaxKind::LParent)) {
        return;
    }
    let peek = p.peek();
    if peek.kind() != SyntaxKind::StringLiteral {
        p.error("@image-url must contain a plain path as a string literal");
        p.until(SyntaxKind::RParent);
        return;
    }
    if !peek.as_str().starts_with('"') || !peek.as_str().ends_with('"') {
        p.error("@image-url must contain a plain path as a string literal, without any '\\{}' expressions");
        p.until(SyntaxKind::RParent);
        return;
    }
    p.expect(SyntaxKind::StringLiteral);
    if !p.test(SyntaxKind::Comma) {
        if !p.test(SyntaxKind::RParent) {
            p.error("Expected ')' or ','");
            p.until(SyntaxKind::RParent);
        }
        return;
    }
    if p.test(SyntaxKind::RParent) {
        return;
    }
    if p.peek().as_str() != "nine-slice" {
        p.error("Expected 'nine-slice(...)' argument");
        p.until(SyntaxKind::RParent);
        return;
    }
    p.consume();
    if !p.expect(SyntaxKind::LParent) {
        p.until(SyntaxKind::RParent);
        return;
    }
    let mut count = 0;
    loop {
        match p.peek().kind() {
            SyntaxKind::RParent => {
                if count != 1 && count != 2 && count != 4 {
                    p.error("Expected 1 or 2 or 4 numbers");
                }
                p.consume();
                break;
            }
            SyntaxKind::NumberLiteral => {
                count += 1;
                p.consume();
            }
            SyntaxKind::Comma | SyntaxKind::Colon => {
                p.error("Arguments of nine-slice need to be separated by spaces");
                p.until(SyntaxKind::RParent);
                break;
            }
            _ => {
                p.error("Expected number literal or ')'");
                p.until(SyntaxKind::RParent);
                break;
            }
        }
    }
    if !p.expect(SyntaxKind::RParent) {
        p.until(SyntaxKind::RParent);
    }
}
