// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::vec::Vec;
use core::ops::Range;

/// This struct describes a glyph from shaping to rendering. This includes the relative shaping
/// offsets, advance (in abstract lengths) and platform specific glyph data.
#[derive(Clone, Default, Debug)]
pub struct Glyph<Length, PlatformGlyphData> {
    pub advance: Length,
    pub offset_x: Length,
    pub offset_y: Length,
    pub platform_glyph: PlatformGlyphData,
    pub byte_offset: usize,
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
    type PlatformGlyphData: Clone;
    // Shapes the given string and emits the result into the given glyphs buffer,
    // as tuples of glyph handle and corresponding byte offset in the originating string.
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<Self::Length, Self::PlatformGlyphData>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    );
    fn glyph_for_char(&self, ch: char) -> Option<Glyph<Self::Length, Self::PlatformGlyphData>>;

    fn letter_spacing(&self) -> Option<Self::Length> {
        None
    }
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

pub struct TextRun {
    pub byte_range: Range<usize>,
    //pub glyph_range: Range<usize>,
    // TODO: direction, etc.
}

pub struct ShapeBuffer<Length, PlatformGlyphData> {
    pub glyphs: Vec<Glyph<Length, PlatformGlyphData>>,
    pub text_runs: Vec<TextRun>,
}

