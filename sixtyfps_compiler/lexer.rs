/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! This module contains the code for the lexer.
//!
//! It is kind of shared with parser.rs, which implements the lex_next_token based on the macro_rules
//! that declares token

/// This trait is used by the `crate::parser::lex_next_token` function and is implemented
/// for rule passed to the macro which can be either a string literal, or a function
pub trait LexingRule {
    /// Return the size of the match for this rule, or 0 if there is no match
    fn lex(&self, text: &str) -> usize;
}

impl<'a> LexingRule for &'a str {
    #[inline]
    fn lex(&self, text: &str) -> usize {
        if text.starts_with(*self) {
            self.len()
        } else {
            0
        }
    }
}

impl<F: for<'r> Fn(&'r str) -> usize> LexingRule for F {
    #[inline]
    fn lex(&self, text: &str) -> usize {
        (self)(text)
    }
}

pub fn lex_whitespace(text: &str) -> usize {
    let mut len = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if !c.is_whitespace() {
            break;
        }
        len += c.len_utf8();
    }
    len
}

pub fn lex_comment(text: &str) -> usize {
    // FIXME: could report proper error if not properly terminated
    if text.starts_with("//") {
        return text.find('\n').unwrap_or(0);
    }
    if text.starts_with("/*") {
        let mut nested = 0;
        let mut offset = 2;
        let bytes = text.as_bytes();
        while offset < bytes.len() {
            if let Some(star) = bytes[offset..].iter().position(|c| *c == b'*') {
                let star = star + offset;
                if star > offset && bytes[star - 1] == b'/' {
                    nested += 1;
                    offset = star + 1;
                } else if bytes[star + 1] == b'/' {
                    if nested == 0 {
                        return star + 2;
                    }
                    nested -= 1;
                    offset = star + 2;
                } else {
                    offset = star + 1;
                }
            } else {
                // Unterminated
                return 0;
            }
        }
        // Unterminated
        return 0;
    }

    0
}

pub fn lex_string(text: &str) -> usize {
    if !text.starts_with('"') {
        return 0;
    }
    let end = text[1..].find('"').unwrap_or(0) + 2;
    assert!(!text[..end].contains("\\"), "escape code not yet supported");
    end
}

pub fn lex_number(text: &str) -> usize {
    let mut len = 0;
    let mut chars = text.chars();
    let mut had_period = false;
    while let Some(c) = chars.next() {
        if !c.is_ascii_digit() {
            if !had_period && c == '.' && len > 0 {
                had_period = true;
            } else {
                if len > 0 {
                    if c == '%' {
                        return len + 1;
                    }
                    if c.is_ascii_alphabetic() {
                        len += c.len_utf8();
                        // The unit
                        while let Some(c) = chars.next() {
                            if !c.is_ascii_alphabetic() {
                                return len;
                            }
                            len += c.len_utf8();
                        }
                    }
                }
                break;
            }
        }
        len += c.len_utf8();
    }
    len
}

pub fn lex_color(text: &str) -> usize {
    if !text.starts_with("#") {
        return 0;
    }
    let mut len = 1;
    let mut chars = text[1..].chars();
    while let Some(c) = chars.next() {
        if !c.is_ascii_alphanumeric() {
            break;
        }
        len += c.len_utf8();
    }
    len
}

pub fn lex_identifier(text: &str) -> usize {
    let mut len = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if !c.is_alphanumeric() && c != '_' && (c != '-' || len == 0) {
            break;
        }
        len += c.len_utf8();
    }
    len
}

pub fn lex(mut source: &str) -> Vec<crate::parser::Token> {
    let mut result = vec![];
    let mut offset = 0;
    while !source.is_empty() {
        if let Some((len, kind)) = crate::parser::lex_next_token(source) {
            result.push(crate::parser::Token {
                kind,
                text: source[..len].into(),
                offset,
                ..Default::default()
            });
            offset += len;
            source = &source[len..];
        } else {
            // FIXME: recover
            result.push(crate::parser::Token {
                kind: crate::parser::SyntaxKind::Error,
                text: source.into(),
                offset,
                ..Default::default()
            });
            //offset += source.len();
            break;
        }
    }
    result
}

#[test]
fn basic_lexer_test() {
    fn compare(source: &str, expected: &[(crate::parser::SyntaxKind, &str)]) {
        let actual = lex(source);
        let actual =
            actual.iter().map(|token| (token.kind, token.text.as_str())).collect::<Vec<_>>();
        assert_eq!(actual.as_slice(), expected);
    }

    compare(
        r#"45  /*hi/*_*/ho*/ "string""#,
        &[
            (crate::parser::SyntaxKind::NumberLiteral, "45"),
            (crate::parser::SyntaxKind::Whitespace, "  "),
            (crate::parser::SyntaxKind::Comment, "/*hi/*_*/ho*/"),
            (crate::parser::SyntaxKind::Whitespace, " "),
            (crate::parser::SyntaxKind::StringLiteral, r#""string""#),
        ],
    );

    compare(
        r#"12px+5.2+=0.7%"#,
        &[
            (crate::parser::SyntaxKind::NumberLiteral, "12px"),
            (crate::parser::SyntaxKind::Plus, "+"),
            (crate::parser::SyntaxKind::NumberLiteral, "5.2"),
            (crate::parser::SyntaxKind::PlusEqual, "+="),
            (crate::parser::SyntaxKind::NumberLiteral, "0.7%"),
        ],
    );
    compare(
        r#"aa_a.b1,c"#,
        &[
            (crate::parser::SyntaxKind::Identifier, "aa_a"),
            (crate::parser::SyntaxKind::Dot, "."),
            (crate::parser::SyntaxKind::Identifier, "b1"),
            (crate::parser::SyntaxKind::Comma, ","),
            (crate::parser::SyntaxKind::Identifier, "c"),
        ],
    );
    compare(
        r#"/*/**/*//**/*"#,
        &[
            (crate::parser::SyntaxKind::Comment, "/*/**/*/"),
            (crate::parser::SyntaxKind::Comment, "/**/"),
            (crate::parser::SyntaxKind::Star, "*"),
        ],
    );
}
