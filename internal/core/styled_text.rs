// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[derive(Clone, Debug, PartialEq)]
/// Styles that can be applied to text spans
#[allow(missing_docs, dead_code)]
pub(crate) enum Style {
    Emphasis,
    Strong,
    Strikethrough,
    Code,
    Link,
    Underline,
    Color(crate::Color),
}

#[derive(Clone, Debug, PartialEq)]
/// A style and a text span
pub(crate) struct FormattedSpan {
    /// Span of text to style
    pub(crate) range: core::ops::Range<usize>,
    /// The style to apply
    pub(crate) style: Style,
}

#[cfg(feature = "std")]
#[derive(Clone, Debug)]
enum ListItemType {
    Ordered(u64),
    Unordered,
}

/// A section of styled text, split up by a linebreak
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StyledTextParagraph {
    /// The raw paragraph text
    pub(crate) text: alloc::string::String,
    /// Formatting styles and spans
    pub(crate) formatting: alloc::vec::Vec<FormattedSpan>,
    /// Locations of clickable links within the paragraph
    pub(crate) links: alloc::vec::Vec<(core::ops::Range<usize>, alloc::string::String)>,
}

/// Error type returned by `StyledText::parse`
#[cfg(feature = "std")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StyledTextError<'a> {
    /// Spans are unbalanced: stack already empty when popped
    #[error("Spans are unbalanced: stack already empty when popped")]
    Pop,
    /// Spans are unbalanced: stack contained items at end of function
    #[error("Spans are unbalanced: stack contained items at end of function")]
    NotEmpty,
    /// Paragraph not started
    #[error("Paragraph not started")]
    ParagraphNotStarted,
    /// Unimplemented markdown tag
    #[error("Unimplemented: {:?}", .0)]
    UnimplementedTag(pulldown_cmark::Tag<'a>),
    /// Unimplemented markdown event
    #[error("Unimplemented: {:?}", .0)]
    UnimplementedEvent(pulldown_cmark::Event<'a>),
    /// Unimplemented html event
    #[error("Unimplemented: {}", .0)]
    UnimplementedHtmlEvent(alloc::string::String),
    /// Unimplemented html tag
    #[error("Unimplemented html tag: {}", .0)]
    UnimplementedHtmlTag(alloc::string::String),
    /// Unimplemented html attribute
    #[error("Unexpected {} attribute in html {}", .0, .1)]
    UnexpectedAttribute(alloc::string::String, alloc::string::String),
    /// Missing color attribute in html
    #[error("Missing color attribute in html {}", .0)]
    MissingColor(alloc::string::String),
    /// Closing html tag doesn't match the opening tag
    #[error("Closing html tag doesn't match the opening tag. Expected {}, got {}", .0, .1)]
    ClosingTagMismatch(&'a str, alloc::string::String),
}

/// Styled text that has been parsed and seperated into paragraphs
#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Default)]
pub struct StyledText {
    /// Paragraphs of styled text
    pub(crate) paragraphs: crate::SharedVector<StyledTextParagraph>,
}

#[cfg(feature = "std")]
impl StyledText {
    /// Parse a markdown string as styled text
    pub fn parse(string: &str) -> Result<Self, StyledTextError<'_>> {
        let parser =
            pulldown_cmark::Parser::new_ext(string, pulldown_cmark::Options::ENABLE_STRIKETHROUGH);

        let mut paragraphs = alloc::vec::Vec::new();
        let mut list_state_stack: alloc::vec::Vec<Option<u64>> = alloc::vec::Vec::new();
        let mut style_stack = alloc::vec::Vec::new();
        let mut current_url = None;

        let begin_paragraph = |paragraphs: &mut alloc::vec::Vec<StyledTextParagraph>,
                               indentation: u32,
                               list_item_type: Option<ListItemType>| {
            let mut text = alloc::string::String::with_capacity(indentation as usize * 4);
            for _ in 0..indentation {
                text.push_str("    ");
            }
            match list_item_type {
                Some(ListItemType::Unordered) => {
                    if indentation % 3 == 0 {
                        text.push_str("• ")
                    } else if indentation % 3 == 1 {
                        text.push_str("◦ ")
                    } else {
                        text.push_str("▪ ")
                    }
                }
                Some(ListItemType::Ordered(num)) => text.push_str(&alloc::format!("{}. ", num)),
                None => {}
            };
            paragraphs.push(StyledTextParagraph {
                text,
                formatting: Default::default(),
                links: Default::default(),
            });
        };

        for event in parser {
            let indentation = list_state_stack.len().saturating_sub(1) as _;

            match event {
                pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                    begin_paragraph(&mut paragraphs, indentation, None);
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::List(_)) => {
                    if list_state_stack.pop().is_none() {
                        return Err(StyledTextError::Pop);
                    }
                }
                pulldown_cmark::Event::End(
                    pulldown_cmark::TagEnd::Paragraph | pulldown_cmark::TagEnd::Item,
                ) => {}
                pulldown_cmark::Event::Start(tag) => {
                    let style = match tag {
                        pulldown_cmark::Tag::Paragraph => {
                            begin_paragraph(&mut paragraphs, indentation, None);
                            continue;
                        }
                        pulldown_cmark::Tag::Item => {
                            begin_paragraph(
                                &mut paragraphs,
                                indentation,
                                Some(match list_state_stack.last().copied() {
                                    Some(Some(index)) => ListItemType::Ordered(index),
                                    _ => ListItemType::Unordered,
                                }),
                            );
                            if let Some(state) = list_state_stack.last_mut() {
                                *state = state.map(|state| state + 1);
                            }
                            continue;
                        }
                        pulldown_cmark::Tag::List(index) => {
                            list_state_stack.push(index);
                            continue;
                        }
                        pulldown_cmark::Tag::Strong => Style::Strong,
                        pulldown_cmark::Tag::Emphasis => Style::Emphasis,
                        pulldown_cmark::Tag::Strikethrough => Style::Strikethrough,
                        pulldown_cmark::Tag::Link { dest_url, .. } => {
                            current_url = Some(dest_url);
                            Style::Link
                        }

                        pulldown_cmark::Tag::Heading { .. }
                        | pulldown_cmark::Tag::Image { .. }
                        | pulldown_cmark::Tag::DefinitionList
                        | pulldown_cmark::Tag::DefinitionListTitle
                        | pulldown_cmark::Tag::DefinitionListDefinition
                        | pulldown_cmark::Tag::TableHead
                        | pulldown_cmark::Tag::TableRow
                        | pulldown_cmark::Tag::TableCell
                        | pulldown_cmark::Tag::HtmlBlock
                        | pulldown_cmark::Tag::Superscript
                        | pulldown_cmark::Tag::Subscript
                        | pulldown_cmark::Tag::Table(_)
                        | pulldown_cmark::Tag::MetadataBlock(_)
                        | pulldown_cmark::Tag::BlockQuote(_)
                        | pulldown_cmark::Tag::CodeBlock(_)
                        | pulldown_cmark::Tag::FootnoteDefinition(_) => {
                            return Err(StyledTextError::UnimplementedTag(tag));
                        }
                    };

                    style_stack.push((
                        style,
                        paragraphs.last().ok_or(StyledTextError::ParagraphNotStarted)?.text.len(),
                    ));
                }
                pulldown_cmark::Event::Text(text) => {
                    paragraphs
                        .last_mut()
                        .ok_or(StyledTextError::ParagraphNotStarted)?
                        .text
                        .push_str(&text);
                }
                pulldown_cmark::Event::End(_) => {
                    let (style, start) = if let Some(value) = style_stack.pop() {
                        value
                    } else {
                        return Err(StyledTextError::Pop);
                    };

                    let paragraph =
                        paragraphs.last_mut().ok_or(StyledTextError::ParagraphNotStarted)?;
                    let end = paragraph.text.len();

                    if let Some(url) = current_url.take() {
                        paragraph.links.push((start..end, url.into()));
                    }

                    paragraph.formatting.push(FormattedSpan { range: start..end, style });
                }
                pulldown_cmark::Event::Code(text) => {
                    let paragraph =
                        paragraphs.last_mut().ok_or(StyledTextError::ParagraphNotStarted)?;
                    let start = paragraph.text.len();
                    paragraph.text.push_str(&text);
                    paragraph.formatting.push(FormattedSpan {
                        range: start..paragraph.text.len(),
                        style: Style::Code,
                    });
                }
                pulldown_cmark::Event::InlineHtml(html) => {
                    if html.starts_with("</") {
                        let (style, start) = if let Some(value) = style_stack.pop() {
                            value
                        } else {
                            return Err(StyledTextError::Pop);
                        };

                        let expected_tag = match &style {
                            Style::Color(_) => "</font>",
                            Style::Underline => "</u>",
                            other => std::unreachable!(
                                "Got unexpected closing style {:?} with html {}. This error should have been caught earlier.",
                                other,
                                html
                            ),
                        };

                        if (&*html) != expected_tag {
                            return Err(StyledTextError::ClosingTagMismatch(
                                expected_tag,
                                (&*html).into(),
                            ));
                        }

                        let paragraph =
                            paragraphs.last_mut().ok_or(StyledTextError::ParagraphNotStarted)?;
                        let end = paragraph.text.len();
                        paragraph.formatting.push(FormattedSpan { range: start..end, style });
                    } else {
                        let mut expecting_color_attribute = false;

                        for token in htmlparser::Tokenizer::from(&*html) {
                            match token {
                                Ok(htmlparser::Token::ElementStart { local: tag_type, .. }) => {
                                    match &*tag_type {
                                        "u" => {
                                            style_stack.push((
                                                Style::Underline,
                                                paragraphs
                                                    .last()
                                                    .ok_or(StyledTextError::ParagraphNotStarted)?
                                                    .text
                                                    .len(),
                                            ));
                                        }
                                        "font" => {
                                            expecting_color_attribute = true;
                                        }
                                        _ => {
                                            return Err(StyledTextError::UnimplementedHtmlTag(
                                                (&*tag_type).into(),
                                            ));
                                        }
                                    }
                                }
                                Ok(htmlparser::Token::Attribute {
                                    local: key,
                                    value: Some(value),
                                    ..
                                }) => match &*key {
                                    "color" => {
                                        if !expecting_color_attribute {
                                            return Err(StyledTextError::UnexpectedAttribute(
                                                (&*key).into(),
                                                (&*html).into(),
                                            ));
                                        }
                                        expecting_color_attribute = false;

                                        let value =
                                            i_slint_common::color_parsing::parse_color_literal(
                                                &*value,
                                            )
                                            .or_else(|| {
                                                i_slint_common::color_parsing::named_colors()
                                                    .get(&*value)
                                                    .copied()
                                            })
                                            .expect("invalid color value");

                                        style_stack.push((
                                            Style::Color(crate::Color::from_argb_encoded(value)),
                                            paragraphs
                                                .last()
                                                .ok_or(StyledTextError::ParagraphNotStarted)?
                                                .text
                                                .len(),
                                        ));
                                    }
                                    _ => {
                                        return Err(StyledTextError::UnexpectedAttribute(
                                            (&*key).into(),
                                            (&*html).into(),
                                        ));
                                    }
                                },
                                Ok(htmlparser::Token::ElementEnd { .. }) => {}
                                _ => {
                                    return Err(StyledTextError::UnimplementedHtmlEvent(
                                        alloc::format!("{:?}", token),
                                    ));
                                }
                            }
                        }

                        if expecting_color_attribute {
                            return Err(StyledTextError::MissingColor((&*html).into()));
                        }
                    }
                }
                pulldown_cmark::Event::Rule
                | pulldown_cmark::Event::TaskListMarker(_)
                | pulldown_cmark::Event::FootnoteReference(_)
                | pulldown_cmark::Event::InlineMath(_)
                | pulldown_cmark::Event::DisplayMath(_)
                | pulldown_cmark::Event::Html(_) => {
                    return Err(StyledTextError::UnimplementedEvent(event));
                }
            }
        }

        if !style_stack.is_empty() {
            return Err(StyledTextError::NotEmpty);
        }

        Ok(StyledText { paragraphs: (&paragraphs[..]).into() })
    }
}

