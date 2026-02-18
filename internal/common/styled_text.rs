// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[derive(Clone, Debug, PartialEq)]
/// Styles that can be applied to text spans
#[allow(missing_docs, dead_code)]
pub enum Style {
    Emphasis,
    Strong,
    Strikethrough,
    Code,
    Link,
    Underline,
    // ARGB encoded
    Color(u32),
}

#[derive(Clone, Debug, PartialEq)]
/// A style and a text span
pub struct FormattedSpan {
    /// Span of text to style
    pub range: core::ops::Range<usize>,
    /// The style to apply
    pub style: Style,
}

#[cfg(feature = "markdown")]
#[derive(Clone, Debug)]
enum ListItemType {
    Ordered(u64),
    Unordered,
}

/// A section of styled text, split up by a linebreak
#[derive(Clone, Debug, PartialEq)]
pub struct StyledTextParagraph {
    /// The raw paragraph text
    pub text: alloc::string::String,
    /// Formatting styles and spans
    pub formatting: alloc::vec::Vec<FormattedSpan>,
    /// Locations of clickable links within the paragraph
    pub links: alloc::vec::Vec<(core::ops::Range<usize>, alloc::string::String)>,
}

/// Error type returned by `StyledText::parse`
#[cfg(feature = "markdown")]
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
    #[error("Unimplemented tag: {:?}", .0.to_end())]
    UnimplementedTag(pulldown_cmark::Tag<'a>),
    /// Unimplemented markdown event
    #[error("Unimplemented event: {:?}", .0)]
    UnimplementedEvent(pulldown_cmark::Event<'a>),
    /// Unimplemented html event
    #[error("Unimplemented html: {}", .0)]
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
    /// Unexpected trailing '{' in format string
    #[error("Unexpected '{{' in format string. Escape '{{' with '{{{{'")]
    UnexpectedTrailingBrace,
    /// Unexpected '}' in format string
    #[error("Unexpected '}}' in format string. Escape '}}' with '}}}}'")]
    UnexpectedClosingBrace,
    /// Unterminated placeholder in format string
    #[error("Unterminated placeholder in format string. '{{' must be escaped with '{{{{'")]
    UnterminatedPlaceholder,
    /// Invalid placeholder in format string
    #[error(
        "Invalid '{{...}}' placeholder in format string. The placeholder must be a number, or braces must be escaped with '{{' and '}}'"
    )]
    InvalidPlaceholder,
    /// Argument index out of range
    #[error("Argument index {} out of range: {} arguments provided", .0, .1)]
    ArgumentOutOfRange(usize, usize),
    /// Format string placeholders count mismatch
    #[error("Format string contains {} placeholders, but {} arguments were provided", .0, .1)]
    PlaceholderCountMismatch(usize, usize),
    /// Mixed placeholder types
    #[error("Cannot mix positional and non-positional placeholder in format string")]
    MixedPlaceholders,
    #[error("Interpolating multiple styled text paragraphs is not currently implemented")]
    MultiParagraphInterpolation,
}

/// Styled text that has been parsed and seperated into paragraphs
#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Default)]
pub struct StyledText {
    /// Paragraphs of styled text
    pub paragraphs: alloc::vec::Vec<StyledTextParagraph>,
}

#[cfg(feature = "markdown")]
impl StyledText {
    pub fn from_plain_text(text: alloc::string::String) -> Self {
        Self {
            paragraphs: alloc::vec![StyledTextParagraph {
                text,
                formatting: Default::default(),
                links: Default::default()
            }],
        }
    }

    /// Parse a markdown string with interpolated arguments as styled text
    pub fn parse_interpolated<S: AsRef<[StyledTextParagraph]>>(
        format_string: &str,
        args: &[S],
    ) -> Result<Self, StyledTextError<'static>> {
        let parser = pulldown_cmark::Parser::new_ext(
            format_string,
            pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
        );

        let mut paragraphs = alloc::vec::Vec::new();
        let mut list_state_stack: alloc::vec::Vec<Option<u64>> = alloc::vec::Vec::new();
        let mut style_stack = alloc::vec::Vec::new();
        let mut current_url = None;
        let mut implicit_arg_index = 0;
        let mut positioned_arg_index_max = 0;

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

