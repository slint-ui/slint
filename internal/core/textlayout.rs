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
//!         // Fall back to breaking anywhere if we can't find any other break point
//!         If the current fragment ends before the first break opportunity and width of current line + fragment <= available width:
//!              Add fragment to current line
//!              Clear fragment
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

use core::cell::RefCell;
use core::ops::Range;

use alloc::{rc::Rc, vec::Vec};

use euclid::num::{One, Zero};

use crate::items::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap};

#[cfg(feature = "unicode-linebreak")]
mod linebreak_unicode;
#[cfg(feature = "unicode-linebreak")]
use linebreak_unicode::{BreakOpportunity, LineBreakIterator};

#[cfg(not(feature = "unicode-linebreak"))]
mod linebreak_simple;
#[cfg(not(feature = "unicode-linebreak"))]
use linebreak_simple::{BreakOpportunity, LineBreakIterator};

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
    type Glyph;
    // Shapes the given string and emits the result into the given glyphs buffer,
    // as tuples of glyph handle and corresponding byte offset in the originating string.
    fn shape_text<GlyphStorage: core::iter::Extend<(Self::Glyph, usize)>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    );
    fn glyph_for_char(&self, ch: char) -> Option<Self::Glyph>;
    fn glyph_advance_x(&self, glyph: &Self::Glyph) -> Self::Length;
}

pub struct ShapeBoundaries<'a> {
    text: &'a str,
    #[cfg(feature = "unicode-script")]
    // TODO: We should do a better analysis to find boundaries for text shaping; including
    // boundaries when the bidi level changes or an explicit separator like
    // paragraph/lineseparator/space is encountered.
    chars: core::str::CharIndices<'a>,
    next_boundary_start: Option<usize>,
    #[cfg(feature = "unicode-script")]
    last_script: Option<unicode_script::Script>,
}

impl<'a> ShapeBoundaries<'a> {
    pub fn new(text: &'a str) -> Self {
        let next_boundary_start = if !text.is_empty() { Some(0) } else { None };
        Self {
            text,
            #[cfg(feature = "unicode-script")]
            chars: text.char_indices(),
            next_boundary_start,
            #[cfg(feature = "unicode-script")]
            last_script: None,
        }
    }
}

impl<'a> Iterator for ShapeBoundaries<'a> {
    type Item = usize;

    #[cfg(feature = "unicode-script")]
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_boundary_start.is_none() {
            return None;
        }

