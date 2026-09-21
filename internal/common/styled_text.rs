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

/// Error returned by markdown styled text parsing
#[cfg(feature = "markdown")]
#[derive(Debug, derive_more::Error, derive_more::Display)]
#[display("{kind}")]
pub struct StyledTextParseError {
    kind: StyledTextParseErrorKind,
    /// Byte range in the format string where the error occurred
    range: Option<core::ops::Range<usize>>,
}

#[cfg(feature = "markdown")]
impl StyledTextParseError {
    /// Byte range in the markdown format string where the error occurred
    pub fn range(&self) -> Option<core::ops::Range<usize>> {
        self.range.clone()
    }

    fn new(kind: StyledTextParseErrorKind, range: core::ops::Range<usize>) -> Self {
        Self { kind, range: Some(range) }
    }

    fn without_range(kind: StyledTextParseErrorKind) -> Self {
        Self { kind, range: None }
    }
}

#[cfg(feature = "markdown")]
impl PartialEq for StyledTextParseError {
    /// Compares only the error kind, ignoring the byte range.
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

#[cfg(feature = "markdown")]
#[derive(Debug, derive_more::Display, PartialEq)]
enum StyledTextParseErrorKind {
    /// Spans are unbalanced: stack already empty when popped
    #[display("Spans are unbalanced: stack already empty when popped")]
    Pop,
    /// Unterminated tag
    #[display("Unterminated tag")]
    UnterminatedTag,
    /// Paragraph not started
    #[display("Paragraph not started")]
    ParagraphNotStarted,
    /// Unsupported markdown syntax
    #[display("Markdown {_0} are not supported")]
    UnsupportedMarkdown(alloc::string::String),
    /// Unsupported html tag
    #[display("HTML tag <{_0}> is not supported")]
    UnsupportedHtmlTag(alloc::string::String),
    /// Unimplemented html attribute
    #[display("Unexpected {_0} attribute in html {_1}")]
    UnexpectedAttribute(alloc::string::String, alloc::string::String),
    /// Missing color attribute in html
    #[display("Missing color attribute in html {_0}")]
    MissingColor(alloc::string::String),
    /// Closing html tag doesn't match the opening tag
    #[display("Closing html tag doesn't match the opening tag. Expected {_0}, got {_1}")]
    ClosingTagMismatch(alloc::string::String, alloc::string::String),
    /// Argument index out of range
    #[display("Argument index {_0} out of range: {_1} arguments provided")]
    ArgumentOutOfRange(usize, usize),
    /// Format string placeholders count mismatch
    #[display("Format string contains {_0} placeholders, but {_1} arguments were provided")]
    PlaceholderCountMismatch(usize, usize),
    #[display("Interpolating multiple styled text paragraphs is not currently implemented")]
    MultiParagraphInterpolation,
    /// HTML closing tag overlaps with markdown formatting
    #[display("HTML tag {_0} overlaps with markdown formatting")]
    InterleavedStyles(alloc::string::String),
    /// Invalid color value
    #[display("Invalid color value '{_0}'")]
    InvalidColor(alloc::string::String),
}

#[cfg(feature = "markdown")]
/// If this is an `InvalidColor` error, return the color value string.
pub fn invalid_color_value(error: &StyledTextParseError) -> Option<&str> {
    match &error.kind {
        StyledTextParseErrorKind::InvalidColor(v) => Some(v),
        _ => None,
    }
}

#[cfg(feature = "markdown")]
pub fn paragraph_from_plain_text(text: alloc::string::String) -> StyledTextParagraph {
    StyledTextParagraph { text, formatting: Default::default(), links: Default::default() }
}

#[cfg(feature = "markdown")]
/// This is the character for private use that is used to make interpolation possible in markdown.
pub const MARKDOWN_INTERPOLATION_PLACEHOLDER: char = '\u{e541}';

#[cfg(feature = "markdown")]
fn begin_paragraph(indentation: u32, list_item_type: Option<ListItemType>) -> StyledTextParagraph {
    let mut text = alloc::string::String::with_capacity(indentation as usize * 4);
    for _ in 0..indentation {
        text.push_str("    ");
    }
    match list_item_type {
        Some(ListItemType::Unordered) => {
            let remainder = indentation % 3;
            if remainder == 0 {
                text.push_str("• ")
            } else if remainder == 1 {
                text.push_str("◦ ")
            } else {
                text.push_str("▪ ")
            }
        }
        Some(ListItemType::Ordered(num)) => text.push_str(&alloc::format!("{}. ", num)),
        None => {}
    };
    StyledTextParagraph { text, formatting: Default::default(), links: Default::default() }
}

#[cfg(feature = "markdown")]
fn append_paragraph(target: &mut StyledTextParagraph, source: &StyledTextParagraph) {
    let offset = target.text.len();
    target.text.push_str(&source.text);
    target.formatting.extend(source.formatting.iter().cloned().map(|mut f| {
        f.range.start += offset;
        f.range.end += offset;
        f
    }));
    target.links.extend(source.links.iter().cloned().map(|(mut range, link)| {
        range.start += offset;
        range.end += offset;
        (range, link)
    }));
}

#[cfg(feature = "markdown")]
fn substitute<S: AsRef<[StyledTextParagraph]>>(
    paragraph: &mut StyledTextParagraph,
    string: &str,
    args: &[S],
    arg_index: &mut usize,
    errors: &mut alloc::vec::Vec<StyledTextParseError>,
    event_range: &core::ops::Range<usize>,
) {
    use StyledTextParseErrorKind as E;
    let mut pos = 0;
    while let Some(mut p) = string[pos..].find(MARKDOWN_INTERPOLATION_PLACEHOLDER) {
        p += pos;
        paragraph.text.push_str(&string[pos..p]);

        if let Some(arg) = args.get(*arg_index) {
            match arg.as_ref() {
                [source] => append_paragraph(paragraph, source),
                [] => {}
                [first, ..] => {
                    errors.push(StyledTextParseError::new(
                        E::MultiParagraphInterpolation,
                        event_range.clone(),
                    ));
                    append_paragraph(paragraph, first);
                }
            }
        } else {
            errors.push(StyledTextParseError::new(
                E::ArgumentOutOfRange(*arg_index, args.len()),
                event_range.clone(),
            ));
        }

        *arg_index += 1;

        p += MARKDOWN_INTERPOLATION_PLACEHOLDER.len_utf8();
        pos = p;
    }
    paragraph.text.push_str(&string[pos..]);
}

#[cfg(feature = "markdown")]
fn substitute_in_string<S: AsRef<[StyledTextParagraph]>>(
    string: &str,
    args: &[S],
    arg_index: &mut usize,
    errors: &mut alloc::vec::Vec<StyledTextParseError>,
    event_range: &core::ops::Range<usize>,
) -> alloc::string::String {
    use StyledTextParseErrorKind as E;
    let mut result = alloc::string::String::with_capacity(string.len());
    let mut pos = 0;
    while let Some(mut p) = string[pos..].find(MARKDOWN_INTERPOLATION_PLACEHOLDER) {
        p += pos;
        result.push_str(&string[pos..p]);
        if let Some(arg) = args.get(*arg_index) {
            match arg.as_ref() {
                [arg_paragraph] => result.push_str(&arg_paragraph.text),
                [] => {}
                [first, ..] => {
                    errors.push(StyledTextParseError::new(
                        E::MultiParagraphInterpolation,
                        event_range.clone(),
                    ));
                    result.push_str(&first.text);
                }
            }
        } else {
            errors.push(StyledTextParseError::new(
                E::ArgumentOutOfRange(*arg_index, args.len()),
                event_range.clone(),
            ));
        }
        *arg_index += 1;
        p += MARKDOWN_INTERPOLATION_PLACEHOLDER.len_utf8();
        pos = p;
    }
    result.push_str(&string[pos..]);
    result
}

#[cfg(feature = "markdown")]
/// An open style: what it is, where it starts in the current paragraph, and the pieces of it
/// that earlier paragraphs already hold.
type OpenSpan = (Style, usize, alloc::vec::Vec<(usize, core::ops::Range<usize>)>);

#[cfg(feature = "markdown")]
fn push_span(
    paragraph: &mut StyledTextParagraph,
    range: core::ops::Range<usize>,
    style: Style,
    url: Option<alloc::string::String>,
) {
    if let Some(url) = url {
        paragraph.links.push((range.clone(), url));
    }
    paragraph.formatting.push(FormattedSpan { range, style });
}

#[cfg(feature = "markdown")]
/// Ends the current paragraph and starts `new_paragraph`.
///
/// A style the paragraphs are inside of covers text in both of them, and a range only addresses
/// one paragraph's text, so every open style is cut in two here (#13548). The piece that starts
/// in `new_paragraph` starts after whatever indentation or bullet it already holds.
fn start_paragraph(
    paragraphs: &mut alloc::vec::Vec<StyledTextParagraph>,
    current_paragraph: &mut Option<StyledTextParagraph>,
    style_stack: &mut [OpenSpan],
    new_paragraph: StyledTextParagraph,
) {
    let offset = new_paragraph.text.len();
    let Some(paragraph) = current_paragraph.replace(new_paragraph) else { return };
    let index = paragraphs.len();
    let end = paragraph.text.len();
    paragraphs.push(paragraph);
    for (_, start, pieces) in style_stack.iter_mut() {
        pieces.push((index, *start..end));
        *start = offset;
    }
}

#[cfg(feature = "markdown")]
/// Records the pieces an open style left in earlier paragraphs, and its last one in `paragraph`.
fn close_span(
    paragraphs: &mut [StyledTextParagraph],
    paragraph: &mut StyledTextParagraph,
    pieces: alloc::vec::Vec<(usize, core::ops::Range<usize>)>,
    last: core::ops::Range<usize>,
    style: Style,
    url: Option<alloc::string::String>,
) {
    // Each paragraph owns the text of its links, so every piece but the last needs a copy.
    for (index, range) in pieces {
        push_span(&mut paragraphs[index], range, style.clone(), url.clone());
    }
    push_span(paragraph, last, style, url);
}

#[cfg(feature = "markdown")]
fn get_or_create_paragraph<'a>(
    current_paragraph: &'a mut Option<StyledTextParagraph>,
    errors: &mut alloc::vec::Vec<StyledTextParseError>,
    event_range: &core::ops::Range<usize>,
) -> &'a mut StyledTextParagraph {
    use StyledTextParseErrorKind as E;
    if current_paragraph.is_none() {
        errors.push(StyledTextParseError::new(E::ParagraphNotStarted, event_range.clone()));
        *current_paragraph = Some(StyledTextParagraph {
            text: Default::default(),
            formatting: Default::default(),
            links: Default::default(),
        });
    }
    current_paragraph.as_mut().unwrap()
}

