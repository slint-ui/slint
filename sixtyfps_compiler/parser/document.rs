use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test
/// Type = Base { }
/// ```
pub fn parse_document(p: &mut Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Document);
    let mut p = p.start_node(SyntaxKind::Component);

    if !(p.expect(SyntaxKind::Identifier) && p.expect(SyntaxKind::Equal)) {
        return false;
    }

    if !parse_element(&mut *p) {
        return false;
    }

    if p.peek_kind() != SyntaxKind::Eof {
        p.error("Should be end of file");
        return false;
    }
    true
}

#[cfg_attr(test, parser_test)]
/// ```test
/// Item { }
/// ```
pub fn parse_element(p: &mut Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Element);
    if !(p.expect(SyntaxKind::Identifier) && p.expect(SyntaxKind::LBrace)) {
        return false;
    }

    parse_element_content(&mut *p);

    p.expect(SyntaxKind::RBrace)
}

fn parse_element_content(p: &mut Parser) {
    loop {
        match p.peek_kind() {
            SyntaxKind::RBrace => return,
            SyntaxKind::Eof => return,
            SyntaxKind::Identifier => {
                let mut p = p.start_node(SyntaxKind::Binding);
                p.consume();
                p.expect(SyntaxKind::Colon);
                parse_code_statement(&mut *p);
            }
            // TODO: right now this only parse bindings
            _ => {
                p.consume();
                p.error("FIXME");
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test
/// {  }
/// expression ;
/// ```
fn parse_code_statement(p: &mut Parser) {
    let mut p = p.start_node(SyntaxKind::CodeStatement);
    match p.peek_kind() {
        SyntaxKind::LBrace => parse_code_block(&mut *p),
        _ => {
            parse_expression(&mut *p);
            p.expect(SyntaxKind::Semicolon);
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test
/// {  }
/// ```
fn parse_code_block(p: &mut Parser) {
    let mut p = p.start_node(SyntaxKind::CodeBlock);
    p.expect(SyntaxKind::LBrace); // Or assert?

    // FIXME

    p.until(SyntaxKind::RBrace);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// something
/// ```
fn parse_expression(p: &mut Parser) {
    let mut p = p.start_node(SyntaxKind::Expression);
    p.expect(SyntaxKind::Identifier);
}