        let mut substitute = |paragraph: &mut StyledTextParagraph,
                              string: &str|
         -> Result<(), StyledTextError<'static>> {
            let mut pos = 0;
            let mut literal_start_pos = 0;
            while let Some(mut p) = string[pos..].find(['{', '}']) {
                if string.len() - pos < p + 1 {
                    return Err(StyledTextError::UnexpectedTrailingBrace);
                }
                p += pos;

                // Skip escaped }
                if string.get(p..=p) == Some("}") {
                    if string.get(p + 1..=p + 1) == Some("}") {
                        pos = p + 2;
                        continue;
                    } else {
                        return Err(StyledTextError::UnexpectedClosingBrace);
                    }
                }

                // Skip escaped {
                if string.get(p + 1..=p + 1) == Some("{") {
                    pos = p + 2;
                    continue;
                }

                // Find the argument
                let end = if let Some(end) = string[p..].find('}') {
                    end + p
                } else {
                    return Err(StyledTextError::UnterminatedPlaceholder);
                };

                let inner_arg_string = &string[p + 1..end];
                let arg_index = if inner_arg_string.is_empty() {
                    let arg_index = implicit_arg_index;
                    implicit_arg_index += 1;
                    arg_index
                } else if let Ok(n) = inner_arg_string.parse::<u16>() {
                    let positioned_arg_index = n as usize;
                    positioned_arg_index_max =
                        positioned_arg_index_max.max(positioned_arg_index + 1);
                    positioned_arg_index
                } else {
                    return Err(StyledTextError::InvalidPlaceholder);
                };

                paragraph.text.push_str(&string[literal_start_pos..p]);

                if let Some(arg) = args.get(arg_index) {
                    let arg_paragraphs = arg.as_ref();
                    if arg_paragraphs.len() != 1 {
                        return Err(StyledTextError::MultiParagraphInterpolation);
                    }
                    let arg_paragraph = &arg_paragraphs[0];

                    let offset = paragraph.text.len();
                    paragraph.text.push_str(&arg_paragraph.text);
                    paragraph.formatting.extend(arg_paragraph.formatting.iter().cloned().map(
                        |mut f| {
                            f.range.start += offset;
                            f.range.end += offset;
                            f
                        },
                    ));
                    paragraph.links.extend(arg_paragraph.links.iter().cloned().map(
                        |(mut range, link)| {
                            range.start += offset;
                            range.end += offset;
                            (range, link)
                        },
                    ));
                } else {
                    return Err(StyledTextError::ArgumentOutOfRange(arg_index, args.len()));
                }

                pos = end + 1;
                literal_start_pos = pos;
            }
            paragraph.text.push_str(&string[literal_start_pos..]);

            Ok(())
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
                            return Err(StyledTextError::UnimplementedTag(tag.into_static()));
                        }
                    };

                    style_stack.push((
                        style,
                        paragraphs.last().ok_or(StyledTextError::ParagraphNotStarted)?.text.len(),
                    ));
                }
                pulldown_cmark::Event::Text(text) => {
                    let mut paragraph =
                        paragraphs.last_mut().ok_or(StyledTextError::ParagraphNotStarted)?;
                    substitute(&mut paragraph, &text)?;
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
                    let mut paragraph =
                        paragraphs.last_mut().ok_or(StyledTextError::ParagraphNotStarted)?;
                    let start = paragraph.text.len();

                    substitute(&mut paragraph, &text)?;
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
                                            crate::color_parsing::parse_color_literal(&*value)
                                                .or_else(|| {
                                                    crate::color_parsing::named_colors()
                                                        .get(&*value)
                                                        .copied()
                                                })
                                                .expect("invalid color value");

                                        style_stack.push((
                                            Style::Color(value),
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
                    return Err(StyledTextError::UnimplementedEvent(event.into_static()));
                }
            }
        }

        if implicit_arg_index > 0 && positioned_arg_index_max > 0 {
            return Err(StyledTextError::MixedPlaceholders);
        }

        if (positioned_arg_index_max == 0 && implicit_arg_index != args.len())
            || positioned_arg_index_max > args.len()
        {
            return Err(StyledTextError::PlaceholderCountMismatch(
                implicit_arg_index.max(positioned_arg_index_max),
                args.len(),
            ));
        }

        if !style_stack.is_empty() {
            return Err(StyledTextError::NotEmpty);
        }

        Ok(StyledText { paragraphs: (&paragraphs[..]).into() })
    }
}