#[cfg(feature = "markdown")]
fn unsupported_tag_name(tag: &pulldown_cmark::Tag<'_>) -> alloc::string::String {
    use pulldown_cmark::Tag::*;
    match tag {
        Heading { .. } => "headings",
        Image { .. } => "images",
        BlockQuote(_) => "block quotes",
        CodeBlock(_) => "code blocks",
        Table(_) => "tables",
        HtmlBlock => "HTML blocks",
        FootnoteDefinition(_) => "footnotes",
        DefinitionList | DefinitionListTitle | DefinitionListDefinition => "definition lists",
        TableHead | TableRow | TableCell => "tables",
        MetadataBlock(_) => "metadata blocks",
        Superscript => "superscript",
        Subscript => "subscript",
        other => return alloc::format!("{:?}", other.to_end()),
    }
    .into()
}

#[cfg(feature = "markdown")]
fn unsupported_event_name(event: &pulldown_cmark::Event<'_>) -> alloc::string::String {
    use pulldown_cmark::Event::*;
    match event {
        Rule => "horizontal rules".into(),
        TaskListMarker(_) => "task lists".into(),
        FootnoteReference(_) => "footnote references".into(),
        InlineMath(_) | DisplayMath(_) => "math".into(),
        Html(text) => alloc::format!("HTML blocks ({})", text.trim()),
        _ => alloc::format!("{event:?}"),
    }
}