#[test]
fn markdown_parsing() {
    assert_eq!(
        StyledText::parse("hello *world*").unwrap().paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 6..11, style: Style::Emphasis }],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        StyledText::parse(
            "
- line 1
- line 2
            "
        )
        .unwrap()
        .paragraphs,
        [
            StyledTextParagraph {
                text: "• line 1".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "• line 2".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }
        ]
    );

    assert_eq!(
        StyledText::parse(
            "
1. a
2. b
4. c
        "
        )
        .unwrap()
        .paragraphs,
        [
            StyledTextParagraph {
                text: "1. a".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "2. b".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "3. c".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }
        ]
    );

    assert_eq!(
        StyledText::parse(
            "
Normal _italic_ **strong** ~~strikethrough~~ `code`
new *line*
"
        )
        .unwrap()
        .paragraphs,
        [
            StyledTextParagraph {
                text: "Normal italic strong strikethrough code".into(),
                formatting: alloc::vec![
                    FormattedSpan { range: 7..13, style: Style::Emphasis },
                    FormattedSpan { range: 14..20, style: Style::Strong },
                    FormattedSpan { range: 21..34, style: Style::Strikethrough },
                    FormattedSpan { range: 35..39, style: Style::Code }
                ],
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "new line".into(),
                formatting: alloc::vec![FormattedSpan { range: 4..8, style: Style::Emphasis },],
                links: alloc::vec::Vec::new()
            }
        ]
    );

    assert_eq!(
        StyledText::parse(
            "
- root
  - child
    - grandchild
      - great grandchild
"
        )
        .unwrap()
        .paragraphs,
        [
            StyledTextParagraph {
                text: "• root".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "    ◦ child".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "        ▪ grandchild".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "            • great grandchild".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
        ]
    );

    assert_eq!(
        StyledText::parse("hello [*world*](https://example.com)").unwrap().paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 6..11, style: Style::Emphasis },
                FormattedSpan { range: 6..11, style: Style::Link }
            ],
            links: alloc::vec![(6..11, "https://example.com".into())]
        }]
    );

    assert_eq!(
        StyledText::parse("<u>hello world</u>").unwrap().paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..11, style: Style::Underline },],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        StyledText::parse(r#"<font color="blue">hello world</font>"#).unwrap().paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan {
                range: 0..11,
                style: Style::Color(crate::Color::from_rgb_u8(0, 0, 255))
            },],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        StyledText::parse(r#"<u><font color="red">hello world</font></u>"#).unwrap().paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![
                FormattedSpan {
                    range: 0..11,
                    style: Style::Color(crate::Color::from_rgb_u8(255, 0, 0))
                },
                FormattedSpan { range: 0..11, style: Style::Underline },
            ],
            links: alloc::vec::Vec::new()
        }]
    );
}

