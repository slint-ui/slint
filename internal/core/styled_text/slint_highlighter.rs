// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A small `.slint` syntax highlighter for `TextInput`'s `highlight` callback.
//!
//! It's written like a highlighter in application code: it splits the text into tokens and
//! interpolates each one into a format string of `<font color>` tags, the equivalent of
//! `@markdown("\{a}<font color=\"#…\">\{b}</font>")` in Slint.
//! Interpolated text isn't parsed as markdown, so the result has the characters of the input.

use super::StyledText;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use i_slint_common::styled_text::MARKDOWN_INTERPOLATION_PLACEHOLDER;

const KEYWORD_COLOR: &str = "#9b4dca";
const TYPE_COLOR: &str = "#2a7fd4";
const STRING_COLOR: &str = "#3d9a3d";
const NUMBER_COLOR: &str = "#d4762a";
const COMMENT_COLOR: &str = "#808080";

const KEYWORDS: &[&str] = &[
    "animate",
    "callback",
    "changed",
    "component",
    "else",
    "enum",
    "export",
    "false",
    "for",
    "from",
    "function",
    "global",
    "if",
    "import",
    "in",
    "in-out",
    "inherits",
    "out",
    "parent",
    "private",
    "property",
    "public",
    "pure",
    "return",
    "root",
    "self",
    "states",
    "struct",
    "transitions",
    "true",
    "when",
];

/// Returns `text` with the comments, strings, numbers, keywords and element names colored.
pub fn slint_highlighter(text: &str) -> StyledText {
    let placeholder = MARKDOWN_INTERPOLATION_PLACEHOLDER;
    // Starting with a placeholder keeps the format from starting with an HTML tag.
    let mut format = String::from(placeholder);
    let mut args = vec![StyledText::default()];
    let mut plain = String::new();
    let mut push = |token: &str, color: Option<&str>| {
        let Some(color) = color else { return plain.push_str(token) };
        format.push(placeholder);
        args.push(super::string_to_styled_text(core::mem::take(&mut plain)));
        format.push_str(&format!("<font color=\"{color}\">{placeholder}</font>"));
        args.push(super::string_to_styled_text(token.into()));
    };

    let mut rest = text;
    while let Some(first) = rest.chars().next() {
        let (len, color) = if rest.starts_with("//") {
            (rest.find('\n').unwrap_or(rest.len()), Some(COMMENT_COLOR))
        } else if rest.starts_with("/*") {
            (rest[2..].find("*/").map_or(rest.len(), |end| end + 4), Some(COMMENT_COLOR))
        } else if first == '"' {
            (string_literal_len(rest), Some(STRING_COLOR))
        } else if first.is_ascii_digit() || first == '#' {
            let len = rest[1..]
                .find(|c: char| !(c.is_alphanumeric() || c == '.' || c == '%'))
                .map_or(rest.len(), |len| len + 1);
            (len, Some(NUMBER_COLOR))
        } else if first.is_alphabetic() || first == '_' || first == '@' {
            let len = rest[first.len_utf8()..]
                .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
                .map_or(rest.len(), |len| len + first.len_utf8());
            let word = &rest[..len];
            let color = if first == '@' || KEYWORDS.contains(&word) {
                Some(KEYWORD_COLOR)
            } else if first.is_uppercase() {
                Some(TYPE_COLOR)
            } else {
                None
            };
            (len, color)
        } else {
            (first.len_utf8(), None)
        };
        push(&rest[..len], color);
        rest = &rest[len..];
    }
    format.push(placeholder);
    args.push(super::string_to_styled_text(plain));

    super::parse_markdown(&format, &args)
}

/// The length of the string literal at the start of `text`, up to the closing quote or the end of
/// the line.
fn string_literal_len(text: &str) -> usize {
    let mut chars = text.char_indices().skip(1);
    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return index + 1,
            '\n' => return index,
            '\\' => {
                chars.next();
            }
            _ => {}
        }
    }
    text.len()
}

#[cfg(test)]
#[test]
fn keeps_the_characters() {
    for text in [
        "",
        "export component Foo inherits Window {\n    in property <int> x: 42px;\n}",
        "  - not a list\n# not a heading\n\n1. nor this\r\n\ttab *a* _b_ `c` <u>d</u> &amp; \\",
        "text: \"a \\\"quoted\\\" \\{x} string\"; // comment\n/* block\ncomment */ #ff0000",
        "\"unterminated\nnext line",
        "\n\nstarts with empty lines",
        "日本語 🎉 @image-url(\"x.png\")",
    ] {
        let styled = slint_highlighter(text);
        assert_eq!(super::get_raw_text(&styled), text);
    }
}

#[cfg(test)]
#[test]
fn colors_tokens() {
    use i_slint_common::styled_text::Style;
    let styled = slint_highlighter("in property <int> x: root.y + 4.2px; // note");
    let colors: alloc::vec::Vec<_> = styled.paragraphs[0]
        .formatting
        .iter()
        .map(|span| match span.style {
            Style::Color(color) => (
                &styled.paragraphs[0].text[span.range.clone()],
                crate::Color::from_argb_encoded(color),
            ),
            _ => panic!("unexpected style {:?}", span.style),
        })
        .collect();
    let rgb = |hex: u32| crate::Color::from_argb_encoded(0xff000000 | hex);
    assert_eq!(
        colors,
        [
            ("in", rgb(0x9b4dca)),
            ("property", rgb(0x9b4dca)),
            ("root", rgb(0x9b4dca)),
            ("4.2px", rgb(0xd4762a)),
            ("// note", rgb(0x808080)),
        ]
    );
}
