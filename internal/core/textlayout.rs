// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! module for basic text layout
//!
//! The basic algorithm for breaking text into multiple lines:
//! 1. First we determine the boundaries for text shaping. As shaping happens based on a single font and we know that different fonts cater different
//!    writing systems, we split up the text into chunks that maximize our chances of finding a font that covers all glyphs in the chunk. This way for
//!    example arabic text can be covered by a font that has excellent arabic coverage while latin text is rendered using a different font.
//!    Shaping boundaries are always also grapheme boundaries.
//! 2. Then we shape the text at shaping boundaries, to determine the metrics of glyphs and glyph clusters
//! 3. Loop over all glyph clusters as well as the line break opportunities produced by the unicode line break algorithm:
//!     Sum up the width of all glyph clusters until the next line break opportunity (encapsulated in FragmentIterator), record separately the width of
//!     trailing space within the fragment.
//!     If the width of the current line (including trailing whitespace) and the new fragment of glyph clusters (without trailing whitepace) is less or
//!         equal to the available width:
//!         Add fragment of glyph clusters to the current line
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
mod glyphclusters;
mod shaping;
use shaping::ShapeBuffer;
pub use shaping::{AbstractFont, FontMetrics, Glyph, TextShaper};

mod linebreaker;
pub use linebreaker::TextLine;

pub use linebreaker::TextLineBreaker;

pub struct TextLayout<'a, Font: AbstractFont> {
    pub font: &'a Font,
    pub letter_spacing: Option<<Font as TextShaper>::Length>,
}

impl<'a, Font: AbstractFont> TextLayout<'a, Font> {
    // Measures the size of the given text when rendered with the specified font and optionally constrained
    // by the provided `max_width`.
    // Returns a tuple of the width of the longest line as well as height of all lines.
    pub fn text_size(
        &self,
        text: &str,
        max_width: Option<Font::Length>,
    ) -> (Font::Length, Font::Length)
    where
        Font::Length: core::fmt::Debug,
    {
        let mut max_line_width = Font::Length::zero();
        let mut line_count: i16 = 0;
        let shape_buffer = ShapeBuffer::new(self, text);

        for line in TextLineBreaker::<Font>::new(text, &shape_buffer, max_width) {
            max_line_width = euclid::approxord::max(max_line_width, line.text_width);
            line_count += 1;
        }

        (max_line_width, self.font.height() * line_count.into())
    }
}

pub struct PositionedGlyph<Length> {
    pub x: Length,
    pub y: Length,
    pub advance: Length,
    pub glyph_id: core::num::NonZeroU16,
    pub text_byte_offset: usize,
}

pub struct TextParagraphLayout<'a, Font: AbstractFont> {
    pub string: &'a str,
    pub layout: TextLayout<'a, Font>,
    pub max_width: Font::Length,
    pub max_height: Font::Length,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub wrap: TextWrap,
    pub overflow: TextOverflow,
    pub single_line: bool,
}

