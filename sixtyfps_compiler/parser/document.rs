use super::expressions::parse_expression;
use super::prelude::*;

#[cfg_attr(test, parser_test)]
/// ```test
/// Type := Base { }
/// Type := Base { SubElement { } }
/// component Comp := Base {}  Type := Base {}
/// ```
pub fn parse_document(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Document);

    loop {
        if p.peek().as_str() == "component" && p.nth(1) != SyntaxKind::ColonEqual {
            p.expect(SyntaxKind::Identifier);
        }

        if !parse_component(&mut *p) {
            return false;
        }

        if p.nth(0) == SyntaxKind::Eof {
            return true;
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test
/// Type := Base { }
/// Type := Base { prop: value; }
/// Type := Base { SubElement { } }
/// ```
pub fn parse_component(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Component);
    if !(p.expect(SyntaxKind::Identifier) && p.expect(SyntaxKind::ColonEqual)) {
        return false;
    }

    if !parse_element(&mut *p) {
        return false;
    }
    true
}

#[cfg_attr(test, parser_test)]
/// ```test
/// Item { }
/// Item { property: value; SubElement { } }
/// ```
pub fn parse_element(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::Element);
    if !(parse_qualified_type_name(&mut *p) && p.expect(SyntaxKind::LBrace)) {
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
/// clicked => {}
/// signal foobar;
/// property<int> width;
/// ```
fn parse_element_content(p: &mut impl Parser) {
    loop {
        match p.nth(0) {
            SyntaxKind::RBrace => return,
            SyntaxKind::Eof => return,
            SyntaxKind::Identifier => match p.nth(1) {
                SyntaxKind::Colon => parse_property_binding(&mut *p),
                SyntaxKind::ColonEqual | SyntaxKind::LBrace => parse_sub_element(&mut *p),
                SyntaxKind::FatArrow => parse_signal_connection(&mut *p),
                SyntaxKind::Identifier if p.peek().as_str() == "for" => {
                    parse_repeated_element(&mut *p);
                }
                SyntaxKind::Identifier if p.peek().as_str() == "signal" => {
                    parse_signal_declaration(&mut *p);
                }
                SyntaxKind::LAngle if p.peek().as_str() == "property" => {
                    parse_property_declaration(&mut *p);
                }
                _ => {
                    p.consume();
                    p.error("FIXME");
                }
            },
            _ => {
                p.consume();
                p.error("FIXME");
            }
        }
    }
}

#[cfg_attr(test, parser_test)]
/// ```test
/// Bar {}
/// foo := Bar {}
/// Bar { x : y ; }
/// ```
/// Must consume at least one token
fn parse_sub_element(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::SubElement);
    if p.nth(1) == SyntaxKind::ColonEqual {
        assert!(p.expect(SyntaxKind::Identifier));
        p.expect(SyntaxKind::ColonEqual);
    }
    parse_element(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// for xx in mm: Elem { }
/// ```
/// Must consume at least one token
fn parse_repeated_element(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "for");
    let mut p = p.start_node(SyntaxKind::RepeatedElement);
    p.consume();
    if !(p.expect(SyntaxKind::Identifier) && p.peek().as_str() == "in") {
        return;
    }
    p.consume(); // "in"
    parse_expression(&mut *p);
    p.expect(SyntaxKind::Colon);
    parse_element(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// Rectangle
/// MyModule.Rectangle
/// Deeply.Nested.MyModule.Rectangle
/// ```
fn parse_qualified_type_name(p: &mut impl Parser) -> bool {
    let mut p = p.start_node(SyntaxKind::QualifiedTypeName);
    if !p.expect(SyntaxKind::Identifier) {
        return false;
    }

    loop {
        if p.nth(0) != SyntaxKind::Dot {
            break;
        }
        p.consume();
        p.expect(SyntaxKind::Identifier);
    }

    return true;
}

#[cfg_attr(test, parser_test)]
/// ```test
/// foo: bar;
/// foo: {}
/// ```
fn parse_property_binding(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::Binding);
    p.consume();
    p.expect(SyntaxKind::Colon);
    parse_code_statement(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// {  }
/// expression ;
/// ```
fn parse_code_statement(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::CodeStatement);
    match p.nth(0) {
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
/// { expression }
/// ```
fn parse_code_block(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::CodeBlock);
    p.expect(SyntaxKind::LBrace); // Or assert?

    if p.nth(0) != SyntaxKind::RBrace {
        parse_expression(&mut *p);
    }

    p.until(SyntaxKind::RBrace);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// clicked => {}
/// ```
fn parse_signal_connection(p: &mut impl Parser) {
    let mut p = p.start_node(SyntaxKind::SignalConnection);
    p.consume(); // the identifier
    p.expect(SyntaxKind::FatArrow);
    parse_code_block(&mut *p);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// signal foobar;
/// ```
/// Must consume at least one token
fn parse_signal_declaration(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "signal");
    let mut p = p.start_node(SyntaxKind::SignalDeclaration);
    p.consume(); // "signal"
    p.expect(SyntaxKind::Identifier);
    p.expect(SyntaxKind::Semicolon);
}

#[cfg_attr(test, parser_test)]
/// ```test
/// property<int> foobar;
/// ```
fn parse_property_declaration(p: &mut impl Parser) {
    debug_assert_eq!(p.peek().as_str(), "property");
    let mut p = p.start_node(SyntaxKind::PropertyDeclaration);
    p.consume(); // property
    p.expect(SyntaxKind::LAngle);
    parse_qualified_type_name(&mut *p);
    p.expect(SyntaxKind::RAngle);
    p.expect(SyntaxKind::Identifier);
    p.expect(SyntaxKind::Semicolon);
}
