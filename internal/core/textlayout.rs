// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! module for basic text layout
//!
//! The basic algorithm for breaking text into multiple lines:
//! 1. First we determine the boundaries for text shaping. As shaping happens based on a single font and we know that different fonts cater different
//!    writing systems, we split up the text into chunks that maximize our chances of finding a font that covers all glyphs in the chunk. This way for
//!    example arabic text can be covered by a font that has excellent arabic coverage while latin text is rendered using a different font.
//!    Shaping boundaries are always also grapheme boundaries.
//! 2. Then we shape the text at shaping boundaries, to determine the metrics of glyphs and glyph clusters (grapheme boundaries with the shapable)
//! 3. Allocate graphemes into new text lines until all graphemes are consumed:
//! 4. Loop over all graphemes:
//!     Compute the width of the grapheme
//!     Determine if the grapheme is produced by a white space character
//!     If grapheme is not at break opportunity:
//!         Add grapheme to fragment
//!         If width of current line <= available width AND width of current line + fragment > available width:
//!             Emit current line
//!             Current line starts with fragment
//!             Clear fragment
//!         Else:
//!              Continue
//!     Else if break opportunity at grapheme boundary is optional OR if current is space and next is optional:
//!         If width of current line + fragment <= available width:
//!              Add fragment to current line
//!              Clear fragment
//!         Else:
//!              Emit current line
//!              Current line starts with fragment
//!              Clear fragment
//!         Add grapheme to fragment
//!             
//!     Else if break opportunity at grapheme boundary is mandatory:
//!         Add fragment to current line
//!         Emit current line
//!         Clear fragment
//!         Add grapheme to fragment
//!

use core::ops::Range;

use alloc::boxed::Box;
use alloc::vec::Vec;

use euclid::num::{One, Zero};

use crate::items::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap};

pub trait GlyphMetrics<Length> {
    fn advance_x(&self) -> Length;
    fn byte_offset(&self) -> usize;
}

pub trait TextShaper {
    type LengthPrimitive: core::ops::Mul
        + core::ops::Div
        + core::ops::Add<Output = Self::LengthPrimitive>
        + core::ops::AddAssign
        + euclid::num::Zero
        + euclid::num::One
        + core::convert::From<i16>
        + Copy;
    type Length: euclid::num::Zero
        + core::ops::AddAssign
        + core::ops::Add<Output = Self::Length>
        + core::ops::Sub<Output = Self::Length>
        + Default
        + Clone
        + Copy
        + core::cmp::PartialOrd
        + core::ops::Mul<Self::LengthPrimitive, Output = Self::Length>
        + core::ops::Div<Self::LengthPrimitive, Output = Self::Length>;
    type Glyph: GlyphMetrics<Self::Length>;
    fn shape_text<GlyphStorage: core::iter::Extend<Self::Glyph>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    );
    fn glyph_for_char(&self, ch: char) -> Option<Self::Glyph>;
}

pub struct ShapeBoundaries<'a> {
    text: &'a str,
    // TODO: We should do a better analysis to find boundaries for text shaping; including
    // boundaries when the bidi level changes, the script changes or an explicit separator like
    // paragraph/lineseparator/space is encountered.
    chars: core::str::CharIndices<'a>,
    next_boundary_start: Option<usize>,
    last_script: Option<unicode_script::Script>,
}

impl<'a> ShapeBoundaries<'a> {
    pub fn new(text: &'a str) -> Self {
        let chars = text.char_indices();
        let next_boundary_start = if !text.is_empty() { Some(0) } else { None };
        Self { text, chars, next_boundary_start, last_script: None }
    }
}