impl<'a, Font: AbstractFont> TextParagraphLayout<'a, Font> {
    /// Layout the given string in lines, and call the `layout_line` callback with the line to draw at position y.
    /// The signature of the `layout_line` function is: `(glyph_iterator, line_x, line_y)`.
    /// Returns the baseline y coordinate as Ok, or the break value if `line_callback` returns `core::ops::ControlFlow::Break`.
    pub fn layout_lines<R>(
        &self,
        mut line_callback: impl FnMut(
            &mut dyn Iterator<Item = PositionedGlyph<Font::Length>>,
            Font::Length,
            Font::Length,
        ) -> core::ops::ControlFlow<R>,
    ) -> Result<Font::Length, R> {
        let wrap = self.wrap == TextWrap::WordWrap;
        let elide_glyph = if self.overflow == TextOverflow::Elide {
            self.layout.font.glyph_for_char('…').filter(|glyph| glyph.glyph_id.is_some())
        } else {
            None
        };
        let max_width_without_elision =
            self.max_width - elide_glyph.as_ref().map_or(Font::Length::zero(), |g| g.advance);

        let shape_buffer = ShapeBuffer::new(&self.layout, self.string);

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
                self.layout.font.height()
            } else {
                text_lines = Some(new_line_break_iter().collect::<Vec<_>>());
                self.layout.font.height() * (text_lines.as_ref().unwrap().len() as i16).into()
            }
        };

        let two = Font::LengthPrimitive::one() + Font::LengthPrimitive::one();

        let baseline_y = match self.vertical_alignment {
            TextVerticalAlignment::Top => Font::Length::zero(),
            TextVerticalAlignment::Center => self.max_height / two - text_height() / two,
            TextVerticalAlignment::Bottom => self.max_height - text_height(),
        };

        let mut y = baseline_y;

        let mut process_line = |line: &TextLine<Font::Length>, glyphs: &[Glyph<Font::Length>]| {
            let x = match self.horizontal_alignment {
                TextHorizontalAlignment::Left => Font::Length::zero(),
                TextHorizontalAlignment::Center => {
                    self.max_width / two
                        - euclid::approxord::min(self.max_width, line.text_width) / two
                }
                TextHorizontalAlignment::Right => {
                    self.max_width - euclid::approxord::min(self.max_width, line.text_width)
                }
            };

            let mut elide_glyph = elide_glyph.as_ref().clone();

            let glyph_it = glyphs[line.glyph_range.clone()].iter();
            let mut glyph_x = Font::Length::zero();
            let mut positioned_glyph_it = glyph_it.filter_map(|glyph| {
                // TODO: cut off at grapheme boundaries
                if glyph_x > max_width_without_elision {
                    if let Some(elide_glyph) = elide_glyph.take() {
                        return Some(PositionedGlyph {
                            x: glyph_x,
                            y: Font::Length::zero(),
                            advance: glyph.advance,
                            glyph_id: elide_glyph.glyph_id.unwrap(), // checked earlier when initializing elide_glyph
                            text_byte_offset: glyph.text_byte_offset,
                        });
                    } else {
                        return None;
                    }
                }
                let x = glyph_x;
                glyph_x += glyph.advance;

                glyph.glyph_id.map(|existing_glyph_id| PositionedGlyph {
                    x,
                    y: Font::Length::zero(),
                    advance: glyph.advance,
                    glyph_id: existing_glyph_id,
                    text_byte_offset: glyph.text_byte_offset,
                })
            });

            if let core::ops::ControlFlow::Break(break_val) =
                line_callback(&mut positioned_glyph_it, x, y)
            {
                return core::ops::ControlFlow::Break(break_val);
            }
            y += self.layout.font.height();

            core::ops::ControlFlow::Continue(())
        };

        if let Some(lines_vec) = text_lines.take() {
            for line in lines_vec {
                if let core::ops::ControlFlow::Break(break_val) =
                    process_line(&line, &shape_buffer.glyphs)
                {
                    return Err(break_val);
                }
            }
        } else {
            for line in new_line_break_iter() {
                if let core::ops::ControlFlow::Break(break_val) =
                    process_line(&line, &shape_buffer.glyphs)
                {
                    return Err(break_val);
                }
            }
        }

        Ok(baseline_y)
    }

    pub fn cursor_pos_for_byte_offset(&self, byte_offset: usize) -> (Font::Length, Font::Length) {
        let mut last_glyph_right_edge = Font::Length::zero();
        let mut last_line_y = Font::Length::zero();

        match self.layout_lines(|glyphs, line_x, line_y| {
            while let Some(positioned_glyph) = glyphs.next() {
                last_line_y = line_y;
                if positioned_glyph.text_byte_offset == byte_offset {
                    return core::ops::ControlFlow::Break((
                        line_x + positioned_glyph.x,
                        last_line_y,
                    ));
                }
                last_glyph_right_edge = line_x + positioned_glyph.x + positioned_glyph.advance;
            }

            core::ops::ControlFlow::Continue(())
        }) {
            Ok(_) => (last_glyph_right_edge, last_line_y),
            Err(position) => position,
        }
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
    fn shape_text<GlyphStorage: std::iter::Extend<Glyph<f32>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let glyph_iter = text.char_indices().map(|(byte_offset, char)| {
            let mut utf16_buf = [0; 2];
            let utf16_char_as_glyph_id = char.encode_utf16(&mut utf16_buf)[0];

            Glyph {
                offset_x: 0.,
                offset_y: 0.,
                glyph_id: core::num::NonZeroU16::new(utf16_char_as_glyph_id),
                advance: 10.,
                text_byte_offset: byte_offset,
            }
        });
        glyphs.extend(glyph_iter);
    }

    fn glyph_for_char(&self, ch: char) -> Option<Glyph<f32>> {
        let mut utf16_buf = [0; 2];
        let utf16_char_as_glyph_id = ch.encode_utf16(&mut utf16_buf)[0];

        Glyph {
            offset_x: 0.,
            offset_y: 0.,
            glyph_id: core::num::NonZeroU16::new(utf16_char_as_glyph_id),
            advance: 10.,
            text_byte_offset: 0,
        }
        .into()
    }
}

