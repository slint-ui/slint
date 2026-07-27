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
    Superscript,
    Subscript,
    Math,
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichText {
    pub text: alloc::string::String,
    pub formatting: alloc::vec::Vec<FormattedSpan>,
    pub links: alloc::vec::Vec<(core::ops::Range<usize>, alloc::string::String)>,
}

/// Column alignment for tables, matching pulldown_cmark::Alignment values.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TableAlignment {
    #[default]
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableCell {
    pub content: RichText,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParagraphBlock {
    Text(RichText),
    /// level is 1-6
    Heading { level: u8, content: RichText },
    /// level is the nesting depth (1 for `>`, 2 for `>>`, etc.)
    BlockQuote { level: u8, content: RichText },
    HorizontalRule,
    Table {
        columns: usize,
        header: alloc::vec::Vec<alloc::vec::Vec<TableCell>>,
        body: alloc::vec::Vec<alloc::vec::Vec<TableCell>>,
        alignments: alloc::vec::Vec<TableAlignment>,
    },
    CodeBlock { language: Option<alloc::string::String>, text: alloc::string::String },
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
pub fn paragraph_from_plain_text(text: alloc::string::String) -> ParagraphBlock {
    ParagraphBlock::Text(RichText { text, formatting: Default::default(), links: Default::default() })
}

#[cfg(feature = "markdown")]
/// This is the character for private use that is used to make interpolation possible in markdown.
pub const MARKDOWN_INTERPOLATION_PLACEHOLDER: char = '\u{e541}';

#[cfg(feature = "markdown")]
fn begin_paragraph(indentation: u32, list_item_type: Option<ListItemType>) -> ParagraphBlock {
    let mut text = alloc::string::String::with_capacity(indentation as usize * 4);
    for _ in 0..indentation { text.push_str("    "); }
    match list_item_type {
        Some(ListItemType::Unordered) => {
            let remainder = indentation % 3;
            text.push_str(if remainder == 0 { "• " } else if remainder == 1 { "◦ " } else { "▪ " });
        }
        Some(ListItemType::Ordered(num)) => text.push_str(&alloc::format!("{:>3}. ", num)),
        None => {}
    };
    ParagraphBlock::Text(RichText { text, formatting: Default::default(), links: Default::default() })
}

pub fn rich_text_content(block: &ParagraphBlock) -> Option<&RichText> {
    match block {
        ParagraphBlock::Text(content)
        | ParagraphBlock::Heading { content, .. }
        | ParagraphBlock::BlockQuote { content, .. } => Some(content),
        ParagraphBlock::HorizontalRule => None,
        ParagraphBlock::Table { .. } => None,
        ParagraphBlock::CodeBlock { .. } => None,
    }
}

#[cfg(feature = "markdown")]
fn append_rich_text(target: &mut RichText, source: &RichText) {
    let offset = target.text.len();
    target.text.push_str(&source.text);
    target.formatting.extend(source.formatting.iter().cloned().map(|mut f| {
        f.range.start += offset; f.range.end += offset; f
    }));
    target.links.extend(source.links.iter().cloned().map(|(mut range, link)| {
        range.start += offset; range.end += offset; (range, link)
    }));
}

#[cfg(feature = "markdown")]
fn substitute<S: AsRef<[ParagraphBlock]>>(
    paragraph: &mut ParagraphBlock,
    string: &str,
    args: &[S],
    arg_index: &mut usize,
    errors: &mut alloc::vec::Vec<StyledTextParseError>,
    event_range: &core::ops::Range<usize>,
) {
    use StyledTextParseErrorKind as E;
    let ParagraphBlock::Text(rt) = paragraph else { return };
    let mut pos = 0;
    while let Some(mut p) = string[pos..].find(MARKDOWN_INTERPOLATION_PLACEHOLDER) {
        p += pos;
        rt.text.push_str(&string[pos..p]);

        if let Some(arg) = args.get(*arg_index) {
            match arg.as_ref() {
                [source] => {
                    if let Some(source_rt) = rich_text_content(source) {
                        append_rich_text(rt, source_rt);
                    }
                }
                [] => {}
                [first, ..] => {
                    errors.push(StyledTextParseError::new(
                        E::MultiParagraphInterpolation,
                        event_range.clone(),
                    ));
                    if let Some(source_rt) = rich_text_content(first) {
                        append_rich_text(rt, source_rt);
                    }
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
    rt.text.push_str(&string[pos..]);
}

#[cfg(feature = "markdown")]
fn substitute_in_string<S: AsRef<[ParagraphBlock]>>(
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
                [arg_paragraph] => {
                    if let Some(rt) = rich_text_content(arg_paragraph) {
                        result.push_str(&rt.text);
                    }
                }
                [] => {}
                [first, ..] => {
                    errors.push(StyledTextParseError::new(
                        E::MultiParagraphInterpolation,
                        event_range.clone(),
                    ));
                    if let Some(rt) = rich_text_content(first) {
                        result.push_str(&rt.text);
                    }
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
fn get_or_create_paragraph<'a>(
    current_paragraph: &'a mut Option<ParagraphBlock>,
    errors: &mut alloc::vec::Vec<StyledTextParseError>,
    event_range: &core::ops::Range<usize>,
) -> &'a mut ParagraphBlock {
    use StyledTextParseErrorKind as E;
    if current_paragraph.is_none() {
        errors.push(StyledTextParseError::new(E::ParagraphNotStarted, event_range.clone()));
        *current_paragraph = Some(ParagraphBlock::Text(RichText {
            text: Default::default(),
            formatting: Default::default(),
            links: Default::default(),
        }));
    }
    current_paragraph.as_mut().unwrap()
}

#[cfg(feature = "markdown")]
fn unsupported_tag_name(tag: &pulldown_cmark::Tag<'_>) -> alloc::string::String {
    use pulldown_cmark::Tag::*;
    match tag {
        Heading { .. } => "headings",
        // Image { .. } => "images",            // handled inline
        // BlockQuote(_) => "block quotes",     // handled inline
        // Table(_) => "tables",
        HtmlBlock => "HTML blocks",
        // FootnoteDefinition(_) => "footnotes", // handled inline
        DefinitionList | DefinitionListTitle | DefinitionListDefinition => "definition lists",
        // TableHead | TableRow | TableCell => "tables",
        MetadataBlock(_) => "metadata blocks",
        // Superscript => "superscript",
        // Subscript => "subscript",
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
        // FootnoteReference(_) => "footnote references",   // handled inline
        // InlineMath(_) | DisplayMath(_) => "math",        // handled inline
        Html(text) => alloc::format!("HTML blocks ({})", text.trim()),
        _ => alloc::format!("{event:?}"),
    }
}

#[cfg(feature = "markdown")]
pub fn parse_interpolated<S: AsRef<[ParagraphBlock]>>(
    format_string: &str,
    args: &[S],
) -> (alloc::vec::Vec<ParagraphBlock>, alloc::vec::Vec<StyledTextParseError>) {
    use StyledTextParseErrorKind as E;

    let parser = pulldown_cmark::Parser::new_ext(
        format_string,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_FOOTNOTES
            | pulldown_cmark::Options::ENABLE_MATH
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_SUBSCRIPT
            | pulldown_cmark::Options::ENABLE_SUPERSCRIPT,
    );

    let mut list_state_stack: alloc::vec::Vec<Option<u64>> = alloc::vec::Vec::new();
    let mut style_stack: alloc::vec::Vec<(Style, usize, bool)> = alloc::vec::Vec::new();
    let mut current_url = None;
    let mut arg_index = 0;
    let mut paragraphs = alloc::vec::Vec::new();
    let mut errors = alloc::vec::Vec::new();
    // Tracks skipped Start tags whose End events haven't been seen yet.
    // When an End event fails to pop the style stack and this is > 0,
    // we silently consume it instead of reporting a cascading Pop error.
    let mut skip_end_count: usize = 0;
    let mut interleaved_count: usize = 0;
    // When > 0, we are inside a tag whose entire sub-tree should be skipped
    // (e.g. FootnoteDefinition). Every Start/End event increments/decrements
    // this depth; non-tag events (Text, Code, etc.) are silently consumed.
    let mut skip_until_depth: usize = 0;

    let mut in_table = false;
    let mut table_alignments: Vec<TableAlignment> = Vec::new();
    let mut table_header_rows: Vec<Vec<RichText>> = Vec::new();
    let mut table_body_rows: Vec<Vec<RichText>> = Vec::new();
    let mut table_current_row: Vec<RichText> = Vec::new();
    let mut table_current_cell: RichText = RichText::default();
    let mut table_current_cell_style_stack: Vec<(Style, usize)> = Vec::new();
    let mut in_table_head = false;
    let mut table_columns = 0;

    let mut current_heading_level: Option<u8> = None;
    let mut block_quote_level: u32 = 0;

    let mut code_block_language: Option<alloc::string::String> = None;
    let mut code_block_text: Option<alloc::string::String> = None;

    let mut current_paragraph: Option<ParagraphBlock> = None;
    let mut footnote_definitions: Vec<(alloc::string::String, alloc::vec::Vec<ParagraphBlock>)> = Vec::new();
    let mut current_footnote_name: Option<alloc::string::String> = None;
    let mut footnote_start_index: usize = 0;

    for (event, event_range) in parser.into_offset_iter() {
        let indentation = list_state_stack.len().saturating_sub(1) as _;

        if skip_until_depth > 0 {
            match &event {
                pulldown_cmark::Event::Start(_) => skip_until_depth += 1,
                pulldown_cmark::Event::End(_) => skip_until_depth -= 1,
                _ => {}
            }
            continue;
        }

        match event {
            pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                if let Some(paragraph) =
                    current_paragraph.replace(begin_paragraph(indentation, None))
                {
                    if block_quote_level > 0 {
                        if let ParagraphBlock::Text(content) = paragraph {
                            paragraphs.push(ParagraphBlock::BlockQuote {
                                level: block_quote_level as u8,
                                content,
                            });
                            continue;
                        }
                    }
                    paragraphs.push(paragraph);
                }
            }
            pulldown_cmark::Event::Rule => {
                if let Some(paragraph) =
                    current_paragraph.replace(begin_paragraph(indentation, None))
                {
                    paragraphs.push(paragraph);
                }
                paragraphs.push(ParagraphBlock::HorizontalRule);
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableCell) => {
                table_current_row.push(core::mem::take(&mut table_current_cell));
                table_current_cell_style_stack.clear();
                continue;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableRow) => {
                let row = core::mem::take(&mut table_current_row);
                table_columns = table_columns.max(row.len());
                if in_table_head {
                    table_header_rows.push(row);
                } else {
                    table_body_rows.push(row);
                }
                continue;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::TableHead) => {
                in_table_head = false;
                if !table_current_row.is_empty() {
                    table_header_rows.push(core::mem::take(&mut table_current_row));
                }
                continue;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Table) => {
                in_table = false;

                let header: Vec<Vec<TableCell>> = table_header_rows.drain(..).map(|row| {
                    row.into_iter().map(|content| TableCell { content }).collect()
                }).collect();
                let body: Vec<Vec<TableCell>> = table_body_rows.drain(..).map(|row| {
                    row.into_iter().map(|content| TableCell { content }).collect()
                }).collect();

                paragraphs.push(ParagraphBlock::Table {
                    columns: table_columns,
                    header,
                    body,
                    alignments: core::mem::take(&mut table_alignments),
                });
                current_paragraph = None;
                continue;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::List(_)) => {
                if list_state_stack.pop().is_none() {
                    errors.push(StyledTextParseError::new(E::Pop, event_range.clone()));
                }
            }
            pulldown_cmark::Event::End(
                pulldown_cmark::TagEnd::Paragraph | pulldown_cmark::TagEnd::Item,
            ) => {}
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some(level) = current_heading_level.take() {
                    if let Some(ParagraphBlock::Text(content)) = current_paragraph.take() {
                        paragraphs.push(ParagraphBlock::Heading { level, content });
                    }
                }
                current_paragraph = Some(begin_paragraph(indentation, None));
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::BlockQuote(_)) => {
                if let Some(paragraph) = current_paragraph.take() {
                    if let ParagraphBlock::Text(content) = paragraph {
                        paragraphs.push(ParagraphBlock::BlockQuote {
                            level: block_quote_level as u8,
                            content,
                        });
                    }
                }
                block_quote_level = block_quote_level.saturating_sub(1);
                current_paragraph = None;
            }
            pulldown_cmark::Event::Start(tag) => {
                let style = match tag {
                    pulldown_cmark::Tag::Paragraph => {
                        if let Some(paragraph) =
                            current_paragraph.replace(begin_paragraph(indentation, None))
                        {
                            if block_quote_level > 0 {
                                if let ParagraphBlock::Text(content) = paragraph {
                                    paragraphs.push(ParagraphBlock::BlockQuote {
                                        level: block_quote_level as u8,
                                        content,
                                    });
                                    continue;
                                }
                            }
                            paragraphs.push(paragraph);
                        }
                        continue;
                    }
                    pulldown_cmark::Tag::Item => {
                        let old_paragraph = current_paragraph.replace(begin_paragraph(
                            indentation,
                            Some(match list_state_stack.last().copied() {
                                Some(Some(index)) => ListItemType::Ordered(index),
                                _ => ListItemType::Unordered,
                            }),
                        ));
                        if let Some(state) = list_state_stack.last_mut() {
                            *state = state.map(|state| state + 1);
                        }
                        if let Some(paragraph) = old_paragraph {
                            paragraphs.push(paragraph);
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

                    pulldown_cmark::Tag::BlockQuote(_) => {
                        block_quote_level += 1;
                        continue;
                    }
                    pulldown_cmark::Tag::HtmlBlock => {
                        // Don't report an error here; the accompanying Html event
                        // provides a more descriptive message with the actual content
                        skip_end_count += 1;
                        continue;
                    }
                    pulldown_cmark::Tag::Heading { level, .. } => {
                        if let Some(paragraph) =
                            current_paragraph.replace(begin_paragraph(indentation, None))
                        {
                            paragraphs.push(paragraph);
                        }
                        current_heading_level = Some(level as u8);
                        continue;
                    }

                    pulldown_cmark::Tag::Superscript => Style::Superscript,
                    pulldown_cmark::Tag::Subscript => Style::Subscript,

                    pulldown_cmark::Tag::Table(alignments) => {
                        in_table = true;
                        table_alignments = alignments.iter().map(|a| match a {
                            pulldown_cmark::Alignment::None => TableAlignment::None,
                            pulldown_cmark::Alignment::Left => TableAlignment::Left,
                            pulldown_cmark::Alignment::Center => TableAlignment::Center,
                            pulldown_cmark::Alignment::Right => TableAlignment::Right,
                        }).collect();
                        table_header_rows = Vec::new();
                        table_body_rows = Vec::new();
                        table_current_row = Vec::new();
                        table_current_cell = RichText::default();
                        table_current_cell_style_stack = Vec::new();
                        in_table_head = false;
                        table_columns = 0;
                        continue;
                    }
                    pulldown_cmark::Tag::TableHead => {
                        in_table_head = true;
                        continue;
                    }
                    pulldown_cmark::Tag::TableRow => {
                        table_current_row = Vec::new();
                        continue;
                    }
                    pulldown_cmark::Tag::TableCell => {
                        table_current_cell = RichText::default();
                        table_current_cell_style_stack.clear();
                        continue;
                    }

                    pulldown_cmark::Tag::FootnoteDefinition(name) => {
                        if let Some(paragraph) = current_paragraph.take() {
                            paragraphs.push(paragraph);
                        }
                        current_footnote_name = Some(name.into());
                        footnote_start_index = paragraphs.len();
                        continue;
                    }
                    pulldown_cmark::Tag::Image { .. } => {
                        skip_end_count += 1;
                        continue;
                    }
                    pulldown_cmark::Tag::CodeBlock(kind) => {
                        if let Some(paragraph) =
                            current_paragraph.replace(begin_paragraph(indentation, None))
                        {
                            paragraphs.push(paragraph);
                        }
                        let language = match kind {
                            pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                                let l = lang.trim();
                                if l.is_empty() { None } else { Some(l.into()) }
                            }
                            pulldown_cmark::CodeBlockKind::Indented => None,
                        };
                        code_block_language = language;
                        code_block_text = Some(alloc::string::String::new());
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

                if in_table {
                    let start = table_current_cell.text.len();
                    table_current_cell_style_stack.push((style, start));
                } else {
                    let ParagraphBlock::Text(rt) =
                        get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range)
                    else { unreachable!() };

                    style_stack.push((style, rt.text.len(), false));
                }
            }
            pulldown_cmark::Event::Text(text) => {
                if in_table {
                    table_current_cell.text.push_str(&text);
                    continue;
                }
                if let Some(ref mut code_text) = code_block_text {
                    code_text.push_str(&text);
                    continue;
                }
                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);

                substitute(paragraph, &text, args, &mut arg_index, &mut errors, &event_range);
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::FootnoteDefinition) => {
                if let Some(name) = current_footnote_name.take() {
                    if let Some(paragraph) = current_paragraph.take() {
                        paragraphs.push(paragraph);
                    }
                    let footnote_paras: Vec<ParagraphBlock> =
                        if footnote_start_index < paragraphs.len() {
                            paragraphs.drain(footnote_start_index..).collect()
                        } else {
                            Vec::new()
                        };
                    footnote_definitions.push((name, footnote_paras));
                }
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                skip_end_count = skip_end_count.saturating_sub(1);
                if let Some(text) = code_block_text.take() {
                    let language = code_block_language.take();
                    paragraphs.push(ParagraphBlock::CodeBlock { language, text });
                }
                continue;
            }
            pulldown_cmark::Event::End(_) => {
                if in_table {
                    if let Some((style, start)) = table_current_cell_style_stack.pop() {
                        table_current_cell.formatting.push(FormattedSpan {
                            range: start..table_current_cell.text.len(),
                            style,
                        });
                    }
                    continue;
                }
                let (style, start, _from_html) = if let Some(value) = style_stack.pop() {
                    value
                } else if skip_end_count > 0 {
                    skip_end_count -= 1;
                    continue;
                } else {
                    errors.push(StyledTextParseError::new(E::Pop, event_range.clone()));
                    continue;
                };

                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                let end = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);

                if let Some(url) = current_url.take() {
                    let url = if url.contains(MARKDOWN_INTERPOLATION_PLACEHOLDER) {
                        substitute_in_string(&url, args, &mut arg_index, &mut errors, &event_range)
                    } else {
                        url.into()
                    };
                    if let ParagraphBlock::Text(rt) = paragraph {
                        rt.links.push((start..end, url));
                    }
                }

                if let ParagraphBlock::Text(rt) = paragraph {
                    rt.formatting.push(FormattedSpan { range: start..end, style });
                }
            }
            pulldown_cmark::Event::Code(text) => {
                if in_table {
                    let start = table_current_cell.text.len();
                    table_current_cell.text.push_str(&text);
                    table_current_cell.formatting.push(FormattedSpan {
                        range: start..table_current_cell.text.len(),
                        style: Style::Code,
                    });
                    continue;
                }
                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                let start = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);

                substitute(paragraph, &text, args, &mut arg_index, &mut errors, &event_range);
                if let ParagraphBlock::Text(rt) = paragraph {
                    rt.formatting
                        .push(FormattedSpan { range: start..rt.text.len(), style: Style::Code });
                }
            }
            pulldown_cmark::Event::InlineHtml(html) => {
                if html.starts_with("</") {
                    let (style, start, from_html) = if let Some(value) = style_stack.pop() {
                        value
                    } else if skip_end_count > 0 {
                        skip_end_count -= 1;
                        continue;
                    } else {
                        errors.push(StyledTextParseError::new(E::Pop, event_range.clone()));
                        continue;
                    };

                    if !from_html {
                        // The top of the stack is a markdown style, not
                        // the expected HTML style. Push it back and report
                        // an error instead of consuming it (issue #11563).
                        style_stack.push((style, start, from_html));
                        interleaved_count += 1;
                        errors.push(StyledTextParseError::new(
                            E::InterleavedStyles((&*html).into()),
                            event_range.clone(),
                        ));
                        continue;
                    }

                    let is_valid_close = match &style {
                        Style::Color(_) => (&*html) == "</font>",
                        Style::Underline => (&*html) == "</u>",
                        Style::Strikethrough => matches!(&*html, "</s>" | "</del>"),
                        Style::Subscript => (&*html) == "</sub>",
                        Style::Superscript => (&*html) == "</sup>",
                        Style::Emphasis => matches!(&*html, "</i>" | "</em>"),
                        Style::Strong => matches!(&*html, "</b>" | "</strong>"),
                        _ => {
                            // HTML-pushed style we don't know how to close
                            style_stack.push((style, start, from_html));
                            interleaved_count += 1;
                            errors.push(StyledTextParseError::new(
                                E::InterleavedStyles((&*html).into()),
                                event_range.clone(),
                            ));
                            continue;
                        }
                    };

                    if !is_valid_close {
                        let expected_tag = match &style {
                            Style::Color(_) => "</font>",
                            Style::Underline => "</u>",
                            Style::Strikethrough => "</s> or </del>",
                            Style::Emphasis => "</i> or </em>",
                            Style::Strong => "</b> or </strong>",
                            _ => unreachable!(),
                        };
                        errors.push(StyledTextParseError::new(
                            E::ClosingTagMismatch(expected_tag.into(), (&*html).into()),
                            event_range.clone(),
                        ));
                        // Still apply the style as best-effort
                    }

                    let paragraph =
                        get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                    let end = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                    if let ParagraphBlock::Text(rt) = paragraph {
                        rt.formatting.push(FormattedSpan { range: start..end, style });
                    }
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
                                    let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                    style_stack.push((Style::Underline, len, true));
                                }
                                "font" => {
                                    expecting_color_attribute = true;
                                }
                                "s" | "del" => {
                                    let paragraph = get_or_create_paragraph(
                                        &mut current_paragraph,
                                        &mut errors,
                                        &event_range,
                                    );
                                    let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                    style_stack.push((Style::Strikethrough, len, true));
                                }
                                "i" | "em" => {
                                    let paragraph = get_or_create_paragraph(
                                        &mut current_paragraph,
                                        &mut errors,
                                        &event_range,
                                    );
                                    let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                    style_stack.push((Style::Emphasis, len, true));
                                }
                                "b" | "strong" => {
                                    let paragraph = get_or_create_paragraph(
                                        &mut current_paragraph,
                                        &mut errors,
                                        &event_range,
                                    );
                                    let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                    style_stack.push((Style::Strong, len, true));
                                }
                                "sub" => {
                                    let paragraph = get_or_create_paragraph(
                                        &mut current_paragraph,
                                        &mut errors,
                                        &event_range,
                                    );
                                    let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                    style_stack.push((Style::Subscript, len, true));
                                }
                                "sup" => {
                                    let paragraph = get_or_create_paragraph(
                                        &mut current_paragraph,
                                        &mut errors,
                                        &event_range,
                                    );
                                    let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                    style_stack.push((Style::Superscript, len, true));
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

                                    match color_value {
                                        Some(value) => {
                                            let paragraph = get_or_create_paragraph(
                                                &mut current_paragraph,
                                                &mut errors,
                                                &event_range,
                                            );
                                            let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                            style_stack.push((Style::Color(value), len, true));
                                        }
                                        None => {
                                            let r = base + span.start()..base + span.end();
                                            errors.push(StyledTextParseError::new(
                                                E::InvalidColor(color_str.into()),
                                                r,
                                            ));
                                            // Push a dummy style so the closing </font> tag
                                            // can pop it without error
                                            let paragraph = get_or_create_paragraph(
                                                &mut current_paragraph,
                                                &mut errors,
                                                &event_range,
                                            );
                                            let len = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                                            style_stack.push((Style::Color(0), len, true));
                                        }
                                    }
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
                            Ok(htmlparser::Token::Comment { .. }) => {}
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
            pulldown_cmark::Event::FootnoteReference(name) => {
                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                let start = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                let ref_text = alloc::format!("[{}]", name);
                substitute(paragraph, &ref_text, args, &mut arg_index, &mut errors, &event_range);
                if let ParagraphBlock::Text(rt) = paragraph {
                    rt.formatting.push(FormattedSpan {
                        range: start..rt.text.len(),
                        style: Style::Superscript,
                    });
                }
            }
            pulldown_cmark::Event::InlineMath(text) => {
                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                let start = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                substitute(paragraph, &text, args, &mut arg_index, &mut errors, &event_range);
                if let ParagraphBlock::Text(rt) = paragraph {
                    rt.formatting.push(FormattedSpan {
                        range: start..rt.text.len(),
                        style: Style::Math,
                    });
                }
            }
            pulldown_cmark::Event::DisplayMath(text) => {
                if let Some(paragraph) =
                    current_paragraph.replace(begin_paragraph(indentation, None))
                {
                    paragraphs.push(paragraph);
                }
                let paragraph =
                    get_or_create_paragraph(&mut current_paragraph, &mut errors, &event_range);
                let start = rich_text_content(paragraph).map(|r| r.text.len()).unwrap_or(0);
                substitute(paragraph, &text, args, &mut arg_index, &mut errors, &event_range);
                if let ParagraphBlock::Text(rt) = paragraph {
                    rt.formatting.push(FormattedSpan {
                        range: start..rt.text.len(),
                        style: Style::Math,
                    });
                }
            }
            pulldown_cmark::Event::Html(ref html) => {
                // HTML comments <!-- ... --> are silently skipped
                if html.starts_with("<!--") {
                    // skip
                } else {
                    errors.push(StyledTextParseError::new(
                        E::UnsupportedMarkdown(unsupported_event_name(&event)),
                        event_range,
                    ));
                }
            }
            pulldown_cmark::Event::TaskListMarker(_) => {
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

    if let Some(level) = current_heading_level.take() {
        if let Some(ParagraphBlock::Text(content)) = current_paragraph.take() {
            paragraphs.push(ParagraphBlock::Heading { level, content });
        }
    } else if let Some(paragraph) = current_paragraph.take() {
        paragraphs.push(paragraph);
    }

    if !footnote_definitions.is_empty() {
        paragraphs.push(ParagraphBlock::HorizontalRule);
        for (name, content) in footnote_definitions {
            let ref_text = alloc::format!("[{}] ", name);
            let mut rt = RichText::default();
            rt.text = ref_text;
            paragraphs.push(ParagraphBlock::Text(rt));
            paragraphs.extend(content);
        }
    }

    (paragraphs, errors)
}

#[cfg(all(feature = "markdown", test))]
fn assert_no_errors(
    result: (alloc::vec::Vec<ParagraphBlock>, alloc::vec::Vec<StyledTextParseError>),
) -> alloc::vec::Vec<ParagraphBlock> {
    let (paragraphs, errors) = result;
    assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    paragraphs
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_parsing() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("hello *world*", &[])),
        [ParagraphBlock::Text(RichText {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 6..11, style: Style::Emphasis }],
            links: alloc::vec::Vec::new()
        })]
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
            ParagraphBlock::Text(RichText {
                text: "• line 1".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }),
            ParagraphBlock::Text(RichText {
                text: "• line 2".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            })
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
            ParagraphBlock::Text(RichText {
                text: "  1. a".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }),
            ParagraphBlock::Text(RichText {
                text: "  2. b".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }),
            ParagraphBlock::Text(RichText {
                text: "  3. c".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            })
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
            ParagraphBlock::Text(RichText {
                text: "Normal italic strong strikethrough code".into(),
                formatting: alloc::vec![
                    FormattedSpan { range: 7..13, style: Style::Emphasis },
                    FormattedSpan { range: 14..20, style: Style::Strong },
                    FormattedSpan { range: 21..34, style: Style::Strikethrough },
                    FormattedSpan { range: 35..39, style: Style::Code }
                ],
                links: alloc::vec::Vec::new()
            }),
            ParagraphBlock::Text(RichText {
                text: "new line".into(),
                formatting: alloc::vec![FormattedSpan { range: 4..8, style: Style::Emphasis },],
                links: alloc::vec::Vec::new()
            })
        ]
    );

    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>(
            "<s>strike</s> <i>italic</i> <b>bold</b> <del>del</del> <em>em</em> <strong>strong</strong>",
            &[]
        )),
        [ParagraphBlock::Text(RichText {
            text: "strike italic bold del em strong".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..6, style: Style::Strikethrough },
                FormattedSpan { range: 7..13, style: Style::Emphasis },
                FormattedSpan { range: 14..18, style: Style::Strong },
                FormattedSpan { range: 19..22, style: Style::Strikethrough },
                FormattedSpan { range: 23..25, style: Style::Emphasis },
                FormattedSpan { range: 26..32, style: Style::Strong },
            ],
            links: alloc::vec![],
        })]
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
            ParagraphBlock::Text(RichText {
                text: "• root".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }),
            ParagraphBlock::Text(RichText {
                text: "    ◦ child".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }),
            ParagraphBlock::Text(RichText {
                text: "        ▪ grandchild".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }),
            ParagraphBlock::Text(RichText {
                text: "            • great grandchild".into(),
                formatting: alloc::vec::Vec::new(),
                links: alloc::vec::Vec::new()
            }),
        ]
    );

    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("hello [*world*](https://example.com)", &[])),
        [ParagraphBlock::Text(RichText {
            text: "hello world".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 6..11, style: Style::Emphasis },
                FormattedSpan { range: 6..11, style: Style::Link }
            ],
            links: alloc::vec![(6..11, "https://example.com".into())]
        })]
    );

    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("<u>hello world</u>", &[])),
        [ParagraphBlock::Text(RichText {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..11, style: Style::Underline },],
            links: alloc::vec::Vec::new()
        })]
    );

    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>(
            r#"<font color="blue">hello world</font>"#,
            &[]
        )),
        [ParagraphBlock::Text(RichText {
            text: "hello world".into(),
            formatting: alloc::vec![FormattedSpan {
                range: 0..11,
                style: Style::Color(0xff_00_00_ff)
            },],
            links: alloc::vec::Vec::new()
        })]
    );

    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>(
            r#"<u><font color="red">hello world</font></u>"#,
            &[]
        )),
        [ParagraphBlock::Text(RichText {
            text: "hello world".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..11, style: Style::Color(0xff_ff_00_00) },
                FormattedSpan { range: 0..11, style: Style::Underline },
            ],
            links: alloc::vec::Vec::new()
        })]
    );

    // Invalid color: text still renders, error is reported
    {
        let (paragraphs, errors) =
            parse_interpolated::<&[_]>(r#"<u><font color="\#a">hello world</font></u>"#, &[]);
        assert_eq!(
            paragraphs,
            [ParagraphBlock::Text(RichText {
                text: "hello world".into(),
                formatting: alloc::vec![
                    FormattedSpan { range: 0..11, style: Style::Color(0) },
                    FormattedSpan { range: 0..11, style: Style::Underline },
                ],
                links: alloc::vec::Vec::new()
            })]
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
        [ParagraphBlock::Text(RichText {
            text: "Text: italic".into(),
            formatting: alloc::vec![FormattedSpan { range: 6..12, style: Style::Emphasis }],
            links: alloc::vec![]
        })]
    );
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("Escaped text: {MARKDOWN_INTERPOLATION_PLACEHOLDER}"),
            &[&[paragraph_from_plain_text("*bold*".into())]]
        )),
        [ParagraphBlock::Text(RichText {
            text: "Escaped text: *bold*".into(),
            formatting: alloc::vec![],
            links: alloc::vec![]
        })]
    );
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("Code block text: `{MARKDOWN_INTERPOLATION_PLACEHOLDER}`"),
            &[&[paragraph_from_plain_text("*bold*".into())]]
        )),
        [ParagraphBlock::Text(RichText {
            text: "Code block text: *bold*".into(),
            formatting: alloc::vec![FormattedSpan { range: 17..23, style: Style::Code }],
            links: alloc::vec![]
        })]
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
        [ParagraphBlock::Text(RichText {
            text: "Hello World".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..5, style: Style::Strong },
                FormattedSpan { range: 6..11, style: Style::Emphasis }
            ],
            links: alloc::vec![]
        })]
    );
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("<u>{MARKDOWN_INTERPOLATION_PLACEHOLDER}</u>"),
            &[parse_interpolated::<&[_]>("*underline_and_italic*", &[]).0]
        )),
        [ParagraphBlock::Text(RichText {
            text: "underline_and_italic".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..20, style: Style::Emphasis },
                FormattedSpan { range: 0..20, style: Style::Underline },
            ],
            links: alloc::vec![]
        })]
    );
    // Empty paragraph list might be caused by a StyledText::default()
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("{MARKDOWN_INTERPOLATION_PLACEHOLDER}"),
            &[[]]
        )),
        [ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] })]
    );
    // Interpolation in link URL
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("[Click here]({MARKDOWN_INTERPOLATION_PLACEHOLDER})"),
            &[&[paragraph_from_plain_text("https://example.com".into())]]
        )),
        [ParagraphBlock::Text(RichText {
            text: "Click here".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..10, style: Style::Link }],
            links: alloc::vec![(0..10, "https://example.com".into())]
        })]
    );
    // Interpolation in link URL with surrounding text
    assert_eq!(
        assert_no_errors(parse_interpolated(
            &format!("[link](https://{MARKDOWN_INTERPOLATION_PLACEHOLDER}/path) after"),
            &[&[paragraph_from_plain_text("example.com".into())]]
        )),
        [ParagraphBlock::Text(RichText {
            text: "link after".into(),
            formatting: alloc::vec![FormattedSpan { range: 0..4, style: Style::Link }],
            links: alloc::vec![(0..4, "https://example.com/path".into())]
        })]
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

#[cfg(feature = "markdown")]
#[test]
fn markdown_heading_levels() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6", &[])),
        [
            ParagraphBlock::Heading { level: 1, content: RichText { text: "H1".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Heading { level: 2, content: RichText { text: "H2".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Heading { level: 3, content: RichText { text: "H3".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Heading { level: 4, content: RichText { text: "H4".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Heading { level: 5, content: RichText { text: "H5".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Heading { level: 6, content: RichText { text: "H6".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
        ]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_heading_with_inline() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("# *Italic* heading", &[])),
        [
            ParagraphBlock::Heading {
                level: 1,
                content: RichText {
                    text: "Italic heading".into(),
                    formatting: alloc::vec![FormattedSpan { range: 0..6, style: Style::Emphasis }],
                    links: alloc::vec![],
                }
            },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
        ]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_subscript_superscript() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("~sub~ and ^super^", &[])),
        [ParagraphBlock::Text(RichText {
            text: "sub and super".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 0..3, style: Style::Subscript },
                FormattedSpan { range: 8..13, style: Style::Superscript },
            ],
            links: alloc::vec![],
        })]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_heading_and_text() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("# Title\n\nBody text", &[])),
        [
            ParagraphBlock::Heading { level: 1, content: RichText { text: "Title".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Text(RichText { text: "Body text".into(), formatting: alloc::vec![], links: alloc::vec![] }),
        ]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_horizontal_rules() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("---\n\n***\n\n___", &[])),
        [
            ParagraphBlock::HorizontalRule,
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::HorizontalRule,
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::HorizontalRule,
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
        ]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_block_quote() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("> Block quote text", &[])),
        [ParagraphBlock::BlockQuote {
            level: 1,
            content: RichText {
                text: "Block quote text".into(),
                formatting: alloc::vec![],
                links: alloc::vec![],
            }
        }]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_block_quote_multi_paragraph() {
    let result = assert_no_errors(parse_interpolated::<&[_]>(
        "> First paragraph\n>\n> Second paragraph",
        &[]
    ));
    assert_eq!(result.len(), 2);
    assert!(matches!(result[0], ParagraphBlock::BlockQuote { level: 1, .. }));
    assert!(matches!(result[1], ParagraphBlock::BlockQuote { level: 1, .. }));
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_block_quote_with_formatting() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("> *Italic* in quote", &[])),
        [ParagraphBlock::BlockQuote {
            level: 1,
            content: RichText {
                text: "Italic in quote".into(),
                formatting: alloc::vec![FormattedSpan { range: 0..6, style: Style::Emphasis }],
                links: alloc::vec![],
            }
        }]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_mixed_heading_and_rules() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("# Title\n\nContent\n\n---\n\n## Subtitle", &[])),
        [
            ParagraphBlock::Heading { level: 1, content: RichText { text: "Title".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Text(RichText { text: "Content".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::HorizontalRule,
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
            ParagraphBlock::Heading { level: 2, content: RichText { text: "Subtitle".into(), formatting: alloc::vec![], links: alloc::vec![] } },
            ParagraphBlock::Text(RichText { text: "".into(), formatting: alloc::vec![], links: alloc::vec![] }),
        ]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_table_simple() {
    let result = assert_no_errors(parse_interpolated::<&[_]>(
        "| H1 | H2 |\n| --- | --- |\n| A1 | A2 |",
        &[]
    ));
    assert_eq!(result.len(), 1);
    assert!(matches!(result[0], ParagraphBlock::Table { .. }));
    if let ParagraphBlock::Table { columns, ref header, ref body, .. } = result[0] {
        assert_eq!(columns, 2);
        assert_eq!(header.len(), 1);
        assert_eq!(body.len(), 1);
        assert_eq!(header[0][0].content.text, "H1");
        assert_eq!(body[0][0].content.text, "A1");
    }
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_footnote_reference() {
    let result = assert_no_errors(parse_interpolated::<&[_]>(
        "Text with a footnote[^1]\n\n[^1]: The footnote content",
        &[]
    ));
    assert_eq!(result.len(), 4);
    assert_eq!(result[0], ParagraphBlock::Text(RichText {
        text: "Text with a footnote[1]".into(),
        formatting: alloc::vec![FormattedSpan { range: 20..23, style: Style::Superscript }],
        links: alloc::vec![],
    }));
    assert_eq!(result[1], ParagraphBlock::HorizontalRule);
    assert_eq!(result[2], ParagraphBlock::Text(RichText {
        text: "[1] ".into(),
        formatting: alloc::vec![],
        links: alloc::vec![],
    }));
    assert_eq!(result[3], ParagraphBlock::Text(RichText {
        text: "The footnote content".into(),
        formatting: alloc::vec![],
        links: alloc::vec![],
    }));
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_math_inline() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("Math: $E=mc^2$", &[])),
        [ParagraphBlock::Text(RichText {
            text: "Math: E=mc^2".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 6..12, style: Style::Math },
            ],
            links: alloc::vec![],
        })]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_image() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("An image ![alt text](url.png)", &[])),
        [ParagraphBlock::Text(RichText {
            text: "An image alt text".into(),
            formatting: alloc::vec![],
            links: alloc::vec![],
        })]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_code_block() {
    let empty: &[&[ParagraphBlock]] = &[];
    let result = assert_no_errors(parse_interpolated(
        "Text before\n\n```\ncode block\n```\n\nText after",
        empty,
    ));
    assert!(
        result.iter().any(|b| matches!(b, ParagraphBlock::CodeBlock { text, .. } if text == "code block\n")),
        "Expected a CodeBlock with text 'code block\\n', got: {result:?}"
    );
    let code_blocks: alloc::vec::Vec<_> = result.iter().filter(|b| matches!(b, ParagraphBlock::CodeBlock { .. })).collect();
    assert_eq!(code_blocks.len(), 1);
    if let ParagraphBlock::CodeBlock { language, .. } = &code_blocks[0] {
        assert_eq!(*language, None);
    }
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_code_block_with_language() {
    let empty: &[&[ParagraphBlock]] = &[];
    let result = assert_no_errors(parse_interpolated(
        "```rust\nfn main() {}\n```",
        empty,
    ));
    assert!(
        result.iter().any(|b| matches!(b, ParagraphBlock::CodeBlock { language: Some(lang), text } if lang == "rust" && text.contains("fn main"))),
        "Expected a CodeBlock with language 'rust' containing 'fn main', got: {result:?}"
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_code_block_indented() {
    let empty: &[&[ParagraphBlock]] = &[];
    let result = assert_no_errors(parse_interpolated(
        "    indented code block",
        empty,
    ));
    assert!(
        result.iter().any(|b| matches!(b, ParagraphBlock::CodeBlock { .. })),
        "Expected a CodeBlock, got: {result:?}"
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_html_sub_sup() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("H<sub>2</sub>O and E=mc<sup>2</sup>", &[])),
        [ParagraphBlock::Text(RichText {
            text: "H2O and E=mc2".into(),
            formatting: alloc::vec![
                FormattedSpan { range: 1..2, style: Style::Subscript },
                FormattedSpan { range: 12..13, style: Style::Superscript },
            ],
            links: alloc::vec![],
        })]
    );
}

#[cfg(feature = "markdown")]
#[test]
fn markdown_html_comment() {
    assert_eq!(
        assert_no_errors(parse_interpolated::<&[_]>("before<!-- comment --> after", &[])),
        [ParagraphBlock::Text(RichText {
            text: "before after".into(),
            formatting: alloc::vec![],
            links: alloc::vec![],
        })]
    );
}