impl<'a> Iterator for ShapeBoundaries<'a> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.next_boundary_start?;

        use unicode_script::UnicodeScript;
        let (next_offset, script) = loop {
            match self.chars.next() {
                Some((byte_offset, ch)) => {
                    let next_script = ch.script();
                    let previous_script = *self.last_script.get_or_insert(next_script);

                    if next_script == previous_script {
                        continue;
                    }
                    if matches!(
                        next_script,
                        unicode_script::Script::Unknown
                            | unicode_script::Script::Common
                            | unicode_script::Script::Inherited,
                    ) {
                        continue;
                    }

                    break (Some(byte_offset), Some(next_script));
                }
                None => {
                    break (None, None);
                }
            }
        };

        let item = Range { start, end: next_offset.unwrap_or(self.text.len()) };

        self.last_script = script;
        self.next_boundary_start = next_offset;

        Some(item)
    }
}

#[derive(Clone, Default, Debug)]
struct Whitespace<Length: Default + Clone> {
    // size in bytes in the text
    len: usize,
    // width in pixels
    width: Length,
}

#[derive(Clone, Default, Debug)]
pub struct TextLine<Length: Default + Clone> {
    // The range excludes trailing whitespace
    byte_range: Range<usize>,
    trailing_whitespace: Option<Whitespace<Length>>,
    text_width: Length, // with as occupied by the glyphs
}

impl<Length: Default + Copy + Clone + Zero + core::ops::Add<Output = Length>> TextLine<Length> {
    pub fn line_text<'a>(&self, paragraph: &'a str) -> &'a str {
        &paragraph[self.byte_range.clone()]
    }

    pub fn width_including_trailing_whitespace(&self) -> Length {
        self.text_width + self.trailing_whitespace.as_ref().map_or(Length::zero(), |ws| ws.width)
    }
}

#[derive(Clone)]
struct Grapheme<Length> {
    byte_range: Range<usize>,
    width: Length,
    is_whitespace: bool,
}

impl<Length: Clone + Copy + Default + core::ops::AddAssign> TextLine<Length> {
    fn add_grapheme(&mut self, grapheme: &Grapheme<Length>) {
        if self.byte_range.is_empty() {
            if grapheme.is_whitespace {
                return;
            } else {
                self.byte_range.start = grapheme.byte_range.start;
                self.byte_range.end = self.byte_range.start;
            }
        }

        match (self.trailing_whitespace.as_mut(), grapheme.is_whitespace) {
            (Some(existing_trailing_whitespace), true) => {
                existing_trailing_whitespace.len += grapheme.byte_range.len();
                existing_trailing_whitespace.width += grapheme.width;
            }
            (None, true) => {
                self.trailing_whitespace =
                    Some(Whitespace { len: grapheme.byte_range.len(), width: grapheme.width });
            }
            (Some(_), false) => {
                let Whitespace { len: whitespace_len, width: whitespace_width } =
                    self.trailing_whitespace.take().unwrap();

                self.byte_range.end += whitespace_len;
                self.text_width += whitespace_width;
                self.trailing_whitespace = None;

                // There should not be any gaps between the whitespace and the added grapheme
                debug_assert_eq!(self.byte_range.end, grapheme.byte_range.start);
                self.byte_range.end += grapheme.byte_range.len();

                self.text_width += grapheme.width;
            }
            (None, false) => {
                if !self.byte_range.is_empty() {
                    // There should not be any gaps between the whitespace and the added grapheme
                    debug_assert_eq!(self.byte_range.end, grapheme.byte_range.start);
                }
                self.byte_range.end += grapheme.byte_range.len();

                self.text_width += grapheme.width;
            }
        }
    }
    fn add_line(&mut self, candidate: &mut Self) {
        if candidate.byte_range.is_empty() && candidate.trailing_whitespace.is_none() {
            return;
        }

        if self.byte_range.is_empty() && self.trailing_whitespace.is_none() {
            self.byte_range.start = candidate.byte_range.start;
            self.byte_range.end = self.byte_range.start;
        }

        match (self.trailing_whitespace.as_mut(), candidate.trailing_whitespace.as_ref()) {
            (Some(existing_trailing_whitespace), Some(new_trailing_whitespace)) => {
                existing_trailing_whitespace.len += new_trailing_whitespace.len;
                existing_trailing_whitespace.width += new_trailing_whitespace.width;
            }
            (None, Some(new_trailing_whitespace)) => {
                self.trailing_whitespace = Some(new_trailing_whitespace.clone());
            }
            (Some(_), None) => {
                let Whitespace { len: whitespace_len, width: whitespace_width } =
                    self.trailing_whitespace.take().unwrap();
                self.byte_range.end += whitespace_len;
                self.text_width += whitespace_width;
            }
            (None, None) => {}
        }

        self.byte_range.end = candidate.byte_range.end;

        self.text_width += candidate.text_width;
        *candidate = Default::default();
    }
}

