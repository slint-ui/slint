// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore sharedparley
//! module for basic text layout
//!
//! The basic algorithm for breaking text into multiple lines:
//! 1. First we determine the boundaries for text shaping. As shaping happens based on a single font and we know that different fonts cater different
//!    writing systems, we split up the text into chunks that maximize our chances of finding a font that covers all glyphs in the chunk. This way for
//!    example arabic text can be covered by a font that has excellent arabic coverage while latin text is rendered using a different font.
//!    Shaping boundaries are always also grapheme boundaries.
//! 2. Then we shape the text at shaping boundaries, to determine the metrics of glyphs and glyph clusters
//! 3. Loop over all glyph clusters as well as the line break opportunities produced by the unicode line break algorithm:
//!    Sum up the width of all glyph clusters until the next line break opportunity (encapsulated in FragmentIterator), record separately the width of
//!    trailing space within the fragment.
//!    ```text
//!    If the width of the current line (including trailing whitespace) and the new fragment of glyph clusters (without trailing whitespace) is less or
//!    equal to the available width:
//!        Add fragment of glyph clusters to the current line
//!    Else:
//!        Emit current line as new line
//!    If encountering a mandatory line break opportunity:
//!        Emit current line as new line
//!    ```

use alloc::vec::Vec;

use euclid::num::{One, Zero};

use crate::items::{TextHorizontalAlignment, TextOverflow, TextVerticalAlignment, TextWrap};

/// The font size to lay text out with when neither the `.slint` code nor the platform
/// provide one. Last level of the precedence chain in
/// [`crate::items::WindowItem::resolved_default_font_size`].
pub const DEFAULT_FONT_SIZE: crate::lengths::LogicalLength =
    crate::lengths::LogicalLength::new(12 as crate::Coord);

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
#[cfg(feature = "shared-parley")]
/// cbindgen:ignore
pub mod sharedparley;
use shaping::ShapeBuffer;
pub use shaping::{AbstractFont, FontMetrics, Glyph, TextShaper};

mod linebreaker;
pub use linebreaker::TextLine;

pub use linebreaker::TextLineBreaker;

pub struct TextLayout<'a, Font: AbstractFont> {
    pub font: &'a Font,
    pub letter_spacing: Option<<Font as TextShaper>::Length>,
}

