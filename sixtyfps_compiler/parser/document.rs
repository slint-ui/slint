use super::prelude::*;

#[parser_test]
/// ```test
/// Type = Base { }
/// ```
pub fn parse_document(p: &mut Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Document);
    let mut p = p.start_node(SyntaxKind::Component);

    if !(p.expect(SyntaxKind::Identifier)
        && p.expect(SyntaxKind::Equal)
        && p.expect(SyntaxKind::Identifier)
        && p.expect(SyntaxKind::LBrace))
    {
        return false;
    }

    parse_element_content(p.0);

    p.expect(SyntaxKind::RBrace);
    if p.peek().0 != SyntaxKind::Eof {
        p.error("Should be end of file");
        return false;
    }
    return true;
}

fn parse_element_content(p: &mut Parser) {
    loop {
        match p.peek().0 {
            SyntaxKind::RBrace => return,
            SyntaxKind::Eof => return,
            SyntaxKind::Identifier => {
                let mut p = p.start_node(SyntaxKind::Binding);
                p.consume();
                p.expect(SyntaxKind::Colon);
                parse_code_statement(p.0);
            }
            // TODO: right now this only parse bindings
            _ => {
                p.consume();
                p.error("FIXME");
            }
        }
    }
}

#[parser_test]
/// ```test
/// {  }
/// expression ;
/// ```
fn parse_code_statement(p: &mut Parser) {
    let mut p = p.start_node(SyntaxKind::CodeStatement);
    match p.peek().0 {
        SyntaxKind::LBrace => parse_code_block(p.0),
        _ => {
            parse_expression(p.0);
            p.expect(SyntaxKind::Semicolon);
        }
    }
}

fn parse_code_block(p: &mut Parser) {
    let mut p = p.start_node(SyntaxKind::CodeBlock);
    p.expect(SyntaxKind::LBrace); // Or assert?

    // FIXME

    p.until(SyntaxKind::RBrace);
}

fn parse_expression(p: &mut Parser) {
    let mut p = p.start_node(SyntaxKind::Expression);
    p.expect(SyntaxKind::Identifier);
}