struct GraphemeCursor<'a, Font: TextShaper> {
    font: &'a Font,
    shape_boundaries: ShapeBoundaries<'a>,
    current_shapable: Range<usize>,
    glyphs: Vec<Font::Glyph>,
    // absolute byte offset in the entire text
    byte_offset: usize,
    glyph_index: usize,
}

impl<'a, Font: TextShaper> GraphemeCursor<'a, Font> {
    fn new(text: &'a str, font: &'a Font) -> Self {
        let mut shape_boundaries = ShapeBoundaries::new(text);

        let current_shapable = shape_boundaries.next().unwrap_or(Range { start: 0, end: 0 });

        let mut glyphs = Vec::new();
        font.shape_text(&text[current_shapable.clone()], &mut glyphs);

        Self { font, shape_boundaries, current_shapable, glyphs, byte_offset: 0, glyph_index: 0 }
    }
}

impl<'a, Font: TextShaper> Iterator for GraphemeCursor<'a, Font> {
    type Item = Grapheme<Font::Length>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.byte_offset >= self.current_shapable.end {
            self.current_shapable = match self.shape_boundaries.next() {
                Some(shapable) => shapable,
                None => return None,
            };
            self.byte_offset = self.current_shapable.start;
            self.glyph_index = 0;
            self.glyphs.clear();
            self.font.shape_text(
                &self.shape_boundaries.text[self.current_shapable.clone()],
                &mut self.glyphs,
            );
        }

        let mut grapheme_width: Font::Length = Font::Length::zero();

        let mut cluster_byte_offset;
        loop {
            let glyph = &self.glyphs[self.glyph_index];
            // Rustybuzz uses a relative byte offset as cluster index
            cluster_byte_offset = self.current_shapable.start + glyph.byte_offset();
            if cluster_byte_offset != self.byte_offset {
                break;
            }
            grapheme_width += glyph.advance_x();

            self.glyph_index += 1;

            if self.glyph_index >= self.glyphs.len() {
                cluster_byte_offset = self.current_shapable.end;
                break;
            }
        }
        let grapheme_byte_offset = self.byte_offset;
        let grapheme_byte_len = cluster_byte_offset - self.byte_offset;
        let first_char = self.shape_boundaries.text[self.byte_offset..].chars().next();
        let is_whitespace = first_char.map(|ch| ch.is_whitespace()).unwrap_or_default();
        self.byte_offset = cluster_byte_offset;

        Some(Grapheme {
            byte_range: Range {
                start: grapheme_byte_offset,
                end: grapheme_byte_offset + grapheme_byte_len,
            },
            width: grapheme_width,
            is_whitespace,
        })
    }
}

pub struct TextLineBreaker<'a, Font: TextShaper> {
    line_breaks: Box<dyn Iterator<Item = (usize, unicode_linebreak::BreakOpportunity)> + 'a>, // Would be nice to get rid of that Box...
    next_break_opportunity: Option<(usize, unicode_linebreak::BreakOpportunity)>,
    grapheme_cursor: GraphemeCursor<'a, Font>,
    available_width: Option<Font::Length>,
    current_line: TextLine<Font::Length>,
    fragment: TextLine<Font::Length>,
    num_emitted_lines: usize,
}