impl<Font: AbstractFont> TextLayout<'_, Font> {
    // Measures the size of the given text when rendered with the specified font and optionally constrained
    // by the provided `max_width`.
    // Returns a tuple of the width of the longest line as well as height of all lines.
    pub fn text_size(
        &self,
        text: &str,
        max_width: Option<Font::Length>,
        text_wrap: TextWrap,
        max_lines: Option<usize>,
    ) -> (Font::Length, Font::Length)
    where
        Font::Length: core::fmt::Debug,
    {
        let mut max_line_width = Font::Length::zero();
        let mut line_count: i16 = 0;
        let shape_buffer = ShapeBuffer::new(self, text);

        for line in
            TextLineBreaker::<Font>::new(text, &shape_buffer, max_width, max_lines, text_wrap)
        {
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
    pub max_lines: Option<usize>,
}

impl<Font: AbstractFont> TextParagraphLayout<'_, Font> {
    /// Layout the given string in lines, and call the `layout_line` callback with the line to draw at position y.
    /// The signature of the `layout_line` function is: `(glyph_iterator, line_x, line_y, text_line, selection)`.
    /// Returns the baseline y coordinate as Ok, or the break value if `line_callback` returns `core::ops::ControlFlow::Break`.
    pub fn layout_lines<R>(
        &self,
        mut line_callback: impl FnMut(
            &mut dyn Iterator<Item = PositionedGlyph<Font::Length>>,
            Font::Length,
            Font::Length,
            &TextLine<Font::Length>,
            Option<core::ops::Range<Font::Length>>,
        ) -> core::ops::ControlFlow<R>,
        selection: Option<core::ops::Range<usize>>,
    ) -> Result<Font::Length, R> {
        let wrap = self.wrap != TextWrap::NoWrap;
        let elide = self.overflow == TextOverflow::Elide;
        let elide_glyph = if elide {
            self.layout.font.glyph_for_char('…').filter(|glyph| glyph.glyph_id.is_some())
        } else {
            None
        };
        let elide_width = elide_glyph.as_ref().map_or(Font::Length::zero(), |g| g.advance);
        let max_width_without_elision = self.max_width - elide_width;

        let shape_buffer = ShapeBuffer::new(&self.layout, self.string);

        // When eliding, always keep at least the first line: when it is taller than the box,
        // dropping it would render nothing at all, which is more confusing than a clipped line.
        // The software renderer already clips glyphs to the Text geometry, so the vertical
        // overflow is trimmed; horizontal elision still places an ellipsis if it is too
        // wide. Mirrors the parley path, which always keeps line index 0.
        let max_lines_from_height =
            elide.then(|| self.layout.font.max_lines(self.max_height).max(1));
        let max_lines = [self.max_lines, max_lines_from_height].into_iter().flatten().min();

        let new_line_break_iter = || {
            TextLineBreaker::<Font>::new(
                self.string,
                &shape_buffer,
                if wrap { Some(self.max_width) } else { None },
                max_lines,
                self.wrap,
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
        let mut line_index = 0usize;

        let mut process_line = |line: &TextLine<Font::Length>, glyphs: &[Glyph<Font::Length>]| {
            let elide_long_line =
                elide && (self.single_line || !wrap) && line.text_width > self.max_width;
            // The last line before the line limit carries the ellipsis just like the last line
            // that fits the height, so `max-lines` truncation is signalled the same way.
            let reached_line_limit = max_lines.is_some_and(|max_lines| line_index + 1 == max_lines);
            let elide_last_line = elide
                && line.glyph_range.end < glyphs.len()
                && (y + self.layout.font.height() * two > self.max_height || reached_line_limit);

            // On a vertically truncated line the ellipsis is anchored right after the text by
            // ignoring trailing whitespace, so it reads "please…" rather than "please   …". The
            // line breaker already excludes trailing whitespace from `byte_range`, so the glyphs
            // past its end are exactly that whitespace to trim. This trimmed range is what gets
            // measured (for alignment) and drawn; other lines keep the full range.
            let glyph_range = if elide_last_line {
                let trailing_whitespace_glyphs = glyphs[line.glyph_range.clone()]
                    .iter()
                    .rev()
                    .take_while(|glyph| glyph.text_byte_offset >= line.byte_range.end)
                    .count();
                line.glyph_range.start..line.glyph_range.end - trailing_whitespace_glyphs
            } else {
                line.glyph_range.clone()
            };

            let text_width = || {
                if elide_long_line || elide_last_line {
                    let mut text_width = Font::Length::zero();
                    for glyph in &glyphs[glyph_range.clone()] {
                        if text_width + glyph.advance > max_width_without_elision {
                            break;
                        }
                        text_width += glyph.advance;
                    }
                    return text_width + elide_width;
                }
                euclid::approxord::min(self.max_width, line.text_width)
            };

            let x = match self.horizontal_alignment {
                TextHorizontalAlignment::Start | TextHorizontalAlignment::Left => {
                    Font::Length::zero()
                }
                TextHorizontalAlignment::Center => self.max_width / two - text_width() / two,
                TextHorizontalAlignment::End | TextHorizontalAlignment::Right => {
                    self.max_width - text_width()
                }
            };

            let mut elide_glyph = elide_glyph.as_ref();

            let selection = selection
                .as_ref()
                .filter(|selection| {
                    line.byte_range.start < selection.end && selection.start < line.byte_range.end
                })
                .map(|selection| {
                    let mut begin = Font::Length::zero();
                    let mut end = Font::Length::zero();
                    for glyph in glyphs[line.glyph_range.clone()].iter() {
                        if glyph.text_byte_offset < selection.start {
                            begin += glyph.advance;
                        }
                        if glyph.text_byte_offset >= selection.end {
                            break;
                        }
                        end += glyph.advance;
                    }
                    begin..end
                });

            let glyph_it = glyphs[glyph_range.clone()].iter();
            let mut glyph_x = Font::Length::zero();
            // Up to two output glyphs per input glyph: the glyph itself, plus -- on the last glyph
            // of a vertically truncated line that still fits the width -- an ellipsis appended
            // after it (trailing whitespace was already trimmed from `glyph_range`).
            let mut positioned_glyph_it = glyph_it.enumerate().flat_map(|(index, glyph)| {
                let mut output: [Option<PositionedGlyph<Font::Length>>; 2] = [None, None];
                // TODO: cut off at grapheme boundaries
                if glyph_x > self.max_width {
                    return output.into_iter().flatten();
                }
                // A line that is too wide gets the ellipsis placed *instead of* the first glyph
                // that no longer fits (horizontal elision); the remaining glyphs are dropped.
                let elide_long_line = (elide_long_line || elide_last_line)
                    && x + glyph_x + glyph.advance > max_width_without_elision;
                if elide_long_line {
                    if let Some(elide_glyph) = elide_glyph.take() {
                        let x = glyph_x;
                        glyph_x += elide_glyph.advance;
                        output[0] = Some(PositionedGlyph {
                            x,
                            y: Font::Length::zero(),
                            advance: elide_glyph.advance,
                            glyph_id: elide_glyph.glyph_id.unwrap(), // checked earlier when initializing elide_glyph
                            text_byte_offset: glyph.text_byte_offset,
                        });
                    }
                    return output.into_iter().flatten();
                }

                let glyph_pos = glyph_x;
                glyph_x += glyph.advance;
                output[0] = glyph.glyph_id.map(|existing_glyph_id| PositionedGlyph {
                    x: glyph_pos,
                    y: Font::Length::zero(),
                    advance: glyph.advance,
                    glyph_id: existing_glyph_id,
                    text_byte_offset: glyph.text_byte_offset,
                });

                // A line that only overflows vertically (it fits the width) keeps all its glyphs
                // and gets the ellipsis *appended* after the last one rather than overwriting it,
                // matching the parley path which renders e.g. "Line Two..." not "Line Tw...".
                let last_glyph = glyph_range.start + index == glyph_range.end - 1;
                if elide_last_line
                    && last_glyph
                    && let Some(elide_glyph) = elide_glyph.take()
                {
                    let x = glyph_x;
                    glyph_x += elide_glyph.advance;
                    output[1] = Some(PositionedGlyph {
                        x,
                        y: Font::Length::zero(),
                        advance: elide_glyph.advance,
                        glyph_id: elide_glyph.glyph_id.unwrap(), // checked earlier when initializing elide_glyph
                        text_byte_offset: glyph.text_byte_offset,
                    });
                }

                output.into_iter().flatten()
            });

            if let core::ops::ControlFlow::Break(break_val) =
                line_callback(&mut positioned_glyph_it, x, y, line, selection)
            {
                return core::ops::ControlFlow::Break(break_val);
            }
            y += self.layout.font.height();
            line_index += 1;

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

    /// Returns the leading edge of the glyph at the given byte offset
    pub fn cursor_pos_for_byte_offset(&self, byte_offset: usize) -> (Font::Length, Font::Length) {
        let mut last_glyph_right_edge = Font::Length::zero();
        let mut last_line_y = Font::Length::zero();

        match self.layout_lines(
            |glyphs, line_x, line_y, line, _| {
                last_glyph_right_edge = euclid::approxord::min(
                    self.max_width,
                    line_x + line.width_including_trailing_whitespace(),
                );
                last_line_y = line_y;
                if byte_offset >= line.byte_range.end + line.trailing_whitespace_bytes {
                    return core::ops::ControlFlow::Continue(());
                }

                for positioned_glyph in glyphs {
                    if positioned_glyph.text_byte_offset == byte_offset {
                        return core::ops::ControlFlow::Break((
                            euclid::approxord::min(self.max_width, line_x + positioned_glyph.x),
                            last_line_y,
                        ));
                    }
                }

                core::ops::ControlFlow::Break((last_glyph_right_edge, last_line_y))
            },
            None,
        ) {
            Ok(_) => (last_glyph_right_edge, last_line_y),
            Err(position) => position,
        }
    }

    /// Returns the bytes offset for the given position
    pub fn byte_offset_for_position(&self, (pos_x, pos_y): (Font::Length, Font::Length)) -> usize {
        let mut byte_offset = 0;
        let two = Font::LengthPrimitive::one() + Font::LengthPrimitive::one();

        match self.layout_lines(
            |glyphs, line_x, line_y, line, _| {
                if pos_y >= line_y + self.layout.font.height() {
                    byte_offset = line.byte_range.end;
                    return core::ops::ControlFlow::Continue(());
                }

                if line.is_empty() {
                    return core::ops::ControlFlow::Break(line.byte_range.start);
                }

                while let Some(positioned_glyph) = glyphs.next() {
                    if pos_x >= line_x + positioned_glyph.x
                        && pos_x <= line_x + positioned_glyph.x + positioned_glyph.advance
                    {
                        if pos_x < line_x + positioned_glyph.x + positioned_glyph.advance / two {
                            return core::ops::ControlFlow::Break(
                                positioned_glyph.text_byte_offset,
                            );
                        } else if let Some(next_glyph) = glyphs.next() {
                            return core::ops::ControlFlow::Break(next_glyph.text_byte_offset);
                        }
                    }
                }

                core::ops::ControlFlow::Break(line.byte_range.end)
            },
            None,
        ) {
            Ok(_) => byte_offset,
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

    fn max_lines(&self, max_height: f32) -> usize {
        let height = self.ascent() - self.descent();
        (max_height / height).floor() as _
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

    fn x_height(&self) -> f32 {
        3.
    }

    fn cap_height(&self) -> f32 {
        4.
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
        max_lines: None,
    };
    paragraph
        .layout_lines::<()>(
            |glyphs, _, _, _, _| {
                lines.push(
                    glyphs.map(|positioned_glyph| positioned_glyph.glyph_id).collect::<Vec<_>>(),
                );
                core::ops::ControlFlow::Continue(())
            },
            None,
        )
        .unwrap();

    assert_eq!(lines.len(), 1);
    let rendered_text = lines[0]
        .iter()
        .flat_map(|glyph_id| {
            core::char::decode_utf16(core::iter::once(glyph_id.get()))
                .map(|r| r.unwrap())
                .collect::<Vec<char>>()
        })
        .collect::<std::string::String>();
    debug_assert_eq!(rendered_text, "This is a lo…")
}

#[test]
fn test_elision_vertical_truncation() {
    // A line that only overflows vertically (more lines below it were dropped for the height) but
    // fits the width keeps all its glyphs and gets the ellipsis appended -- "AB…", not "A…". The
    // box is one line tall, so the second line ("CD") is dropped.
    let font = FixedTestFont;
    let text = "AB\nCD";

    let mut lines = Vec::new();

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 4. * 10., // room for "AB…" (3 glyphs) and more
        max_height: 10.,     // one line tall
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::NoWrap,
        overflow: TextOverflow::Elide,
        single_line: false,
        max_lines: None,
    };
    paragraph
        .layout_lines::<()>(
            |glyphs, _, _, _, _| {
                lines.push(
                    glyphs.map(|positioned_glyph| positioned_glyph.glyph_id).collect::<Vec<_>>(),
                );
                core::ops::ControlFlow::Continue(())
            },
            None,
        )
        .unwrap();

    // Only the first line is drawn (the box is one line tall).
    assert_eq!(lines.len(), 1);
    let rendered_text = lines[0]
        .iter()
        .flat_map(|glyph_id| {
            core::char::decode_utf16(core::iter::once(glyph_id.get()))
                .map(|r| r.unwrap())
                .collect::<Vec<char>>()
        })
        .collect::<std::string::String>();
    assert_eq!(rendered_text, "AB…");
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
        max_lines: None,
    };
    paragraph
        .layout_lines::<()>(
            |glyphs, _, _, _, _| {
                lines.push(
                    glyphs.map(|positioned_glyph| positioned_glyph.glyph_id).collect::<Vec<_>>(),
                );
                core::ops::ControlFlow::Continue(())
            },
            None,
        )
        .unwrap();

    assert_eq!(lines.len(), 1);
    let rendered_text = lines[0]
        .iter()
        .flat_map(|glyph_id| {
            core::char::decode_utf16(core::iter::once(glyph_id.get()))
                .map(|r| r.unwrap())
                .collect::<Vec<char>>()
        })
        .collect::<std::string::String>();
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
        max_lines: None,
    };
    paragraph
        .layout_lines::<()>(
            |glyphs, _, _, _, _| {
                lines.push(
                    glyphs.map(|positioned_glyph| positioned_glyph.glyph_id).collect::<Vec<_>>(),
                );
                core::ops::ControlFlow::Continue(())
            },
            None,
        )
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
                .collect::<std::string::String>()
        })
        .collect::<Vec<_>>();
    debug_assert_eq!(rendered_text, std::vec!["Hello", "World"]);
}

#[test]
fn test_max_lines_limits_visible_lines() {
    let font = FixedTestFont;
    let text = "Hello\nWorld\nAgain";

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 100. * 10.,
        max_height: 100.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::NoWrap,
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: Some(2),
    };
    assert_eq!(render_lines(&paragraph), std::vec!["Hello", "World"]);
}

#[cfg(test)]
fn render_lines(paragraph: &TextParagraphLayout<'_, FixedTestFont>) -> Vec<std::string::String> {
    let mut lines = Vec::new();
    paragraph
        .layout_lines::<()>(
            |glyphs, _, _, _, _| {
                lines.push(
                    glyphs
                        .flat_map(|positioned_glyph| {
                            core::char::decode_utf16(core::iter::once(
                                positioned_glyph.glyph_id.get(),
                            ))
                            .map(|r| r.unwrap())
                            .collect::<Vec<char>>()
                        })
                        .collect::<std::string::String>(),
                );
                core::ops::ControlFlow::Continue(())
            },
            None,
        )
        .unwrap();
    lines
}

#[test]
fn test_max_lines_with_word_wrap() {
    let font = FixedTestFont;
    // Wraps to one word per line at 60px; the line limit counts the wrapped lines.
    let text = "Hello World Again";

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 6. * 10.,
        max_height: 100.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::WordWrap,
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: Some(2),
    };
    assert_eq!(render_lines(&paragraph), std::vec!["Hello ", "World "]);
}

