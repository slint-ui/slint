use super::expressions::parse_expression;
use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test
/// expression
/// expression += expression
/// expression.expression *= 45.2
/// expression = "hello"
/// ```
pub fn parse_statement(p: &mut impl Parser) {
    if matches!(p.nth(0), SyntaxKind::Semicolon | SyntaxKind::RBrace) {
        return;
    }
    let checkpoint = p.checkpoint();
    parse_expression(p);
    if matches!(
        p.nth(0),
        SyntaxKind::MinusEqual
            | SyntaxKind::PlusEqual
            | SyntaxKind::StarEqual
            | SyntaxKind::DivEqual
            | SyntaxKind::Equal
    ) {
        let mut p = p.start_node_at(checkpoint.clone(), SyntaxKind::Expression);
        let mut p = p.start_node_at(checkpoint, SyntaxKind::SelfAssignment);
        p.consume();
        parse_expression(&mut *p);
    }
}