        let (next_offset, script) = loop {
            match self.chars.next() {
                Some((byte_offset, ch)) => {
                    use unicode_script::UnicodeScript;
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

        self.last_script = script;
        self.next_boundary_start = next_offset;

        Some(self.next_boundary_start.unwrap_or(self.text.len()))
    }

    #[cfg(not(feature = "unicode-script"))]
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_boundary_start {
            Some(_) => {
                self.next_boundary_start = None;
                Some(self.text.len())
            }
            None => None,
        }
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
    glyph_range: Range<usize>,
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
    glyph_range: Range<usize>,
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
                self.glyph_range.start = grapheme.glyph_range.start;
                self.glyph_range.end = self.glyph_range.start;
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
                self.glyph_range.end = grapheme.glyph_range.end;

                self.text_width += grapheme.width;
            }
            (None, false) => {
                if !self.byte_range.is_empty() {
                    // There should not be any gaps between the whitespace and the added grapheme
                    debug_assert_eq!(self.byte_range.end, grapheme.byte_range.start);
                }
                self.byte_range.end += grapheme.byte_range.len();
                self.glyph_range.end = grapheme.glyph_range.end;

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
            self.glyph_range.start = candidate.glyph_range.start;
            self.glyph_range.end = candidate.glyph_range.end;
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
        self.glyph_range.end = candidate.glyph_range.end;

        self.text_width += candidate.text_width;
        *candidate = Default::default();
    }
}

struct TextRun {
    byte_range: Range<usize>,
    //glyph_range: Range<usize>,
    // TODO: direction, etc.
}

struct GraphemeCursor<
    'a,
    Font: TextShaper,
    GlyphBuffer: core::iter::Extend<(Font::Glyph, usize)> + core::convert::AsRef<[(Font::Glyph, usize)]>,
> {
    text: &'a str,
    font: &'a Font,
    glyphs: &'a RefCell<GlyphBuffer>,
    runs: Rc<Vec<TextRun>>, // TODO: let caller provide buffer for these
    current_run: usize,
    // absolute byte offset in the entire text
    byte_offset: usize,
    glyph_index: usize,
}

impl<
        'a,
        Font: TextShaper,
        GlyphBuffer: core::iter::Extend<(Font::Glyph, usize)> + core::convert::AsRef<[(Font::Glyph, usize)]>,
    > GraphemeCursor<'a, Font, GlyphBuffer>
{
    fn new(text: &'a str, font: &'a Font, glyph_buffer: &'a RefCell<GlyphBuffer>) -> Self {
        let first_glyph_index = glyph_buffer.borrow().as_ref().len();

        let runs = Rc::new(
            ShapeBoundaries::new(text)
                .scan(0, |run_start, run_end| {
                    //let glyphs_start = glyph_buffer.borrow().as_ref().len();

                    font.shape_text(&text[*run_start..run_end], &mut *glyph_buffer.borrow_mut());

                    let run = TextRun {
                        byte_range: Range { start: *run_start, end: run_end },
                        //glyph_range: Range {
                        //     start: glyphs_start,
                        //     end: glyph_buffer.borrow().as_ref().len(),
                        // },
                    };
                    *run_start = run_end;

                    Some(run)
                })
                .collect(),
        );

        Self {
            text,
            font,
            glyphs: glyph_buffer,
            runs,
            current_run: 0,
            byte_offset: 0,
            glyph_index: first_glyph_index,
        }
    }
}

impl<
        'a,
        Font: TextShaper,
        GlyphBuffer: core::iter::Extend<(Font::Glyph, usize)> + core::convert::AsRef<[(Font::Glyph, usize)]>,
    > Iterator for GraphemeCursor<'a, Font, GlyphBuffer>
{
    type Item = Grapheme<Font::Length>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_run >= self.runs.len() {
            return None;
        }

        let current_run = if self.byte_offset < self.runs[self.current_run].byte_range.end {
            &self.runs[self.current_run]
        } else {
            self.current_run += 1;
            self.runs.get(self.current_run)?
        };

        let mut grapheme_width: Font::Length = Font::Length::zero();
        let glyphs = self.glyphs.borrow();

        let grapheme_glyph_start = self.glyph_index;

        let mut cluster_byte_offset;
        loop {
            let (glyph, glyph_byte_offset) = &glyphs.as_ref()[self.glyph_index];
            // Rustybuzz uses a relative byte offset as cluster index
            cluster_byte_offset = current_run.byte_range.start + glyph_byte_offset;
            if cluster_byte_offset != self.byte_offset {
                break;
            }
            grapheme_width += self.font.glyph_advance_x(glyph);

            self.glyph_index += 1;

            if self.glyph_index >= self.glyphs.borrow().as_ref().len() {
                cluster_byte_offset = current_run.byte_range.end;
                break;
            }
        }
        let grapheme_byte_offset = self.byte_offset;
        let grapheme_byte_len = cluster_byte_offset - self.byte_offset;
        let is_whitespace = self.text[self.byte_offset..]
            .chars()
            .next()
            .map(|ch| ch.is_whitespace())
            .unwrap_or_default();
        self.byte_offset = cluster_byte_offset;

        Some(Grapheme {
            byte_range: Range {
                start: grapheme_byte_offset,
                end: grapheme_byte_offset + grapheme_byte_len,
            },
            glyph_range: Range { start: grapheme_glyph_start, end: self.glyph_index },
            width: grapheme_width,
            is_whitespace,
        })
    }
}

