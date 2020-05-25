use super::document::parse_qualified_name;
use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test
/// something
/// "something"
/// 0.3
/// 42
/// (something)
/// img!"something"
/// some_id.some_property
/// ```
pub fn parse_expression(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::Expression);
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
        SyntaxKind::LParent => {
            p.consume();
            parse_expression(&mut *p);
            p.expect(SyntaxKind::RParent);
        }
        _ => p.error("invalid expression"),
    }
}

#[cfg_attr(test, parser_test)]
/// ```test
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
    parse_expression(&mut *p);
}
