use super::document::parse_qualified_name;
use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test,Expression
/// something
/// "something"
/// 0.3
/// 42
/// #aabbcc
/// (something)
/// img!"something"
/// some_id.some_property
/// function_call()
/// cond ? first : second
/// call_cond() ? first : second
/// (nested()) ? (ok) : (other.ko)
/// 4 + 4
/// 4 + 8 * 7 / 5 + 3 - 7 - 7 * 8
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
    /// `==` `!=` `>=` `<=` `<` `>`
    Equality,
    /// `+ -`
    Add,
    /// `* /`
    Mul,
    Bang,
}

fn parse_expression_helper(p: &mut impl Parser, precedence: OperatorPrecedence) {
    let mut p = p.start_node(SyntaxKind::Expression);
    let checkpoint = p.checkpoint();
    match p.nth(0) {
        SyntaxKind::Identifier => {
            if p.nth(1) == SyntaxKind::Bang {
                parse_bang_expression(&mut *p)
            } else {
                parse_qualified_name(&mut *p);
            }
        }
        SyntaxKind::StringLiteral => p.consume(),
        SyntaxKind::NumberLiteral => p.consume(),
        SyntaxKind::ColorLiteral => p.consume(),
        SyntaxKind::LParent => {
            p.consume();
            parse_expression(&mut *p);
            p.expect(SyntaxKind::RParent);
        }
        SyntaxKind::LBracket => parse_array(&mut *p),
        SyntaxKind::LBrace => parse_object_notation(&mut *p),

        _ => {
            p.error("invalid expression");
            return;
        }
    }

    match p.nth(0) {
        SyntaxKind::LParent => {
            {
                let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
            }
            let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::FunctionCallExpression);

            p.consume();
            p.expect(SyntaxKind::RParent);
        }
        _ => {}
    }

    if precedence >= OperatorPrecedence::Mul {
        return;
    }

    while matches!(p.nth(0), SyntaxKind::Star | SyntaxKind::Div) {
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

    while matches!(p.nth(0), SyntaxKind::Plus | SyntaxKind::Minus) {
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

    while matches!(
        p.nth(0),
        SyntaxKind::LessEqual
            | SyntaxKind::GreaterEqual
            | SyntaxKind::EqualEqual
            | SyntaxKind::NotEqual
            | SyntaxKind::LAngle
            | SyntaxKind::RAngle
    ) {
        if precedence == OperatorPrecedence::Equality {
            p.error("Use parentheses to disambiguate equality expression on the same level");
            return;
        }

        {
            let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        }
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::BinaryExpression);
        p.consume();
        parse_expression_helper(&mut *p, OperatorPrecedence::Equality);
    }

    match p.nth(0) {
        SyntaxKind::Question => {
            {
                let _ = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
            }
            let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::ConditionalExpression);
            p.consume();
            parse_expression(&mut *p);
            p.expect(SyntaxKind::Colon);
            parse_expression(&mut *p);
        }
        _ => (),
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

    while p.nth(0) != SyntaxKind::RBracket {
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

    while p.nth(0) != SyntaxKind::RBrace {
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