pub struct TextLineBreaker<
    'a,
    Font: TextShaper,
    GlyphBuffer: core::iter::Extend<(Font::Glyph, usize)> + core::convert::AsRef<[(Font::Glyph, usize)]>,
> {
    line_breaks: LineBreakIterator<'a>,
    first_break_opportunity: usize,
    next_break_opportunity: Option<(usize, BreakOpportunity)>,
    grapheme_cursor: GraphemeCursor<'a, Font, GlyphBuffer>,
    available_width: Option<Font::Length>,
    current_line: TextLine<Font::Length>,
    fragment: TextLine<Font::Length>,
    num_emitted_lines: usize,
}

impl<
        'a,
        Font: TextShaper,
        GlyphBuffer: core::iter::Extend<(Font::Glyph, usize)> + core::convert::AsRef<[(Font::Glyph, usize)]>,
    > TextLineBreaker<'a, Font, GlyphBuffer>
{
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

    pub fn new(
        text: &'a str,
        font: &'a Font,
        glyph_buffer: &'a RefCell<GlyphBuffer>,
        available_width: Option<Font::Length>,
    ) -> Self {
        let mut line_breaks = LineBreakIterator::new(text);
        let next_break_opportunity = line_breaks.next();

        let grapheme_cursor = GraphemeCursor::new(text, font, glyph_buffer);

        Self {
            line_breaks,
            first_break_opportunity: next_break_opportunity
                .map_or(text.len(), |(offset, _)| offset),
            next_break_opportunity,
            grapheme_cursor,
            available_width,
            current_line: Default::default(),
            fragment: Default::default(),
            num_emitted_lines: 0,
        }
    }
}

