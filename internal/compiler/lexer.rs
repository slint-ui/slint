// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the code for the lexer.
//!
//! It is kind of shared with parser.rs, which implements the lex_next_token based on the macro_rules
//! that declares token

use crate::parser::SyntaxKind;

#[derive(Default)]
pub struct LexState {
    /// The top of the stack is the level of embedded braces `{`.
    /// So we must still lex so many '}' before re-entering into a string mode and pop the stack.
    template_string_stack: Vec<u32>,
}

/// This trait is used by the `crate::parser::lex_next_token` function and is implemented
/// for rule passed to the macro which can be either a string literal, or a function
pub trait LexingRule {
    /// Return the size of the match for this rule, or 0 if there is no match
    fn lex(&self, text: &str, state: &mut LexState) -> usize;
}

impl LexingRule for &str {
    #[inline]
    fn lex(&self, text: &str, _: &mut LexState) -> usize {
        if text.starts_with(*self) {
            self.len()
        } else {
            0
        }
    }
}

impl<F: Fn(&str, &mut LexState) -> usize> LexingRule for F {
    #[inline]
    fn lex(&self, text: &str, state: &mut LexState) -> usize {
        (self)(text, state)
    }
}

pub fn lex_whitespace(text: &str, _: &mut LexState) -> usize {
    let mut len = 0;
    let chars = text.chars();
    for c in chars {
        if !c.is_whitespace() && !['\u{0002}', '\u{0003}'].contains(&c) {
            break;
        }
        len += c.len_utf8();
    }
    len
}