impl<Length, PlatformGlyphData> ShapeBuffer<Length, PlatformGlyphData> {
    pub fn new<Font>(font: &Font, text: &str) -> Self
    where
        Font: TextShaper<Length = Length, PlatformGlyphData = PlatformGlyphData>,
        Length: Copy + core::ops::AddAssign,
    {
        let mut glyphs = Vec::new();
        let text_runs = ShapeBoundaries::new(text)
            .scan(0, |run_start, run_end| {
                let glyphs_start = glyphs.len();

                font.shape_text(&text[*run_start..run_end], &mut glyphs);

                if let Some(letter_spacing) = font.letter_spacing() {
                    if glyphs.len() > glyphs_start {
                        let mut last_byte_offset = glyphs[glyphs_start].byte_offset;
                        for index in glyphs_start + 1..glyphs.len() {
                            let current_glyph_byte_offset = glyphs[index].byte_offset;
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
                    //glyph_range: Range {
                    //     start: glyphs_start,
                    //     end: glyph_buffer.borrow().as_ref().len(),
                    // },
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
#[derive(Clone, Debug, Default)]
pub struct TestGlyphData {
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub width: f32,
    pub height: f32,
    pub glyph_id: Option<core::num::NonZeroU16>,
    pub char: Option<char>,
}

#[cfg(test)]
struct RustyBuzzFont<'a> {
    face: rustybuzz::Face<'a>,
    letter_spacing: Option<f32>,
}

#[cfg(test)]
impl<'a> core::convert::From<rustybuzz::Face<'a>> for RustyBuzzFont<'a> {
    fn from(face: rustybuzz::Face<'a>) -> Self {
        Self { face, letter_spacing: None }
    }
}

#[cfg(test)]

impl<'a> TextShaper for RustyBuzzFont<'a> {
    type LengthPrimitive = f32;
    type Length = f32;
    type PlatformGlyphData = TestGlyphData;
    fn shape_text<GlyphStorage: std::iter::Extend<Glyph<f32, Self::PlatformGlyphData>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let glyph_buffer = rustybuzz::shape(&self.face, &[], buffer);

        let output_glyph_generator =
            glyph_buffer.glyph_infos().iter().zip(glyph_buffer.glyph_positions().iter()).map(
                |(info, position)| {
                    let mut out_glyph = Glyph::default();

                    out_glyph.platform_glyph = Self::PlatformGlyphData {
                        glyph_id: core::num::NonZeroU16::new(info.glyph_id as u16),
                        char: text[info.cluster as usize..].chars().next(),
                        ..Default::default()
                    };

                    out_glyph.offset_x = position.x_offset as _;
                    out_glyph.offset_y = position.y_offset as _;
                    out_glyph.advance = position.x_advance as _;

                    if let Some(bounding_box) = out_glyph
                        .platform_glyph
                        .glyph_id
                        .and_then(|id| self.face.glyph_bounding_box(ttf_parser::GlyphId(id.get())))
                    {
                        out_glyph.platform_glyph.width = bounding_box.width() as _;
                        out_glyph.platform_glyph.height = bounding_box.height() as _;
                        out_glyph.platform_glyph.bearing_x = bounding_box.x_min as _;
                        out_glyph.platform_glyph.bearing_y = bounding_box.y_min as _;
                    }

                    out_glyph.byte_offset = info.cluster as usize;

                    out_glyph
                },
            );

        // Cannot return impl Iterator, so extend argument instead
        glyphs.extend(output_glyph_generator);
    }

    fn glyph_for_char(&self, _ch: char) -> Option<Glyph<f32, Self::PlatformGlyphData>> {
        todo!()
    }

    fn letter_spacing(&self) -> Option<Self::Length> {
        self.letter_spacing
    }
}

#[cfg(test)]
fn with_dejavu_font<R>(mut callback: impl FnMut(&mut RustyBuzzFont) -> R) -> Option<R> {
    let mut fontdb = fontdb::Database::new();
    let dejavu_path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "backends", "gl", "fonts", "DejaVuSans.ttf"]
            .iter()
            .collect();
    fontdb.load_font_file(dejavu_path).expect("unable to load test dejavu font");
    let font_id = fontdb.faces()[0].id;
    fontdb.with_face_data(font_id, |data, font_index| {
        let mut face: RustyBuzzFont = rustybuzz::Face::from_slice(data, font_index)
            .expect("unable to parse dejavu font")
            .into();
        callback(&mut face)
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
            assert_eq!(shaped_glyphs[0].platform_glyph.glyph_id, NonZeroU16::new(195));
            assert_eq!(shaped_glyphs[0].byte_offset, 0);

            assert_eq!(shaped_glyphs[1].platform_glyph.glyph_id, NonZeroU16::new(690));
            assert_eq!(shaped_glyphs[1].byte_offset, 0);

            assert_eq!(shaped_glyphs[2].platform_glyph.glyph_id, NonZeroU16::new(69));
            assert_eq!(shaped_glyphs[2].byte_offset, 5);
        }

        {
            let mut shaped_glyphs = Vec::new();
            // two glyph clusters: ā́b
            face.shape_text("a b", &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);
            assert_eq!(shaped_glyphs[0].platform_glyph.glyph_id, NonZeroU16::new(68));
            assert_eq!(shaped_glyphs[0].byte_offset, 0);

            assert_eq!(shaped_glyphs[1].byte_offset, 1);

            assert_eq!(shaped_glyphs[2].platform_glyph.glyph_id, NonZeroU16::new(69));
            assert_eq!(shaped_glyphs[2].byte_offset, 2);
        }
    });
}

#[test]
fn test_letter_spacing() {
    use TextShaper;

    with_dejavu_font(|mut face| {
        // two glyph clusters: ā́b
        let text = "a\u{0304}\u{0301}b";
        let advances = {
            let mut shaped_glyphs = Vec::new();
            face.shape_text(text, &mut shaped_glyphs);

            assert_eq!(shaped_glyphs.len(), 3);

            shaped_glyphs.iter().map(|g| g.advance).collect::<Vec<_>>()
        };

        face.letter_spacing = Some(20.);

        let buffer = ShapeBuffer::new(face, text);

        assert_eq!(buffer.glyphs.len(), advances.len());

        let mut expected_advances = advances;
        expected_advances[1] += face.letter_spacing.unwrap();
        *expected_advances.last_mut().unwrap() += face.letter_spacing.unwrap();

        assert_eq!(
            buffer.glyphs.iter().map(|glyph| glyph.advance).collect::<Vec<_>>(),
            expected_advances
        );
    });
}
