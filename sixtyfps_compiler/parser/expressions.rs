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
/// ```
pub fn parse_expression(p: &mut impl Parser) {
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
    parse_expression(&mut *p);
}