#[cfg(feature = "markdown")]
pub fn parse_interpolated<S: AsRef<[StyledTextParagraph]>>(
    format_string: &str,
    args: &[S],
) -> (alloc::vec::Vec<StyledTextParagraph>, alloc::vec::Vec<StyledTextParseError>) {
    use StyledTextParseErrorKind as E;

    let parser = pulldown_cmark::Parser::new_ext(
        format_string,
        pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
    );

    let mut list_state_stack: alloc::vec::Vec<Option<u64>> = alloc::vec::Vec::new();
    let mut style_stack: alloc::vec::Vec<OpenSpan> = alloc::vec::Vec::new();
    let mut current_url = None;
    let mut arg_index = 0;
    let mut paragraphs = alloc::vec::Vec::new();
    let mut errors = alloc::vec::Vec::new();
    // Tracks skipped Start tags whose End events haven't been seen yet.
    // When an End event fails to pop the style stack and this is > 0,
    // we silently consume it instead of reporting a cascading Pop error.
    let mut skip_end_count: usize = 0;
    let mut interleaved_count: usize = 0;

    let mut current_paragraph: Option<StyledTextParagraph> = None;

    for (event, event_range) in parser.into_offset_iter() {
        let indentation = list_state_stack.len().saturating_sub(1) as _;

        match event {
            pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                start_paragraph(
                    &mut paragraphs,
                    &mut current_paragraph,
                    &mut style_stack,
                    begin_paragraph(indentation, None),
                );
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::List(_)) => {
                if list_state_stack.pop().is_none() {
                    errors.push(StyledTextParseError::new(E::Pop, event_range.clone()));
                }
            }
            pulldown_cmark::Event::End(
                pulldown_cmark::TagEnd::Paragraph | pulldown_cmark::TagEnd::Item,
            ) => {}
            pulldown_cmark::Event::Start(tag) => {
                let style = match tag {
                    pulldown_cmark::Tag::Paragraph => {
                        start_paragraph(
                            &mut paragraphs,
                            &mut current_paragraph,
                            &mut style_stack,
                            begin_paragraph(indentation, None),
                        );
                        continue;
                    }
                    pulldown_cmark::Tag::Item => {
                        let new_paragraph = begin_paragraph(
                            indentation,
                            Some(match list_state_stack.last().copied() {
                                Some(Some(index)) => ListItemType::Ordered(index),
                                _ => ListItemType::Unordered,
                            }),
                        );
                        if let Some(state) = list_state_stack.last_mut() {
                            *state = state.map(|state| state + 1);
                        }
                        start_paragraph(
                            &mut paragraphs,
                            &mut current_paragraph,
                            &mut style_stack,
                            new_paragraph,
                        );
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

                    pulldown_cmark::Tag::BlockQuote(_) => {
                        let mut r = event_range.clone();
                        if let Some(pos) = format_string[r.clone()].find('>') {
                            r.start += pos;
                        }
                        errors.push(StyledTextParseError::new(
                            E::UnsupportedMarkdown(unsupported_tag_name(&tag)),
                            r,
                        ));
                        skip_end_count += 1;
                        continue;
                    }
                    pulldown_cmark::Tag::HtmlBlock => {
                        // Don't report an error here; the accompanying Html event
                        // provides a more descriptive message with the actual content
                        skip_end_count += 1;
                        continue;
                    }

                    ref unsupported => {
                        errors.push(StyledTextParseError::new(
                            E::UnsupportedMarkdown(unsupported_tag_name(unsupported)),
                            event_range.clone(),
                        ));
                        skip_end_count += 1;
                        continue;
                    }
                };

                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);

                style_stack.push((style, paragraph.text.len(), Default::default()));
            }
            pulldown_cmark::Event::Text(text) => {
                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);

                substitute(paragraph, &text, args, &mut arg_index, &mut errors, &event_range);
            }
            pulldown_cmark::Event::End(_) => {
                let (style, start, pieces) = if let Some(value) = style_stack.pop() {
                    value
                } else if skip_end_count > 0 {
                    skip_end_count -= 1;
                    continue;
                } else {
                    errors.push(StyledTextParseError::new(E::Pop, event_range.clone()));
                    continue;
                };

                let url = current_url.take().map(|url| {
                    if url.contains(MARKDOWN_INTERPOLATION_PLACEHOLDER) {
                        substitute_in_string(&url, args, &mut arg_index, &mut errors, &event_range)
                    } else {
                        url.into()
                    }
                });

                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                let end = paragraph.text.len();

                close_span(&mut paragraphs, paragraph, pieces, start..end, style, url);
            }
            pulldown_cmark::Event::Code(text) => {
                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                let start = paragraph.text.len();

                substitute(paragraph, &text, args, &mut arg_index, &mut errors, &event_range);
                push_span(paragraph, start..paragraph.text.len(), Style::Code, None);
            }
            pulldown_cmark::Event::InlineHtml(html) => {
                if html.starts_with("</") {
                    let (style, start, pieces) = if let Some(value) = style_stack.pop() {
                        value
                    } else if skip_end_count > 0 {
                        skip_end_count -= 1;
                        continue;
                    } else {
                        errors.push(StyledTextParseError::new(E::Pop, event_range.clone()));
                        continue;
                    };

                    let expected_tag = match &style {
                        Style::Color(_) => "</font>",
                        Style::Underline => "</u>",
                        _ => {
                            // The top of the stack is a markdown style, not
                            // the expected HTML style. Push it back and report
                            // an error instead of consuming it (issue #11563).
                            style_stack.push((style, start, pieces));
                            interleaved_count += 1;
                            errors.push(StyledTextParseError::new(
                                E::InterleavedStyles((&*html).into()),
                                event_range.clone(),
                            ));
                            continue;
                        }
                    };

                    if (&*html) != expected_tag {
                        errors.push(StyledTextParseError::new(
                            E::ClosingTagMismatch(expected_tag.into(), (&*html).into()),
                            event_range.clone(),
                        ));
                        // Still apply the style as best-effort
                    }

                    let paragraph =
                        get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);

                    let end = paragraph.text.len();
                    close_span(&mut paragraphs, paragraph, pieces, start..end, style, None);
                } else {
                    let mut expecting_color_attribute = false;
                    let mut push_skip = false;

                    // htmlparser offsets are relative to `html`; add event_range.start
                    // to get absolute format-string offsets
                    let base = event_range.start;

                    let errors_before = errors.len();

                    for token in htmlparser::Tokenizer::from(&*html) {
                        match token {
                            Ok(htmlparser::Token::ElementStart {
                                local: tag_type, span, ..
                            }) => match &*tag_type {
                                "u" => {
                                    let paragraph = get_or_create_paragraph(
                                        &mut current_paragraph,
                                        &mut errors,
                                        &event_range,
                                    );
                                    style_stack.push((
                                        Style::Underline,
                                        paragraph.text.len(),
                                        Default::default(),
                                    ));
                                }
                                "font" => {
                                    expecting_color_attribute = true;
                                }
                                _ => {
                                    let r = base + span.start()..base + span.end();
                                    errors.push(StyledTextParseError::new(
                                        E::UnsupportedHtmlTag((&*tag_type).into()),
                                        r,
                                    ));
                                    push_skip = true;
                                }
                            },
                            Ok(htmlparser::Token::Attribute {
                                local: key,
                                value: Some(value),
                                span,
                                ..
                            }) => match &*key {
                                "color" => {
                                    if !expecting_color_attribute {
                                        let r = base + span.start()..base + span.end();
                                        errors.push(StyledTextParseError::new(
                                            E::UnexpectedAttribute((&*key).into(), (&*html).into()),
                                            r,
                                        ));
                                        continue;
                                    }
                                    expecting_color_attribute = false;

                                    let color_str =
                                        if value.contains(MARKDOWN_INTERPOLATION_PLACEHOLDER) {
                                            Some(substitute_in_string(
                                                &value,
                                                args,
                                                &mut arg_index,
                                                &mut errors,
                                                &event_range,
                                            ))
                                        } else {
                                            None
                                        };
                                    let color_str = color_str.as_deref().unwrap_or(&*value);

                                    let color_value =
                                        crate::color_parsing::parse_color_literal(color_str)
                                            .or_else(|| {
                                                crate::color_parsing::named_colors()
                                                    .get(color_str)
                                                    .copied()
                                            });

                                    let value = color_value.unwrap_or_else(|| {
                                        let r = base + span.start()..base + span.end();
                                        errors.push(StyledTextParseError::new(
                                            E::InvalidColor(color_str.into()),
                                            r,
                                        ));
                                        // A dummy style so the closing </font> tag can pop
                                        // it without error
                                        0
                                    });
                                    let paragraph = get_or_create_paragraph(
                                        &mut current_paragraph,
                                        &mut errors,
                                        &event_range,
                                    );
                                    style_stack.push((
                                        Style::Color(value),
                                        paragraph.text.len(),
                                        Default::default(),
                                    ));
                                }
                                _ => {
                                    let r = base + span.start()..base + span.end();
                                    errors.push(StyledTextParseError::new(
                                        E::UnexpectedAttribute((&*key).into(), (&*html).into()),
                                        r,
                                    ));
                                }
                            },
                            Ok(htmlparser::Token::ElementEnd { .. }) => {}
                            _ => {
                                errors.push(StyledTextParseError::new(
                                    E::UnsupportedMarkdown(alloc::format!("{:?}", token)),
                                    event_range.clone(),
                                ));
                            }
                        }
                    }

                    if expecting_color_attribute {
                        // Only report MissingColor when no other errors were
                        // reported for this HTML fragment (avoids cascading diagnostics)
                        if errors.len() == errors_before {
                            errors.push(StyledTextParseError::new(
                                E::MissingColor((&*html).into()),
                                event_range.clone(),
                            ));
                        }
                        push_skip = true;
                    }

                    if push_skip {
                        skip_end_count += 1;
                    }
                }
            }
            pulldown_cmark::Event::Rule
            | pulldown_cmark::Event::TaskListMarker(_)
            | pulldown_cmark::Event::FootnoteReference(_)
            | pulldown_cmark::Event::InlineMath(_)
            | pulldown_cmark::Event::DisplayMath(_)
            | pulldown_cmark::Event::Html(_) => {
                errors.push(StyledTextParseError::new(
                    E::UnsupportedMarkdown(unsupported_event_name(&event)),
                    event_range,
                ));
            }
        }
    }

    if arg_index != args.len() {
        errors.push(StyledTextParseError::without_range(E::PlaceholderCountMismatch(
            arg_index,
            args.len(),
        )));
    }

    if style_stack.len() > interleaved_count {
        errors.push(StyledTextParseError::without_range(E::UnterminatedTag));
    }

    if let Some(paragraph) = current_paragraph.take() {
        paragraphs.push(paragraph);
    }

    (paragraphs, errors)
}

