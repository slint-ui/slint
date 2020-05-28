use super::expressions::parse_expression;
use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test
/// expression
/// ```
pub fn parse_statement(p: &mut impl Parser) {
    if matches!(p.nth(0), SyntaxKind::Semicolon | SyntaxKind::RBrace) {
        return;
    }
    parse_expression(p);
}