#[cfg(feature = "markdown")]
impl AsRef<[StyledTextParagraph]> for StyledText {
    fn as_ref(&self) -> &[StyledTextParagraph] {
        &self.paragraphs
    }
}

#[test]
fn markdown_parsing() {
    assert_eq!(
        StyledText::parse_interpolated::<StyledText>("hello *world*", &[]).unwrap().paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 6..11, style: Style::Emphasis }],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        StyledText::parse_interpolated::<StyledText>(
            "
- line 1
- line 2
            ",
            &[]
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
        StyledText::parse_interpolated::<StyledText>(
            "
1. a
2. b
4. c
        ",
            &[]
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
        StyledText::parse_interpolated::<StyledText>(
            "
Normal _italic_ **strong** ~~strikethrough~~ `code`
new *line*
",
            &[]
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
        StyledText::parse_interpolated::<StyledText>(
            "
- root
  - child
    - grandchild
      - great grandchild
",
            &[]
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
        StyledText::parse_interpolated::<StyledText>("hello [*world*](https://example.com)", &[])
            .unwrap()
            .paragraphs,
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
        StyledText::parse_interpolated::<StyledText>("<u>hello world</u>", &[]).unwrap().paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..11, style: Style::Underline },],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        StyledText::parse_interpolated::<StyledText>(
            r#"<font color="blue">hello world</font>"#,
            &[]
        )
        .unwrap()
        .paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan {
                range: 0..11,
                style: Style::Color(0xff_00_00_ff)
            },],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        StyledText::parse_interpolated::<StyledText>(
            r#"<u><font color="red">hello world</font></u>"#,
            &[]
        )
        .unwrap()
        .paragraphs,
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..11, style: Style::Color(0xff_ff_00_00) },
                FormattedSpan { range: 0..11, style: Style::Underline },
            ],
            links: alloc::vec::Vec::new()
        }]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_parsing_interpolated() {
    assert_eq!(
        StyledText::parse_interpolated(
            "Text: *{}*",
            &[StyledText::from_plain_text("italic".into())]
        )
        .unwrap()
        .paragraphs,
        [StyledTextParagraph {
            text: "Text: italic".into(),
            formatting: alloc::vec![FormattedSpan { range: 6..12, style: Style::Emphasis }],
            links: alloc::vec![]
        }]
    );
    assert_eq!(
        StyledText::parse_interpolated(
            "Escaped text: {}",
            &[StyledText::from_plain_text("*bold*".into())]
        )
        .unwrap()
        .paragraphs,
        [StyledTextParagraph {
            text: "Escaped text: *bold*".into(),
            formatting: alloc::vec![],
            links: alloc::vec![]
        }]
    );
    assert_eq!(
        StyledText::parse_interpolated(
            "Code block text: `{}`",
            &[StyledText::from_plain_text("*bold*".into())]
        )
        .unwrap()
        .paragraphs,
        [StyledTextParagraph {
            text: "Code block text: *bold*".into(),
            formatting: alloc::vec![FormattedSpan { range: 17..23, style: Style::Code }],
            links: alloc::vec![]
        }]
    );
    assert_eq!(
        StyledText::parse_interpolated(
            "**{}** {}",
            &[
                StyledText::from_plain_text("Hello".into()),
                StyledText::parse_interpolated::<StyledText>("*World*", &[]).unwrap()
            ]
        )
        .unwrap()
        .paragraphs,
        [StyledTextParagraph {
            text: "Hello World".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..5, style: Style::Strong },
                FormattedSpan { range: 6..11, style: Style::Emphasis }
            ],
            links: alloc::vec![]
        }]
    );
    assert_eq!(
        StyledText::parse_interpolated(
            "<u>{}</u>",
            &[StyledText::parse_interpolated::<StyledText>("*underline_and_italic*", &[]).unwrap()]
        )
        .unwrap()
        .paragraphs,
        [StyledTextParagraph {
            text: "underline_and_italic".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..20, style: Style::Emphasis },
                FormattedSpan { range: 0..20, style: Style::Underline },
            ],
            links: alloc::vec![]
        }]
    );
}