pub fn lex_comment(text: &str, _: &mut LexState) -> usize {
    // FIXME: could report proper error if not properly terminated
    if text.starts_with("//") {
        return text.find(&['\n', '\r'] as &[_]).unwrap_or(text.len());
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
                } else if star < bytes.len() - 1 && bytes[star + 1] == b'/' {
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

pub fn lex_string(text: &str, state: &mut LexState) -> usize {
    if let Some(brace_level) = state.template_string_stack.last_mut() {
        if text.starts_with('{') {
            *brace_level += 1;
            return 0;
        } else if text.starts_with('}') {
            if *brace_level > 0 {
                *brace_level -= 1;
                return 0;
            } else {
                state.template_string_stack.pop();
            }
        } else if !text.starts_with('"') {
            return 0;
        }
    } else if !text.starts_with('"') {
        return 0;
    }
    let text_len = text.as_bytes().len();
    let mut end = 1; // skip the '"'
    loop {
        let stop = match text[end..].find(&['"', '\\'][..]) {
            Some(stop) => end + stop,
            // FIXME: report an error for unterminated string
            None => return 0,
        };
        match text.as_bytes()[stop] {
            b'"' => {
                return stop + 1;
            }
            b'\\' => {
                if text_len <= stop + 1 {
                    // FIXME: report an error for unterminated string
                    return 0;
                }
                if text.as_bytes()[stop + 1] == b'{' {
                    state.template_string_stack.push(0);
                    return stop + 2;
                }
                end = stop + 1 + text[stop + 1..].chars().next().map_or(0, |c| c.len_utf8())
            }
            _ => unreachable!(),
        }
    }
}

pub fn lex_number(text: &str, _: &mut LexState) -> usize {
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
                        for c in chars {
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

pub fn lex_color(text: &str, _: &mut LexState) -> usize {
    if !text.starts_with('#') {
        return 0;
    }
    let mut len = 1;
    let chars = text[1..].chars();
    for c in chars {
        if !c.is_ascii_alphanumeric() {
            break;
        }
        len += c.len_utf8();
    }
    len
}

pub fn lex_identifier(text: &str, _: &mut LexState) -> usize {
    let mut len = 0;
    let chars = text.chars();
    for c in chars {
        if !c.is_alphanumeric() && c != '_' && (c != '-' || len == 0) {
            break;
        }
        len += c.len_utf8();
    }
    len
}

#[allow(clippy::needless_update)] // Token may have extra fields depending on selected features
pub fn lex(mut source: &str) -> Vec<crate::parser::Token> {
    let mut result = vec![];
    let mut offset = 0;
    let mut state = LexState::default();
    if source.starts_with("\u{FEFF}") {
        // Skip BOM
        result.push(crate::parser::Token {
            kind: SyntaxKind::Whitespace,
            text: source[..3].into(),
            offset: 0,
            ..Default::default()
        });
        source = &source[3..];
        offset += 3;
    }
    while !source.is_empty() {
        if let Some((len, kind)) = crate::parser::lex_next_token(source, &mut state) {
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
                kind: SyntaxKind::Error,
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
    fn compare(source: &str, expected: &[(SyntaxKind, &str)]) {
        let actual = lex(source);
        let actual =
            actual.iter().map(|token| (token.kind, token.text.as_str())).collect::<Vec<_>>();
        assert_eq!(actual.as_slice(), expected);
    }

    compare(
        r#"45  /*hi/*_*/ho*/ "string""#,
        &[
            (SyntaxKind::NumberLiteral, "45"),
            (SyntaxKind::Whitespace, "  "),
            (SyntaxKind::Comment, "/*hi/*_*/ho*/"),
            (SyntaxKind::Whitespace, " "),
            (SyntaxKind::StringLiteral, r#""string""#),
        ],
    );

    compare(
        r#"12px+5.2+=0.7%"#,
        &[
            (SyntaxKind::NumberLiteral, "12px"),
            (SyntaxKind::Plus, "+"),
            (SyntaxKind::NumberLiteral, "5.2"),
            (SyntaxKind::PlusEqual, "+="),
            (SyntaxKind::NumberLiteral, "0.7%"),
        ],
    );
    compare(
        r#"aa_a.b1,c"#,
        &[
            (SyntaxKind::Identifier, "aa_a"),
            (SyntaxKind::Dot, "."),
            (SyntaxKind::Identifier, "b1"),
            (SyntaxKind::Comma, ","),
            (SyntaxKind::Identifier, "c"),
        ],
    );
    compare(
        r#"/*/**/*//**/*"#,
        &[
            (SyntaxKind::Comment, "/*/**/*/"),
            (SyntaxKind::Comment, "/**/"),
            (SyntaxKind::Star, "*"),
        ],
    );
    compare(
        "a//x\nb//y\r\nc//z",
        &[
            (SyntaxKind::Identifier, "a"),
            (SyntaxKind::Comment, "//x"),
            (SyntaxKind::Whitespace, "\n"),
            (SyntaxKind::Identifier, "b"),
            (SyntaxKind::Comment, "//y"),
            (SyntaxKind::Whitespace, "\r\n"),
            (SyntaxKind::Identifier, "c"),
            (SyntaxKind::Comment, "//z"),
        ],
    );
    compare(r#""x""#, &[(SyntaxKind::StringLiteral, r#""x""#)]);
    compare(
        r#"a"\"\\"x"#,
        &[
            (SyntaxKind::Identifier, "a"),
            (SyntaxKind::StringLiteral, r#""\"\\""#),
            (SyntaxKind::Identifier, "x"),
        ],
    );
    compare(
        r#""a\{b{c}d"e\{f}g"h}i"j"#,
        &[
            (SyntaxKind::StringLiteral, r#""a\{"#),
            (SyntaxKind::Identifier, "b"),
            (SyntaxKind::LBrace, "{"),
            (SyntaxKind::Identifier, "c"),
            (SyntaxKind::RBrace, "}"),
            (SyntaxKind::Identifier, "d"),
            (SyntaxKind::StringLiteral, r#""e\{"#),
            (SyntaxKind::Identifier, "f"),
            (SyntaxKind::StringLiteral, r#"}g""#),
            (SyntaxKind::Identifier, "h"),
            (SyntaxKind::StringLiteral, r#"}i""#),
            (SyntaxKind::Identifier, "j"),
        ],
    );

    // Fuzzer tests:
    compare(r#"/**"#, &[(SyntaxKind::Div, "/"), (SyntaxKind::Star, "*"), (SyntaxKind::Star, "*")]);
    compare(r#""\"#, &[(SyntaxKind::Error, "\"\\")]);
    compare(r#""\ޱ"#, &[(SyntaxKind::Error, "\"\\ޱ")]);
}

/// Given the source of a rust file, find the occurrence of each `slint!(...)`macro.
/// Return an iterator with the range of the location of the macro in the original source
pub fn locate_slint_macro(rust_source: &str) -> impl Iterator<Item = core::ops::Range<usize>> + '_ {
    let mut begin = 0;
    std::iter::from_fn(move || {
        let (open, close) = loop {
            if let Some(m) = rust_source[begin..].find("slint") {
                // heuristics to find if we are not in a comment or a string literal. Not perfect, but should work in most cases
                if let Some(x) = rust_source[begin..(begin + m)].rfind(['\\', '\n', '/', '\"']) {
                    if rust_source.as_bytes()[begin + x] != b'\n' {
                        begin += m + 5;
                        begin += rust_source[begin..].find(['\n']).unwrap_or(0);
                        continue;
                    }
                }
                begin += m + 5;
                while rust_source[begin..].starts_with(' ') {
                    begin += 1;
                }
                if !rust_source[begin..].starts_with('!') {
                    continue;
                }
                begin += 1;
                while rust_source[begin..].starts_with(' ') {
                    begin += 1;
                }
                let Some(open) = rust_source.as_bytes().get(begin) else { continue };
                match open {
                    b'{' => break (SyntaxKind::LBrace, SyntaxKind::RBrace),
                    b'[' => break (SyntaxKind::LBracket, SyntaxKind::RBracket),
                    b'(' => break (SyntaxKind::LParent, SyntaxKind::RParent),
                    _ => continue,
                }
            } else {
                // No macro found, just return
                return None;
            }
        };

        begin += 1;

        // Now find the matching closing delimiter
        // Technically, we should be lexing rust, not slint
        let mut state = LexState::default();
        let start = begin;
        let mut end = begin;
        let mut level = 0;
        while !rust_source[end..].is_empty() {
            let len = match crate::parser::lex_next_token(&rust_source[end..], &mut state) {
                Some((len, x)) if x == open => {
                    level += 1;
                    len
                }
                Some((_, x)) if x == close && level == 0 => {
                    break;
                }
                Some((len, x)) if x == close => {
                    level -= 1;
                    len
                }
                Some((len, _)) => len,
                None => {
                    // Lex error
                    break;
                }
            };
            if len == 0 {
                break; // Shouldn't happen
            }
            end += len;
        }
        begin = end;
        Some(start..end)
    })
}

#[test]
fn test_locate_rust_macro() {
    #[track_caller]
    fn do_test(source: &str, captures: &[&str]) {
        let result = locate_slint_macro(source).map(|r| &source[r]).collect::<Vec<_>>();
        assert_eq!(&result, captures);
    }

    do_test("\nslint{!{}}", &[]);
    do_test(
        "//slint!(123)\nslint!(456)\nslint ![789]\n/*slint!{abc}*/\nslint! {def}",
        &["456", "789", "def"],
    );
    do_test("slint!(slint!(abc))slint!()", &["slint!(abc)", ""]);
}

/// Given a Rust source file contents, return a string containing the contents of the first `slint!` macro
///
/// All the other bytes which are not newlines are replaced by space. This allow offsets in the resulting
/// string to preserve line and column number.
///
/// The last byte before the Slint area will be \u{2} (ASCII Start-of-Text), the first byte after
/// the slint code will be \u{3} (ASCII End-of-Text), so that programs can find the area of slint code
/// within the program.
///
/// Note that the slint compiler considers Start-of-Text and End-of-Text as whitespace and will treat them
/// accordingly.
pub fn extract_rust_macro(rust_source: String) -> Option<String> {
    let core::ops::Range { start, end } = locate_slint_macro(&rust_source).next()?;
    let mut bytes = rust_source.into_bytes();
    for c in &mut bytes[..start] {
        if *c != b'\n' {
            *c = b' '
        }
    }

    if start > 0 {
        bytes[start - 1] = 2;
    }
    if end < bytes.len() {
        bytes[end] = 3;

        for c in &mut bytes[end + 1..] {
            if *c != b'\n' {
                *c = b' '
            }
        }
    }
    Some(String::from_utf8(bytes).expect("We just added spaces"))
}

#[test]
fn test_extract_rust_macro() {
    assert_eq!(extract_rust_macro("\nslint{!{}}".into()), None);
    assert_eq!(
        extract_rust_macro(
            "abc\n€\nslint !  {x \" \\\" }🦀\" { () {}\n {} }xx =}-  ;}\n xxx \n yyy {}\n".into(),
        ),
        Some(
            "   \n   \n         \u{2}x \" \\\" }🦀\" { () {}\n {} }xx =\u{3}     \n     \n       \n".into(),
        )
    );

    assert_eq!(
        extract_rust_macro("xx\nabcd::slint!{abc{}efg".into()),
        Some("  \n            \u{2}abc{}efg".into())
    );
    assert_eq!(
        extract_rust_macro("slint!\nnot.\nslint!{\nunterminated\nxxx".into()),
        Some("      \n    \n      \u{2}\nunterminated\nxxx".into())
    );
    assert_eq!(extract_rust_macro("foo\n/* slint! { hello }\n".into()), None);
    assert_eq!(extract_rust_macro("foo\n/* slint::slint! { hello }\n".into()), None);
    assert_eq!(
        extract_rust_macro("foo\n// slint! { hello }\nslint!{world}\na".into()),
        Some("   \n                   \n      \u{2}world\u{3}\n ".into())
    );
    assert_eq!(extract_rust_macro("foo\n\" slint! { hello }\"\n".into()), None);
    assert_eq!(
        extract_rust_macro(
            "abc\n€\nslint !  (x /* \\\" )🦀*/ { () {}\n {} }xx =)-  ;}\n xxx \n yyy {}\n".into(),
        ),
        Some(
            "   \n   \n         \u{2}x /* \\\" )🦀*/ { () {}\n {} }xx =\u{3}     \n     \n       \n".into(),
        )
    );
    assert_eq!(
        extract_rust_macro("abc slint![x slint!() [{[]}] s] abc".into()),
        Some("          \u{0002}x slint!() [{[]}] s\u{0003}    ".into()),
    );
}