impl<'a, Font: TextShaper> TextLineBreaker<'a, Font> {
    fn commit_fragment(&mut self) {
        self.current_line.add_line(&mut self.fragment);
    }
    fn current_line_fits(&self) -> bool {
        self.available_width.map_or(true, |available_width| {
            self.current_line.width_including_trailing_whitespace() <= available_width
        })
    }
    fn fragment_fits(&self) -> bool {
        self.available_width.map_or(true, |available_width| {
            self.current_line.width_including_trailing_whitespace()
                + self.fragment.width_including_trailing_whitespace()
                <= available_width
        })
    }

    pub fn new(text: &'a str, font: &'a Font, available_width: Option<Font::Length>) -> Self {
        let mut line_breaks = unicode_linebreak::linebreaks(text);
        let next_break_opportunity = line_breaks.next();

        let grapheme_cursor = GraphemeCursor::new(text, font);

        Self {
            line_breaks: Box::new(line_breaks),
            next_break_opportunity,
            grapheme_cursor,
            available_width,
            current_line: Default::default(),
            fragment: Default::default(),
            num_emitted_lines: 0,
        }
    }
}

impl<'a, Font: TextShaper> Iterator for TextLineBreaker<'a, Font> {
    type Item = TextLine<Font::Length>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(grapheme) = self.grapheme_cursor.next() {
            // let ch = self.grapheme_cursor.shape_boundaries.text[grapheme.byte_range.clone()]
            //     .chars()
            //     .next();
            let mut line_to_emit = None;

            match self.next_break_opportunity.as_ref() {
                Some((offset, unicode_linebreak::BreakOpportunity::Mandatory))
                    if *offset == grapheme.byte_range.start
                        || (*offset == grapheme.byte_range.end && grapheme.is_whitespace) =>
                {
                    self.next_break_opportunity = self.line_breaks.next();

                    self.commit_fragment();
                    line_to_emit = Some(core::mem::take(&mut self.current_line));
                    self.fragment.add_grapheme(&grapheme);
                }
                Some((offset, unicode_linebreak::BreakOpportunity::Allowed))
                    if (*offset == grapheme.byte_range.start)
                        || (*offset == grapheme.byte_range.end && grapheme.is_whitespace) =>
                {
                    self.next_break_opportunity = self.line_breaks.next();

                    if self.fragment_fits() {
                        self.commit_fragment();
                    } else {
                        line_to_emit = Some(core::mem::take(&mut self.current_line));
                        self.commit_fragment();
                    }

                    self.fragment.add_grapheme(&grapheme);
                }
                _ => {
                    self.fragment.add_grapheme(&grapheme);

                    if self.current_line_fits() && !self.fragment_fits() {
                        if !self.current_line.byte_range.is_empty() {
                            line_to_emit = Some(core::mem::take(&mut self.current_line));
                        }
                        self.commit_fragment();
                    }
                }
            };

            if line_to_emit.is_some() {
                self.num_emitted_lines += 1;
                return line_to_emit;
            }
        }

        self.commit_fragment();
        if !self.current_line.byte_range.is_empty() || self.num_emitted_lines == 0 {
            self.num_emitted_lines += 1;
            return Some(core::mem::take(&mut self.current_line));
        }

        None
    }
}

// Measures the size of the given text when rendered with the specified font and optionally constrained
// by the provided `max_width`.
// Returns a tuple of the width of the longest line as well as the number of lines.
pub fn text_size<Font: TextShaper>(
    font: &Font,
    text: &str,
    max_width: Option<Font::Length>,
) -> (Font::Length, Font::LengthPrimitive)
where
    Font::Length: core::fmt::Debug,
{
    let mut max_line_width = Font::Length::zero();
    let mut line_count = Font::LengthPrimitive::zero();

    for line in TextLineBreaker::new(text, font, max_width) {
        max_line_width = euclid::approxord::max(max_line_width, line.text_width);
        line_count += Font::LengthPrimitive::one();
    }

    (max_line_width, line_count)
}