#[test]
fn test_max_lines_elide_marks_cut_line() {
    let font = FixedTestFont;
    let text = "Hello\nWorld\nAgain";

    // The box is tall enough for all three lines, but the line limit cuts after the second:
    // with `overflow: elide` the ellipsis goes on the last kept line.
    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 100. * 10.,
        max_height: 100.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::NoWrap,
        overflow: TextOverflow::Elide,
        single_line: false,
        max_lines: Some(2),
    };
    assert_eq!(render_lines(&paragraph), std::vec!["Hello", "World…"]);
}

#[test]
fn test_text_size_with_max_lines() {
    let font = FixedTestFont;
    let layout = TextLayout { font: &font, letter_spacing: None };
    let text = "On\nFour\nLonger";

    let (width, height) = layout.text_size(text, None, TextWrap::NoWrap, None);
    assert_eq!((width, height), (6. * 10., 3. * 10.));

    // With a line limit, both the height and the longest-line width only cover the kept lines.
    let (width, height) = layout.text_size(text, None, TextWrap::NoWrap, Some(2));
    assert_eq!((width, height), (4. * 10., 2. * 10.));
}

#[test]
fn test_cursor_position() {
    let font = FixedTestFont;
    let text = "Hello                    World";

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 10. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::WordWrap,
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: None,
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
    assert_eq!(paragraph.cursor_pos_for_byte_offset(w_offset + 1), (10., 10.));

    assert_eq!(paragraph.cursor_pos_for_byte_offset(text.len()), (10. * 5., 10.));

    let first_space_offset =
        text.char_indices().find_map(|(offset, ch)| ch.is_whitespace().then_some(offset)).unwrap();
    assert_eq!(paragraph.cursor_pos_for_byte_offset(first_space_offset), (5. * 10., 0.));
    assert_eq!(paragraph.cursor_pos_for_byte_offset(first_space_offset + 15), (10. * 10., 0.));
    assert_eq!(paragraph.cursor_pos_for_byte_offset(first_space_offset + 16), (10. * 10., 0.));
}