pub fn get_raw_text(styled_text: &StyledText) -> alloc::borrow::Cow<'_, str> {
    match styled_text.paragraphs.as_slice() {
        [] => "".into(),
        [paragraph] => paragraph.text.as_str().into(),
        _ => {
            let mut result = alloc::string::String::new();
            for paragraph in styled_text.paragraphs.iter() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(paragraph.text.as_str());
            }
            result.into()
        }
    }
}

/// Bindings for cbindgen
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[unsafe(no_mangle)]
    /// Create a new default styled text
    pub unsafe extern "C" fn slint_styled_text_new(out: *mut StyledText) {
        unsafe {
            core::ptr::write(out, Default::default());
        }
    }

    #[unsafe(no_mangle)]
    /// Destroy the shared string
    pub unsafe extern "C" fn slint_styled_text_drop(text: *const StyledText) {
        unsafe {
            core::ptr::read(text);
        }
    }

    #[unsafe(no_mangle)]
    /// Returns true if \a a is equal to \a b; otherwise returns false.
    pub extern "C" fn slint_styled_text_eq(a: &StyledText, b: &StyledText) -> bool {
        a == b
    }

    #[unsafe(no_mangle)]
    /// Clone the styled text
    pub unsafe extern "C" fn slint_styled_text_clone(out: *mut StyledText, ss: &StyledText) {
        unsafe { core::ptr::write(out, ss.clone()) }
    }
}

pub fn escape_markdown(text: &str) -> alloc::string::String {
    let mut out = alloc::string::String::with_capacity(text.len());

    for c in text.chars() {
        match c {
            '*' => out.push_str("\\*"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '_' => out.push_str("\\_"),
            '#' => out.push_str("\\#"),
            '-' => out.push_str("\\-"),
            '`' => out.push_str("\\`"),
            '&' => out.push_str("\\&"),
            _ => out.push(c),
        }
    }

    out
}

pub fn parse_markdown(_text: &str) -> StyledText {
    #[cfg(feature = "std")]
    {
        StyledText::parse(_text).unwrap()
    }
    #[cfg(not(feature = "std"))]
    Default::default()
}
