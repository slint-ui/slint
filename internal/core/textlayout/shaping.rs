// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::vec::Vec;
use core::ops::Range;

use super::TextLayout;

/// This struct describes a glyph from shaping to rendering. This includes the relative shaping
/// offsets, advance (in abstract lengths) and platform specific glyph data.
#[derive(Clone, Default, Debug)]
pub struct Glyph<Length> {
    pub advance: Length,
    pub offset_x: Length,
    pub offset_y: Length,
    /// Glyph IDs are font specific identifiers. In TrueType fonts zero indicates the missing glyph, which
    /// is mapped to an Option here.
    pub glyph_id: Option<core::num::NonZeroU16>,
    /// The byte offset back in the original (Rust) string to the character that
    /// "produced" this glyph. When one character produces multiple glyphs (for example
    /// decomposed ligature), then all glyphs have the same offset.
    pub text_byte_offset: usize,
}

/// This trait defines the interface between the text layout and the platform specific
/// mapping of text to glyphs. An implementation of the TextShaper trait must provide
/// metric types (Length, LengthPrimitive), which is used for the line breaking calculation
/// and glyph positioning, as well as an opaque platform specific glyph data type.
///
/// Functionality wise it provides the ability to convert a string into a set of glyphs,
/// each of which has basic metric fields as well as an offset back into the original string.
/// Typically this is implemented by using a general text shaper, which performs an M:N mapping
/// from unicode characters to glyphs, via glyph substitutions and script specific rules. In addition
/// the glyphs may be positioned for the required appearance (such as stacked diacritics).
///
/// Finally, for convenience the TextShaper also provides a single glyph_for_char function, for example
/// used to lookup single glyphs (such as the elision character) as well as additional metrics
/// used for text paragraph layout.
pub trait TextShaper {
    type LengthPrimitive: core::ops::Mul
        + core::ops::Div
        + core::ops::Add<Output = Self::LengthPrimitive>
        + core::ops::AddAssign
        + euclid::num::Zero
        + euclid::num::One
        + core::convert::From<i16>
        + Copy
        + core::fmt::Debug;
    type Length: euclid::num::Zero
        + core::ops::AddAssign
        + core::ops::Add<Output = Self::Length>
        + core::ops::Sub<Output = Self::Length>
        + Default
        + Clone
        + Copy
        + core::cmp::PartialOrd
        + core::ops::Mul<Self::LengthPrimitive, Output = Self::Length>
        + core::ops::Div<Self::LengthPrimitive, Output = Self::Length>
        + core::fmt::Debug;
    // Shapes the given string and emits the result into the given glyphs buffer.
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<Self::Length>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    );
    fn glyph_for_char(&self, ch: char) -> Option<Glyph<Self::Length>>;
    fn max_lines(&self, max_height: Self::Length) -> usize;
}

pub trait FontMetrics<Length: Copy + core::ops::Sub<Output = Length>> {
    fn height(&self) -> Length {
        self.ascent() - self.descent()
    }
    fn ascent(&self) -> Length;
    fn descent(&self) -> Length;
    fn x_height(&self) -> Length;
    fn cap_height(&self) -> Length;
}

pub trait AbstractFont: TextShaper + FontMetrics<<Self as TextShaper>::Length> {}

impl<T> AbstractFont for T where T: TextShaper + FontMetrics<<Self as TextShaper>::Length> {}

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

impl Iterator for ShapeBoundaries<'_> {
    type Item = usize;

    #[cfg(feature = "unicode-script")]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_boundary_start?;

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

#[derive(Debug)]
pub struct TextRun {
    pub byte_range: Range<usize>,
    pub glyph_range: Range<usize>,
    // TODO: direction, etc.
}

pub struct ShapeBuffer<Length> {
    pub glyphs: Vec<Glyph<Length>>,
    pub text_runs: Vec<TextRun>,
}