#[test]
fn test_cursor_position_with_newline() {
    let font = FixedTestFont;
    let text = "Hello\nWorld";

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 100. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::WordWrap,
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: None,
    };

    assert_eq!(paragraph.cursor_pos_for_byte_offset(5), (5. * 10., 0.));
}

#[test]
fn byte_offset_for_empty_line() {
    let font = FixedTestFont;
    let text = "Hello\n\nWorld";

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 100. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::WordWrap,
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: None,
    };

    assert_eq!(paragraph.byte_offset_for_position((0., 10.)), 6);
}

#[test]
fn test_byte_offset() {
    let font = FixedTestFont;
    let text = "Hello                    World";
    let mut end_helper_text = std::string::String::from(text);
    end_helper_text.push('!');

    let paragraph = TextParagraphLayout {
        string: text,
        layout: TextLayout { font: &font, letter_spacing: None },
        max_width: 10. * 10.,
        max_height: 10.,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        wrap: TextWrap::WordWrap,
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: None,
    };

    assert_eq!(paragraph.byte_offset_for_position((0., 0.)), 0);

    let e_offset = text
        .char_indices()
        .find_map(|(offset, ch)| if ch == 'e' { Some(offset) } else { None })
        .unwrap();

    assert_eq!(paragraph.byte_offset_for_position((14., 0.)), e_offset);

    let l_offset = text
        .char_indices()
        .find_map(|(offset, ch)| if ch == 'l' { Some(offset) } else { None })
        .unwrap();
    assert_eq!(paragraph.byte_offset_for_position((15., 0.)), l_offset);

    let w_offset = text
        .char_indices()
        .find_map(|(offset, ch)| if ch == 'W' { Some(offset) } else { None })
        .unwrap();

    assert_eq!(paragraph.byte_offset_for_position((10., 10.)), w_offset + 1);

    let o_offset = text
        .char_indices()
        .rev()
        .find_map(|(offset, ch)| if ch == 'o' { Some(offset) } else { None })
        .unwrap();

    assert_eq!(paragraph.byte_offset_for_position((15., 10.)), o_offset + 1);

    let d_offset = text
        .char_indices()
        .rev()
        .find_map(|(offset, ch)| if ch == 'd' { Some(offset) } else { None })
        .unwrap();

    assert_eq!(paragraph.byte_offset_for_position((40., 10.)), d_offset);

    let end_offset = end_helper_text
        .char_indices()
        .rev()
        .find_map(|(offset, ch)| if ch == '!' { Some(offset) } else { None })
        .unwrap();

    assert_eq!(paragraph.byte_offset_for_position((45., 10.)), end_offset);
    assert_eq!(paragraph.byte_offset_for_position((0., 20.)), end_offset);
}