#[cfg(test)]
impl FontMetrics<f32> for FixedTestFont {
    fn ascent(&self) -> f32 {
        5.
    }

    fn descent(&self) -> f32 {
        -5.
    }
}

#[test]
fn test_elision() {
    let font = FixedTestFont;
    let text = "This is a longer piece of text";

    let mut lines = Vec::new();

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 13. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::NoWrap,
        overflow: TextOverflow::Elide,
        single_line: true,
    };
    paragraph
        .layout_lines::<()>(|glyphs, _, _| {
            lines.push(
                glyphs
                    .map(|positioned_glyph| positioned_glyph.glyph_id.clone())
                    .collect::<Vec<_>>(),
            );
            core::ops::ControlFlow::Continue(())
        })
        .unwrap();

    assert_eq!(lines.len(), 1);
    let rendered_text = lines[0]
        .iter()
        .flat_map(|glyph_id| {
            core::char::decode_utf16(core::iter::once(glyph_id.get()))
                .map(|r| r.unwrap())
                .collect::<Vec<char>>()
        })
        .collect::<String>();
    debug_assert_eq!(rendered_text, "This is a lon…")
}

#[test]
fn test_exact_fit() {
    let font = FixedTestFont;
    let text = "Fits";

    let mut lines = Vec::new();

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 4. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::NoWrap,
        overflow: TextOverflow::Elide,
        single_line: true,
    };
    paragraph
        .layout_lines::<()>(|glyphs, _, _| {
            lines.push(
                glyphs
                    .map(|positioned_glyph| positioned_glyph.glyph_id.clone())
                    .collect::<Vec<_>>(),
            );
            core::ops::ControlFlow::Continue(())
        })
        .unwrap();

    assert_eq!(lines.len(), 1);
    let rendered_text = lines[0]
        .iter()
        .flat_map(|glyph_id| {
            core::char::decode_utf16(core::iter::once(glyph_id.get()))
                .map(|r| r.unwrap())
                .collect::<Vec<char>>()
        })
        .collect::<String>();
    debug_assert_eq!(rendered_text, "Fits")
}

#[test]
fn test_no_line_separators_characters_rendered() {
    let font = FixedTestFont;
    let text = "Hello\nWorld\n";

    let mut lines = Vec::new();

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 13. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::NoWrap,
        overflow: TextOverflow::Clip,
        single_line: true,
    };
    paragraph
        .layout_lines::<()>(|glyphs, _, _| {
            lines.push(
                glyphs
                    .map(|positioned_glyph| positioned_glyph.glyph_id.clone())
                    .collect::<Vec<_>>(),
            );
            core::ops::ControlFlow::Continue(())
        })
        .unwrap();

    assert_eq!(lines.len(), 2);
    let rendered_text = lines
        .iter()
        .map(|glyphs_per_line| {
            glyphs_per_line
                .iter()
                .flat_map(|glyph_id| {
                    core::char::decode_utf16(core::iter::once(glyph_id.get()))
                        .map(|r| r.unwrap())
                        .collect::<Vec<char>>()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    debug_assert_eq!(rendered_text, vec!["Hello", "World"]);
}

#[test]
fn test_cursor_position() {
    let font = FixedTestFont;
    let text = "Hello\nWorld";

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 100. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::NoWrap,
        overflow: TextOverflow::Clip,
        single_line: false,
    };

    assert_eq!(paragraph.cursor_pos_for_byte_offset(0), (0., 0.));

    let e_offset = text
        .char_indices()
        .find_map(|(offset, ch)| if ch == 'e' { Some(offset) } else { None })
        .unwrap();
    assert_eq!(paragraph.cursor_pos_for_byte_offset(e_offset), (10., 0.));

    let w_offset = text
        .char_indices()
        .find_map(|(offset, ch)| if ch == 'W' { Some(offset) } else { None })
        .unwrap();
    assert_eq!(paragraph.cursor_pos_for_byte_offset(w_offset), (0., 10.));

    assert_eq!(paragraph.cursor_pos_for_byte_offset(text.len()), (10. * 5., 10.));
}
