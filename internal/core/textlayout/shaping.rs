// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::vec::Vec;
use core::ops::Range;

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

pub struct TextRun {
    pub byte_range: Range<usize>,
    //pub glyph_range: Range<usize>,
    // TODO: direction, etc.
}

pub struct ShapeBuffer<Font: TextShaper + ?Sized> {
    pub glyphs: Vec<(Font::Glyph, usize)>,
    pub text_runs: Vec<TextRun>,
}

impl<Font: TextShaper + ?Sized> ShapeBuffer<Font> {
    pub fn new(font: &Font, text: &str) -> Self {
        let mut glyphs = Vec::new();
        let text_runs = ShapeBoundaries::new(text)
            .scan(0, |run_start, run_end| {
                //let glyphs_start = glyph_buffer.borrow().as_ref().len();

                font.shape_text(&text[*run_start..run_end], &mut glyphs);

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

#[cfg(test)]

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