impl<Length> ShapeBuffer<Length> {
    pub fn new<Font>(layout: &TextLayout<Font>, text: &str) -> Self
    where
        Font: AbstractFont<Length = Length>,
        Length: Copy + core::ops::AddAssign,
    {
        let mut glyphs = Vec::new();
        let text_runs = ShapeBoundaries::new(text)
            .scan(0, |run_start, run_end| {
                let glyphs_start = glyphs.len();

                layout.font.shape_text(&text[*run_start..run_end], &mut glyphs);

                if let Some(letter_spacing) = layout.letter_spacing {
                    if glyphs.len() > glyphs_start {
                        let mut last_byte_offset = glyphs[glyphs_start].text_byte_offset;
                        for index in glyphs_start + 1..glyphs.len() {
                            let current_glyph_byte_offset = glyphs[index].text_byte_offset;
                            if current_glyph_byte_offset != last_byte_offset {
                                let previous_glyph = &mut glyphs[index - 1];
                                previous_glyph.advance += letter_spacing;
                            }
                            last_byte_offset = current_glyph_byte_offset;
                        }

                        glyphs.last_mut().unwrap().advance += letter_spacing;
                    }
                }

                let run = TextRun {
                    byte_range: Range { start: *run_start, end: run_end },
                    glyph_range: Range { start: glyphs_start, end: glyphs.len() },
                };
                *run_start = run_end;

                Some(run)
            })
            .collect();

        Self { glyphs, text_runs }
    }
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
#[cfg_attr(
    not(feature = "unicode-script"),
    ignore = "Not supported without the unicode-script feature"
)]
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
impl TextShaper for &rustybuzz::Face<'_> {
    type LengthPrimitive = f32;
    type Length = f32;
    fn shape_text<GlyphStorage: std::iter::Extend<Glyph<f32>>>(
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
                    let mut out_glyph = Glyph::default();
                    out_glyph.glyph_id = core::num::NonZeroU16::new(info.glyph_id as u16);
                    out_glyph.offset_x = position.x_offset as _;
                    out_glyph.offset_y = position.y_offset as _;
                    out_glyph.advance = position.x_advance as _;
                    out_glyph.text_byte_offset = info.cluster as usize;
                    out_glyph
                },
            );

        // Cannot return impl Iterator, so extend argument instead
        glyphs.extend(output_glyph_generator);
    }

    fn glyph_for_char(&self, _ch: char) -> Option<Glyph<f32>> {
        todo!()
    }

    fn max_lines(&self, max_height: f32) -> usize {
        (max_height / self.height()).floor() as _
    }
}

#[cfg(test)]
impl FontMetrics<f32> for &rustybuzz::Face<'_> {
    fn ascent(&self) -> f32 {
        self.ascender() as _
    }

    fn descent(&self) -> f32 {
        self.descender() as _
    }

    fn x_height(&self) -> f32 {
        rustybuzz::ttf_parser::Face::x_height(self).unwrap_or_default() as _
    }

    fn cap_height(&self) -> f32 {
        rustybuzz::ttf_parser::Face::capital_height(self).unwrap_or_default() as _
    }
}

#[cfg(test)]
fn with_dejavu_font<R>(mut callback: impl FnMut(&rustybuzz::Face<'_>) -> R) -> Option<R> {
    let mut fontdb = fontdb::Database::new();
    let dejavu_path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "common", "sharedfontdb", "DejaVuSans.ttf"]
            .iter()
            .collect();
    fontdb.load_font_file(dejavu_path).expect("unable to load test dejavu font");
    let font_id = fontdb.faces().next().unwrap().id;
    fontdb.with_face_data(font_id, |data, font_index| {
        let face =
            rustybuzz::Face::from_slice(data, font_index).expect("unable to parse dejavu font");
        callback(&face)
    })
}

#[test]
fn test_shaping() {
    use std::num::NonZeroU16;
    use TextShaper;

    with_dejavu_font(|face| {
        {
            let mut shaped_glyphs = Vec::new();
            // two glyph clusters: ā́b
            face.shape_text("a\u{0304}\u{0301}b", &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);
            assert_eq!(shaped_glyphs[0].glyph_id, NonZeroU16::new(195));
            assert_eq!(shaped_glyphs[0].text_byte_offset, 0);

            assert_eq!(shaped_glyphs[1].glyph_id, NonZeroU16::new(690));
            assert_eq!(shaped_glyphs[1].text_byte_offset, 0);

            assert_eq!(shaped_glyphs[2].glyph_id, NonZeroU16::new(69));
            assert_eq!(shaped_glyphs[2].text_byte_offset, 5);
        }

        {
            let mut shaped_glyphs = Vec::new();
            // two glyph clusters: ā́b
            face.shape_text("a b", &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);
            assert_eq!(shaped_glyphs[0].glyph_id, NonZeroU16::new(68));
            assert_eq!(shaped_glyphs[0].text_byte_offset, 0);

            assert_eq!(shaped_glyphs[1].text_byte_offset, 1);

            assert_eq!(shaped_glyphs[2].glyph_id, NonZeroU16::new(69));
            assert_eq!(shaped_glyphs[2].text_byte_offset, 2);
        }
    });
}

#[test]
fn test_letter_spacing() {
    use TextShaper;

    with_dejavu_font(|face| {
        // two glyph clusters: ā́b
        let text = "a\u{0304}\u{0301}b";
        let advances = {
            let mut shaped_glyphs = Vec::new();
            face.shape_text(text, &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);

            shaped_glyphs.iter().map(|g| g.advance).collect::<Vec<_>>()
        };

        let layout = TextLayout { font: &face, letter_spacing: Some(20.) };
        let buffer = ShapeBuffer::new(&layout, text);

        assert_eq!(buffer.glyphs.len(), advances.len());

        let mut expected_advances = advances;
        expected_advances[1] += layout.letter_spacing.unwrap();
        *expected_advances.last_mut().unwrap() += layout.letter_spacing.unwrap();

        assert_eq!(
            buffer.glyphs.iter().map(|glyph| glyph.advance).collect::<Vec<_>>(),
            expected_advances
        );
    });
}