#[cfg(all(feature = "markdown", test))]
fn assert_no_errors(
    result: (alloc::vec::Vec<StyledTextParagraph>, alloc::vec::Vec<StyledTextParseError>),
) -> alloc::vec::Vec<StyledTextParagraph> {
    let (paragraphs, errors) = result;
    assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    paragraphs
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_style_across_line_break() {
    // A style that spans a line break becomes one span per paragraph. Before that, the offset
    // recorded in the paragraph before the break was applied to the one after it, which split a
    // multi-byte character and panicked the shaper (#13548).
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("Это **очень\nважно** для нас", &[])),
        [
            StyledTextParagraph {
                text: "Это очень".into(),
                formatting: alloc::vec![FormattedSpan { range: 7..17, style: Style::Strong }],
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "важно для нас".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..10, style: Style::Strong }],
                links: alloc::vec::Vec::new()
            }
        ]
    );

    // A hard break, and a span covering three paragraphs.
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("**a\\\nb\\\nc**", &[])),
        [
            StyledTextParagraph {
                text: "a".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..1, style: Style::Strong }],
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "b".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..1, style: Style::Strong }],
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "c".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..1, style: Style::Strong }],
                links: alloc::vec::Vec::new()
            }
        ]
    );

    // The indentation and the bullet a list item starts with are not part of the span.
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("- a\n  - **x\n    y**", &[])),
        [
            StyledTextParagraph {
                text: "• a".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "    ◦ x".into(),
                formatting: alloc::vec![FormattedSpan { range: 8..9, style: Style::Strong }],
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "    y".into(),
                formatting: alloc::vec![FormattedSpan { range: 4..5, style: Style::Strong }],
                links: alloc::vec::Vec::new()
            }
        ]
    );

    // An HTML style closes the same way.
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("<u>a\nb</u>", &[])),
        [
            StyledTextParagraph {
                text: "a".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..1, style: Style::Underline }],
                links: alloc::vec::Vec::new()
            },
            StyledTextParagraph {
                text: "b".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..1, style: Style::Underline }],
                links: alloc::vec::Vec::new()
            }
        ]
    );

    // A link keeps its destination on every paragraph it covers.
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("[a\nb](http://x)", &[])),
        [
            StyledTextParagraph {
                text: "a".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..1, style: Style::Link }],
                links: alloc::vec![(0..1, "http://x".into())]
            },
            StyledTextParagraph {
                text: "b".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..1, style: Style::Link }],
                links: alloc::vec![(0..1, "http://x".into())]
            }
        ]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_parsing() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("hello *world*", &[])),
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 6..11, style: Style::Emphasis }],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>(
            "
- line 1
- line 2
            ",
            &[]
        )),
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
        assert_no_errors(parse_interpolated::<&[_]>(
            "
1. a
2. b
4. c
        ",
            &[]
        )),
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
        assert_no_errors(parse_interpolated::<&[_]>(
            "
Normal _italic_ **strong** ~~strikethrough~~ `code`
new *line*
",
            &[]
        )),
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
        assert_no_errors(parse_interpolated::<&[_]>(
            "
- root
  - child
    - grandchild
      - great grandchild
",
            &[]
        )),
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
        assert_no_errors(parse_interpolated::<&[_]>("hello [*world*](https://example.com)", &[])),
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
        assert_no_errors(parse_interpolated::<&[_]>("<u>hello world</u>", &[])),
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..11, style: Style::Underline },],
            links: alloc::vec::Vec::new()
        }]
    );

    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>(
            r#"<font color="blue">hello world</font>"#,
            &[]
        )),
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
        assert_no_errors(parse_interpolated::<&[_]>(
            r#"<u><font color="red">hello world</font></u>"#,
            &[]
        )),
        [StyledTextParagraph {
            text: "hello world".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..11, style: Style::Color(0xff_ff_00_00) },
                FormattedSpan { range: 0..11, style: Style::Underline },
            ],
            links: alloc::vec::Vec::new()
        }]
    );

    // Invalid color: text still renders, error is reported
    {
        let (paragraphs, errors) =
            parse_interpolated::<&[_]>(r#"<u><font color="\#a">hello world</font></u>"#, &[]);
        assert_eq!(
            paragraphs,
            [StyledTextParagraph {
                text: "hello world".into(),
                formatting: alloc::vec![
                    FormattedSpan { range: 0..11, style: Style::Color(0) },
                    FormattedSpan { range: 0..11, style: Style::Underline },
                ],
                links: alloc::vec::Vec::new()
            }]
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].to_string(), r"Invalid color value '\#a'");
        assert!(errors[0].range().is_some());
    }
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_parsing_interpolated() {
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("Text: *{MARKDOWN_INTERPOLATION_PLACEHOLDER}*"),
            &[&[paragraph_from_plain_text("italic".into())]]
        )),
        [StyledTextParagraph {
            text: "Text: italic".into(),
            formatting: alloc::vec![FormattedSpan { range: 6..12, style: Style::Emphasis }],
            links: alloc::vec![]
        }]
    );
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("Escaped text: {MARKDOWN_INTERPOLATION_PLACEHOLDER}"),
            &[&[paragraph_from_plain_text("*bold*".into())]]
        )),
        [StyledTextParagraph {
            text: "Escaped text: *bold*".into(),
            formatting: alloc::vec![],
            links: alloc::vec![]
        }]
    );
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("Code block text: `{MARKDOWN_INTERPOLATION_PLACEHOLDER}`"),
            &[&[paragraph_from_plain_text("*bold*".into())]]
        )),
        [StyledTextParagraph {
            text: "Code block text: *bold*".into(),
            formatting: alloc::vec![FormattedSpan { range: 17..23, style: Style::Code }],
            links: alloc::vec![]
        }]
    );
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!(
                "**{MARKDOWN_INTERPOLATION_PLACEHOLDER}** {MARKDOWN_INTERPOLATION_PLACEHOLDER}"
            ),
            &[
                alloc::vec![paragraph_from_plain_text("Hello".into())],
                parse_interpolated::<&[_]>("*World*", &[]).0
            ]
        )),
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
        assert_no_errors(parse_interpolated(
            &format!("<u>{MARKDOWN_INTERPOLATION_PLACEHOLDER}</u>"),
            &[parse_interpolated::<&[_]>("*underline_and_italic*", &[]).0]
        )),
        [StyledTextParagraph {
            text: "underline_and_italic".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..20, style: Style::Emphasis },
                FormattedSpan { range: 0..20, style: Style::Underline },
            ],
            links: alloc::vec![]
        }]
    );
    // Empty paragraph list might be caused by a StyledText::default()
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("{MARKDOWN_INTERPOLATION_PLACEHOLDER}"),
            &[[]]
        )),
        [StyledTextParagraph { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }]
    );
    // Interpolation in link URL
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("[Click here]({MARKDOWN_INTERPOLATION_PLACEHOLDER})"),
            &[&[paragraph_from_plain_text("https://example.com".into())]]
        )),
        [StyledTextParagraph {
            text: "Click here".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..10, style: Style::Link }],
            links: alloc::vec![(0..10, "https://example.com".into())]
        }]
    );
    // Interpolation in link URL with surrounding text
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("[link](https://{MARKDOWN_INTERPOLATION_PLACEHOLDER}/path) after"),
            &[&[paragraph_from_plain_text("example.com".into())]]
        )),
        [StyledTextParagraph {
            text: "link after".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..4, style: Style::Link }],
            links: alloc::vec![(0..4, "https://example.com/path".into())]
        }]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_interleaved_html_and_emphasis() {
    // Issue #11563: interleaved HTML and markdown styles should not panic
    // but should report an error.
    let (_paragraphs, errors) = parse_interpolated::<&[_]>("<u>*</u>*", &[]);
    assert!(errors.iter().any(|e| e.to_string().contains("overlaps with markdown")));

    let (_paragraphs, errors) = parse_interpolated::<&[_]>("<u>*hello</u> world*", &[]);
    assert!(errors.iter().any(|e| e.to_string().contains("overlaps with markdown")));

    // Interleaved HTML-only styles
    let (_paragraphs, errors) =
        parse_interpolated::<&[_]>(r#"<u><font color="red"></u></font>"#, &[]);
    assert!(
        errors.iter().any(|e| e.to_string().contains("Closing html tag")),
        "Expected ClosingTagMismatch, got: {errors:?}"
    );
}