/// Layout the given string in lines, and call the `layout_line` callback with the line to draw at position y.
/// The signature of the `layout_line` function is: `(canvas, text, pos, start_index, line_metrics)`.
/// start index is the starting byte of the text in the string.
/// Returns the baseline y coordinate.
pub fn layout_text_lines<Font: TextShaper>(
    string: &str,
    font: &Font,
    font_height: Font::Length,
    max_width: Font::Length,
    max_height: Font::Length,
    (horizontal_alignment, vertical_alignment): (TextHorizontalAlignment, TextVerticalAlignment),
    wrap: TextWrap,
    overflow: TextOverflow,
    single_line: bool,
    mut layout_line: impl FnMut(
        &mut dyn Iterator<Item = (Font::Length, &'_ Font::Glyph)>,
        Font::Length,
        Font::Length,
        &TextLine<Font::Length>,
    ),
) -> Font::Length {
    let wrap = wrap == TextWrap::word_wrap;
    let elide_glyph =
        if overflow == TextOverflow::elide { font.glyph_for_char('…') } else { None };
    let max_width_without_elision =
        max_width - elide_glyph.as_ref().map_or(Font::Length::zero(), |g| g.advance_x());

    let new_line_break_iter =
        || TextLineBreaker::new(string, font, if wrap { Some(max_width) } else { None });
    let mut text_lines = None;

    let mut text_height = || {
        if single_line {
            font_height
        } else {
            text_lines = Some(new_line_break_iter().collect::<Vec<_>>());
            font_height * (text_lines.as_ref().unwrap().len() as i16).into()
        }
    };

    let two = Font::LengthPrimitive::one() + Font::LengthPrimitive::one();

    let baseline_y = match vertical_alignment {
        TextVerticalAlignment::top => Font::Length::zero(),
        TextVerticalAlignment::center => max_height / two - text_height() / two,
        TextVerticalAlignment::bottom => max_height - text_height(),
    };

    let mut y = baseline_y;

    let mut glyph_buffer = Vec::new(); // TODO: merge with line breaker's glyph buffer to avoid shaping again

    let mut process_line = |line: &TextLine<Font::Length>| {
        let x = match horizontal_alignment {
            TextHorizontalAlignment::left => Font::Length::zero(),
            TextHorizontalAlignment::center => {
                max_width / two - euclid::approxord::min(max_width, line.text_width) / two
            }
            TextHorizontalAlignment::right => {
                max_width - euclid::approxord::min(max_width, line.text_width)
            }
        };

        let mut elide_glyph = elide_glyph.as_ref().clone();

        glyph_buffer.clear();
        let text = line.line_text(string);
        font.shape_text(text, &mut glyph_buffer);

        let glyph_it = glyph_buffer.iter();
        let mut glyph_x = Font::Length::zero();
        let mut positioned_glyph_it = glyph_it.filter_map(|g| {
            if glyph_x >= max_width_without_elision {
                if let Some(elide_glyph) = elide_glyph.take() {
                    return Some((glyph_x, elide_glyph));
                } else {
                    return None;
                }
            }
            let positioned_glyph = (glyph_x, g);
            glyph_x += g.advance_x();
            Some(positioned_glyph)
        });

        layout_line(&mut positioned_glyph_it, x, y, line);
        y += font_height;
    };

    if let Some(lines_vec) = text_lines {
        for line in lines_vec {
            process_line(&line);
        }
    } else {
        for line in new_line_break_iter() {
            process_line(&line);
        }
    }

    baseline_y
}

#[test]
fn test_shape_boundaries_simple() {
    {
        let simple_text = "Hello World";
        let mut itemizer = ShapeBoundaries::new(simple_text);
        assert_eq!(itemizer.next().map(|range| &simple_text[range]), Some("Hello World"));
        assert_eq!(itemizer.next(), None);
    }
}

#[test]
fn test_shape_boundaries_empty() {
    {
        let mut itemizer = ShapeBoundaries::new("");
        assert_eq!(itemizer.next(), None);
    }
}

#[test]
fn test_shape_boundaries_script_change() {
    {
        let text = "abc🍌🐒defதோசை.";
        let mut itemizer = ShapeBoundaries::new(text);
        assert_eq!(itemizer.next().map(|range| &text[range]), Some("abc🍌🐒def"));
        assert_eq!(itemizer.next().map(|range| &text[range]), Some("தோசை."));
        assert_eq!(itemizer.next(), None);
    }
}

#[cfg(test)]
mod shape_tests {

    use super::*;

    #[derive(Clone, Debug, Default)]
    pub struct ShapedGlyph {
        pub offset_x: f32,
        pub offset_y: f32,
        pub bearing_x: f32,
        pub bearing_y: f32,
        pub width: f32,
        pub height: f32,
        pub advance_x: f32,
        pub glyph_id: Option<core::num::NonZeroU16>,
        pub glyph_cluster_index: u32,
    }

    impl GlyphMetrics<f32> for ShapedGlyph {
        fn advance_x(&self) -> f32 {
            self.advance_x
        }

        fn byte_offset(&self) -> usize {
            self.glyph_cluster_index as usize
        }
    }

    impl<'a> TextShaper for rustybuzz::Face<'a> {
        type LengthPrimitive = f32;
        type Length = f32;
        type Glyph = ShapedGlyph;
        fn shape_text<GlyphStorage: std::iter::Extend<ShapedGlyph>>(
            &self,
            text: &str,
            glyphs: &mut GlyphStorage,
        ) {
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(text);
            let glyph_buffer = rustybuzz::shape(self, &[], buffer);

            let output_glyph_generator =
                glyph_buffer.glyph_infos().iter().zip(glyph_buffer.glyph_positions().iter()).map(
                    |(info, position)| {
                        let mut out_glyph = ShapedGlyph::default();

                        out_glyph.glyph_id = core::num::NonZeroU16::new(info.glyph_id as u16);
                        out_glyph.glyph_cluster_index = info.cluster;

                        out_glyph.offset_x = position.x_offset as _;
                        out_glyph.offset_y = position.y_offset as _;
                        out_glyph.advance_x = position.x_advance as _;

                        if let Some(bounding_box) = out_glyph
                            .glyph_id
                            .and_then(|id| self.glyph_bounding_box(ttf_parser::GlyphId(id.get())))
                        {
                            out_glyph.width = bounding_box.width() as _;
                            out_glyph.height = bounding_box.height() as _;
                            out_glyph.bearing_x = bounding_box.x_min as _;
                            out_glyph.bearing_y = bounding_box.y_min as _;
                        }

                        out_glyph
                    },
                );

            // Cannot return impl Iterator, so extend argument instead
            glyphs.extend(output_glyph_generator);
        }

        fn glyph_for_char(&self, _ch: char) -> Option<Self::Glyph> {
            todo!()
        }
    }

    #[test]
    fn test_shaping() {
        use std::num::NonZeroU16;
        use TextShaper;

        let mut fontdb = fontdb::Database::new();
        let dejavu_path: std::path::PathBuf =
            [env!("CARGO_MANIFEST_DIR"), "..", "backends", "gl", "fonts", "DejaVuSans.ttf"]
                .iter()
                .collect();
        fontdb.load_font_file(dejavu_path).expect("unable to load test dejavu font");
        let font_id = fontdb.faces()[0].id;
        fontdb.with_face_data(font_id, |data, font_index| {
            let face =
                rustybuzz::Face::from_slice(data, font_index).expect("unable to parse dejavu font");

            {
                let mut shaped_glyphs = Vec::new();
                // two glyph clusters: ā́b
                face.shape_text("a\u{0304}\u{0301}b", &mut shaped_glyphs);

                assert_eq!(shaped_glyphs.len(), 3);
                assert_eq!(shaped_glyphs[0].glyph_id, NonZeroU16::new(195));
                assert_eq!(shaped_glyphs[0].glyph_cluster_index, 0);

                assert_eq!(shaped_glyphs[1].glyph_id, NonZeroU16::new(690));
                assert_eq!(shaped_glyphs[1].glyph_cluster_index, 0);

                assert_eq!(shaped_glyphs[2].glyph_id, NonZeroU16::new(69));
                assert_eq!(shaped_glyphs[2].glyph_cluster_index, 5);
            }

            {
                let mut shaped_glyphs = Vec::new();
                // two glyph clusters: ā́b
                face.shape_text("a b", &mut shaped_glyphs);

                assert_eq!(shaped_glyphs.len(), 3);
                assert_eq!(shaped_glyphs[0].glyph_id, NonZeroU16::new(68));
                assert_eq!(shaped_glyphs[0].glyph_cluster_index, 0);

                assert_eq!(shaped_glyphs[1].glyph_cluster_index, 1);

                assert_eq!(shaped_glyphs[2].glyph_id, NonZeroU16::new(69));
                assert_eq!(shaped_glyphs[2].glyph_cluster_index, 2);
            }
        });
    }
}

#[cfg(test)]
mod linebreak_tests {
    use super::shape_tests::ShapedGlyph;
    use super::*;

    // All glyphs are 10 pixels wide, break on ascii rules
    struct FixedTestFont;

    impl TextShaper for FixedTestFont {
        type LengthPrimitive = f32;
        type Length = f32;
        type Glyph = ShapedGlyph;
        fn shape_text<GlyphStorage: std::iter::Extend<ShapedGlyph>>(
            &self,
            text: &str,
            glyphs: &mut GlyphStorage,
        ) {
            for (byte_offset, _) in text.char_indices() {
                let out_glyph = ShapedGlyph {
                    offset_x: 0.,
                    offset_y: 0.,
                    bearing_x: 0.,
                    bearing_y: 0.,
                    width: 10.,
                    height: 10.,
                    advance_x: 10.,
                    glyph_id: None,
                    glyph_cluster_index: byte_offset as u32,
                };
                glyphs.extend(core::iter::once(out_glyph));
            }
        }

        fn glyph_for_char(&self, _ch: char) -> Option<Self::Glyph> {
            ShapedGlyph {
                offset_x: 0.,
                offset_y: 0.,
                bearing_x: 0.,
                bearing_y: 0.,
                width: 10.,
                height: 10.,
                advance_x: 10.,
                glyph_id: None,
                glyph_cluster_index: 0,
            }
            .into()
        }
    }

    #[test]
    fn test_empty_line_break() {
        let font = FixedTestFont;
        let text = "";
        let lines = TextLineBreaker::new(text, &font, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_text(&text), "");
    }

    #[test]
    fn test_basic_line_break() {
        let font = FixedTestFont;
        let text = "Hello World";
        let lines = TextLineBreaker::new(text, &font, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_text(&text), "Hello");
        assert_eq!(lines[1].line_text(&text), "World");
    }

    #[test]
    fn test_linebreak_trailing_space() {
        let font = FixedTestFont;
        let text = "Hello              ";
        let lines = TextLineBreaker::new(text, &font, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_text(&text), "Hello");
    }

    #[test]
    fn test_forced_break() {
        let font = FixedTestFont;
        let text = "Hello\nWorld";
        let lines = TextLineBreaker::new(text, &font, None).collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_text(&text), "Hello");
        assert_eq!(lines[1].line_text(&text), "World");
    }

    #[test]
    fn test_forced_break_multi() {
        let font = FixedTestFont;
        let text = "Hello\n\n\nWorld";
        let lines = TextLineBreaker::new(text, &font, None).collect::<Vec<_>>();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].line_text(&text), "Hello");
        assert_eq!(lines[1].line_text(&text), "");
        assert_eq!(lines[2].line_text(&text), "");
        assert_eq!(lines[3].line_text(&text), "World");
    }

    #[test]
    fn test_nbsp_break() {
        let font = FixedTestFont;
        let text = "Hello\u{00a0}World";
        let lines = TextLineBreaker::new(text, &font, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_text(&text), "Hello\u{00a0}World");
    }

    #[test]
    fn test_single_line_multi_break_opportunity() {
        let font = FixedTestFont;
        let text = "a b c";
        let lines = TextLineBreaker::new(text, &font, None).collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_text(&text), "a b c");
    }
}
