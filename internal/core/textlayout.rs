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
//! 3. Loop over all graphemes as well as the line break opportunities produced by the unicode line break algorithm:
//!     Sum up the width of all graphemes until the next line break opportunity (encapsulated in FragmentIterator), record separately the width of
//!     trailing space within the fragment.
//!     If the width of the current line (including trailing whitespace) and the new fragment of graphemes (without trailing whitepace) is less or
//!         equal to the available width:
//!         Add fragment of graphenes to the current line
//!     Else:
//!         Emit current line as new line
//!     If encountering a mandatory line break opportunity:
//!         Emit current line as new line
//!

use alloc::vec::Vec;

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

mod fragments;
mod graphemes;
mod shaping;
use shaping::ShapeBuffer;
pub use shaping::{GlyphMetrics, TextShaper};

mod linebreaker;
pub use linebreaker::TextLine;

pub use linebreaker::TextLineBreaker;

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
    let shape_buffer = ShapeBuffer::new(font, text);

    for line in TextLineBreaker::<Font>::new(text, &shape_buffer, max_width) {
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

        let shape_buffer = ShapeBuffer::new(self.font, self.string);

        let new_line_break_iter = || {
            TextLineBreaker::<Font>::new(
                self.string,
                &shape_buffer,
                if wrap { Some(self.max_width) } else { None },
            )
        };
        let mut text_lines = None;

        let mut text_height = || {
            if self.single_line {
                self.font_height
            } else {
                text_lines = Some(new_line_break_iter().collect::<Vec<_>>());
                self.font_height * (text_lines.as_ref().unwrap().len() as i16).into()
            }
        };

        let two = Font::LengthPrimitive::one() + Font::LengthPrimitive::one();

        let baseline_y = match self.vertical_alignment {
            TextVerticalAlignment::top => Font::Length::zero(),
            TextVerticalAlignment::center => self.max_height / two - text_height() / two,
            TextVerticalAlignment::bottom => self.max_height - text_height(),
        };

        let mut y = baseline_y;

        let mut process_line = |line: &TextLine<Font::Length>, glyphs: &[(Font::Glyph, usize)]| {
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
                process_line(&line, &shape_buffer.glyphs);
            }
        } else {
            for line in new_line_break_iter() {
                process_line(&line, &shape_buffer.glyphs);
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

// All glyphs are 10 pixels wide, break on ascii rules
#[cfg(test)]
pub struct FixedTestFont;

#[cfg(test)]
impl TextShaper for FixedTestFont {
    type LengthPrimitive = f32;
    type Length = f32;
    type Glyph = shaping::ShapedGlyph;
    fn shape_text<GlyphStorage: std::iter::Extend<(shaping::ShapedGlyph, usize)>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let glyph_iter = text.char_indices().map(|(byte_offset, char)| {
            (
                shaping::ShapedGlyph {
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
        shaping::ShapedGlyph {
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