impl<
        'a,
        Font: TextShaper,
        GlyphBuffer: core::iter::Extend<(Font::Glyph, usize)> + core::convert::AsRef<[(Font::Glyph, usize)]>,
    > Iterator for TextLineBreaker<'a, Font, GlyphBuffer>
{
    type Item = TextLine<Font::Length>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(grapheme) = self.grapheme_cursor.next() {
            // let ch = self.grapheme_cursor.shape_boundaries.text[grapheme.byte_range.clone()]
            //     .chars()
            //     .next();
            let mut line_to_emit = None;

            match self.next_break_opportunity.as_ref() {
                Some((offset, BreakOpportunity::Mandatory))
                    if *offset == grapheme.byte_range.start
                        || (*offset == grapheme.byte_range.end && grapheme.is_whitespace) =>
                {
                    self.next_break_opportunity = self.line_breaks.next();

                    self.commit_fragment();
                    line_to_emit = Some(core::mem::take(&mut self.current_line));
                    self.fragment.add_grapheme(&grapheme);
                }
                Some((offset, BreakOpportunity::Allowed))
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

                    if self.fragment.byte_range.end <= self.first_break_opportunity
                        && self.fragment_fits()
                    {
                        self.commit_fragment();
                    }

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
    let glyphs = RefCell::new(Vec::new());

    for line in TextLineBreaker::new(text, font, &glyphs, max_width) {
        max_line_width = euclid::approxord::max(max_line_width, line.text_width);
        line_count += Font::LengthPrimitive::one();
    }

    (max_line_width, line_count)
}

pub struct TextParagraphLayout<'a, Font: TextShaper> {
    pub string: &'a str,
    pub font: &'a Font,
    pub font_height: Font::Length,
    pub max_width: Font::Length,
    pub max_height: Font::Length,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub wrap: TextWrap,
    pub overflow: TextOverflow,
    pub single_line: bool,
}

impl<'a, Font: TextShaper> TextParagraphLayout<'a, Font> {
    /// Layout the given string in lines, and call the `layout_line` callback with the line to draw at position y.
    /// The signature of the `layout_line` function is: `(glyph_iterator, line_x, line_y)`.
    /// Returns the baseline y coordinate.
    pub fn layout_lines(
        &self,
        mut line_callback: impl FnMut(
            &mut dyn Iterator<Item = (Font::Length, &'_ Font::Glyph)>,
            Font::Length,
            Font::Length,
        ),
    ) -> Font::Length {
        let wrap = self.wrap == TextWrap::word_wrap;
        let elide_glyph = if self.overflow == TextOverflow::elide {
            self.font.glyph_for_char('…')
        } else {
            None
        };
        let max_width_without_elision = self.max_width
            - elide_glyph.as_ref().map_or(Font::Length::zero(), |g| self.font.glyph_advance_x(g));

        let glyphs = RefCell::new(Vec::new());

        let new_line_break_iter = |glyphs| {
            TextLineBreaker::new(
                self.string,
                self.font,
                glyphs,
                if wrap { Some(self.max_width) } else { None },
            )
        };
        let mut text_lines = None;

        let mut text_height = |glyphs| {
            if self.single_line {
                self.font_height
            } else {
                text_lines = Some(new_line_break_iter(glyphs).collect::<Vec<_>>());
                self.font_height * (text_lines.as_ref().unwrap().len() as i16).into()
            }
        };

        let two = Font::LengthPrimitive::one() + Font::LengthPrimitive::one();

        let baseline_y = match self.vertical_alignment {
            TextVerticalAlignment::top => Font::Length::zero(),
            TextVerticalAlignment::center => self.max_height / two - text_height(&glyphs) / two,
            TextVerticalAlignment::bottom => self.max_height - text_height(&glyphs),
        };

        let mut y = baseline_y;

        let mut process_line =
            |line: &TextLine<Font::Length>, glyphs: &RefCell<Vec<(Font::Glyph, usize)>>| {
                let x = match self.horizontal_alignment {
                    TextHorizontalAlignment::left => Font::Length::zero(),
                    TextHorizontalAlignment::center => {
                        self.max_width / two
                            - euclid::approxord::min(self.max_width, line.text_width) / two
                    }
                    TextHorizontalAlignment::right => {
                        self.max_width - euclid::approxord::min(self.max_width, line.text_width)
                    }
                };

                let mut elide_glyph = elide_glyph.as_ref().clone();

                let glyphs = glyphs.borrow();
                let glyph_it = glyphs[line.glyph_range.clone()].iter();
                let mut glyph_x = Font::Length::zero();
                let mut positioned_glyph_it = glyph_it.filter_map(|(glyph, _)| {
                    // TODO: cut off at grapheme boundaries
                    if glyph_x > max_width_without_elision {
                        if let Some(elide_glyph) = elide_glyph.take() {
                            return Some((glyph_x, elide_glyph));
                        } else {
                            return None;
                        }
                    }
                    let positioned_glyph = (glyph_x, glyph);
                    glyph_x += self.font.glyph_advance_x(glyph);
                    Some(positioned_glyph)
                });

                line_callback(&mut positioned_glyph_it, x, y);
                y += self.font_height;
            };

        if let Some(lines_vec) = text_lines.take() {
            for line in lines_vec {
                process_line(&line, &glyphs);
            }
        } else {
            for line in new_line_break_iter(&glyphs) {
                process_line(&line, &glyphs);
            }
        }

        baseline_y
    }
}

#[test]
fn test_no_linebreak_opportunity_at_eot() {
    let mut it = LineBreakIterator::new("Hello World");
    assert_eq!(it.next(), Some((6, BreakOpportunity::Allowed)));
    assert_eq!(it.next(), None);
}

#[test]
fn test_shape_boundaries_simple() {
    {
        let simple_text = "Hello World";
        let mut itemizer = ShapeBoundaries::new(simple_text);
        assert_eq!(itemizer.next(), Some(simple_text.len()));
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
        let mut itemizer = ShapeBoundaries::new(text).scan(0, |start, end| {
            let str = &text[*start..end];
            *start = end;
            Some(str)
        });
        assert_eq!(itemizer.next(), Some("abc🍌🐒def"));
        assert_eq!(itemizer.next(), Some("தோசை."));
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
        pub char: Option<char>,
    }

    impl<'a> TextShaper for rustybuzz::Face<'a> {
        type LengthPrimitive = f32;
        type Length = f32;
        type Glyph = ShapedGlyph;
        fn shape_text<GlyphStorage: std::iter::Extend<(ShapedGlyph, usize)>>(
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

                        out_glyph.offset_x = position.x_offset as _;
                        out_glyph.offset_y = position.y_offset as _;
                        out_glyph.advance_x = position.x_advance as _;
                        out_glyph.char = text[info.cluster as usize..].chars().next();

                        if let Some(bounding_box) = out_glyph
                            .glyph_id
                            .and_then(|id| self.glyph_bounding_box(ttf_parser::GlyphId(id.get())))
                        {
                            out_glyph.width = bounding_box.width() as _;
                            out_glyph.height = bounding_box.height() as _;
                            out_glyph.bearing_x = bounding_box.x_min as _;
                            out_glyph.bearing_y = bounding_box.y_min as _;
                        }

                        (out_glyph, info.cluster as usize)
                    },
                );

            // Cannot return impl Iterator, so extend argument instead
            glyphs.extend(output_glyph_generator);
        }

        fn glyph_for_char(&self, _ch: char) -> Option<Self::Glyph> {
            todo!()
        }

        fn glyph_advance_x(&self, glyph: &Self::Glyph) -> Self::Length {
            glyph.advance_x
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
                assert_eq!(shaped_glyphs[0].0.glyph_id, NonZeroU16::new(195));
                assert_eq!(shaped_glyphs[0].1, 0);

                assert_eq!(shaped_glyphs[1].0.glyph_id, NonZeroU16::new(690));
                assert_eq!(shaped_glyphs[1].1, 0);

                assert_eq!(shaped_glyphs[2].0.glyph_id, NonZeroU16::new(69));
                assert_eq!(shaped_glyphs[2].1, 5);
            }

            {
                let mut shaped_glyphs = Vec::new();
                // two glyph clusters: ā́b
                face.shape_text("a b", &mut shaped_glyphs);

                assert_eq!(shaped_glyphs.len(), 3);
                assert_eq!(shaped_glyphs[0].0.glyph_id, NonZeroU16::new(68));
                assert_eq!(shaped_glyphs[0].1, 0);

                assert_eq!(shaped_glyphs[1].1, 1);

                assert_eq!(shaped_glyphs[2].0.glyph_id, NonZeroU16::new(69));
                assert_eq!(shaped_glyphs[2].1, 2);
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
        fn shape_text<GlyphStorage: std::iter::Extend<(ShapedGlyph, usize)>>(
            &self,
            text: &str,
            glyphs: &mut GlyphStorage,
        ) {
            let glyph_iter = text.char_indices().map(|(byte_offset, char)| {
                (
                    ShapedGlyph {
                        offset_x: 0.,
                        offset_y: 0.,
                        bearing_x: 0.,
                        bearing_y: 0.,
                        width: 10.,
                        height: 10.,
                        advance_x: 10.,
                        glyph_id: None,
                        char: Some(char),
                    },
                    byte_offset,
                )
            });
            glyphs.extend(glyph_iter);
        }

        fn glyph_for_char(&self, ch: char) -> Option<Self::Glyph> {
            ShapedGlyph {
                offset_x: 0.,
                offset_y: 0.,
                bearing_x: 0.,
                bearing_y: 0.,
                width: 10.,
                height: 10.,
                advance_x: 10.,
                glyph_id: None,
                char: Some(ch),
            }
            .into()
        }

        fn glyph_advance_x(&self, glyph: &Self::Glyph) -> Self::Length {
            glyph.advance_x
        }
    }

    #[test]
    fn test_empty_line_break() {
        let font = FixedTestFont;
        let text = "";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_text(&text), "");
    }

    #[test]
    fn test_basic_line_break() {
        let font = FixedTestFont;
        let text = "Hello World";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_text(&text), "Hello");
        assert_eq!(lines[1].line_text(&text), "World");
    }

    #[test]
    fn test_linebreak_trailing_space() {
        let font = FixedTestFont;
        let text = "Hello              ";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_text(&text), "Hello");
    }

    #[test]
    fn test_forced_break() {
        let font = FixedTestFont;
        let text = "Hello\nWorld";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, None).collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_text(&text), "Hello");
        assert_eq!(lines[1].line_text(&text), "World");
    }

    #[test]
    fn test_forced_break_multi() {
        let font = FixedTestFont;
        let text = "Hello\n\n\nWorld";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, None).collect::<Vec<_>>();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].line_text(&text), "Hello");
        assert_eq!(lines[1].line_text(&text), "");
        assert_eq!(lines[2].line_text(&text), "");
        assert_eq!(lines[3].line_text(&text), "World");
    }

    #[test]
    fn test_nbsp_break() {
        let font = FixedTestFont;
        let text = "Ok Hello\u{00a0}World";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, Some(110.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_text(&text), "Ok");
        assert_eq!(lines[1].line_text(&text), "Hello\u{00a0}World");
    }

    #[test]
    fn test_single_line_multi_break_opportunity() {
        let font = FixedTestFont;
        let text = "a b c";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, None).collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_text(&text), "a b c");
    }

    #[test]
    fn test_basic_line_break_anywhere_fallback() {
        let font = FixedTestFont;
        let text = "HelloWorld";
        let mut glyphs = RefCell::new(Vec::new());
        let lines = TextLineBreaker::new(text, &font, &mut glyphs, Some(50.)).collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_text(&text), "Hello");
        assert_eq!(lines[1].line_text(&text), "World");
    }

    #[test]
    fn test_elision() {
        let font = FixedTestFont;
        let text = "This is a longer piece of text";

        let mut lines = Vec::new();

        let paragraph = TextParagraphLayout {
            string: text,
            font: &font,
            font_height: 10.,
            max_width: 13. * 10.,
            max_height: 10.,
            horizontal_alignment: TextHorizontalAlignment::left,
            vertical_alignment: TextVerticalAlignment::top,
            wrap: TextWrap::no_wrap,
            overflow: TextOverflow::elide,
            single_line: true,
        };
        paragraph.layout_lines(|glyphs, _, _| {
            lines.push(glyphs.map(|(_, g)| g.clone()).collect::<Vec<_>>());
        });

        assert_eq!(lines.len(), 1);
        let rendered_text = lines[0].iter().map(|glyph| glyph.char.unwrap()).collect::<String>();
        debug_assert_eq!(rendered_text, "This is a lon…")
    }

    #[test]
    fn test_exact_fit() {
        let font = FixedTestFont;
        let text = "Fits";

        let mut lines = Vec::new();

        let paragraph = TextParagraphLayout {
            string: text,
            font: &font,
            font_height: 10.,
            max_width: 4. * 10.,
            max_height: 10.,
            horizontal_alignment: TextHorizontalAlignment::left,
            vertical_alignment: TextVerticalAlignment::top,
            wrap: TextWrap::no_wrap,
            overflow: TextOverflow::elide,
            single_line: true,
        };
        paragraph.layout_lines(|glyphs, _, _| {
            lines.push(glyphs.map(|(_, g)| g.clone()).collect::<Vec<_>>());
        });

        assert_eq!(lines.len(), 1);
        let rendered_text = lines[0].iter().map(|glyph| glyph.char.unwrap()).collect::<String>();
        debug_assert_eq!(rendered_text, "Fits")
    }
}
