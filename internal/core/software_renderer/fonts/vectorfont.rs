// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::cell::RefCell;

use alloc::rc::Rc;

use crate::lengths::PhysicalPx;
use crate::software_renderer::PhysicalLength;
use crate::textlayout::{Glyph, TextShaper};

// A length in font design space.
struct FontUnit;
type FontLength = euclid::Length<i32, FontUnit>;
type FontScaleFactor = euclid::Scale<f32, FontUnit, PhysicalPx>;

pub struct VectorFont {
    db: Rc<RefCell<fontdb::Database>>,
    id: fontdb::ID,
    ascender: PhysicalLength,
    descender: PhysicalLength,
    height: PhysicalLength,
    scale: FontScaleFactor,
    pixel_size: PhysicalLength,
}

impl VectorFont {
    pub fn new(
        db: Rc<RefCell<fontdb::Database>>,
        id: fontdb::ID,
        pixel_size: PhysicalLength,
    ) -> Self {
        let db_for_font = db.clone();
        db.borrow()
            .with_face_data(id, |face_data, font_index| {
                let face = rustybuzz::ttf_parser::Face::parse(face_data, font_index).unwrap();

                let ascender = FontLength::new(face.ascender() as _);
                let descender = FontLength::new(face.descender() as _);
                let height = FontLength::new(face.height() as _);
                let units_per_em = face.units_per_em();
                let scale = FontScaleFactor::new(pixel_size.get() as f32 / units_per_em as f32);
                Self {
                    db: db_for_font,
                    id,
                    ascender: (ascender.cast() * scale).cast(),
                    descender: (descender.cast() * scale).cast(),
                    height: (height.cast() * scale).cast(),
                    scale,
                    pixel_size,
                }
            })
            .unwrap()
    }
}

impl TextShaper for VectorFont {
    type LengthPrimitive = i16;
    type Length = PhysicalLength;
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<PhysicalLength>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);

        self.db
            .borrow()
            .with_face_data(self.id, |face_data, font_index| {
                let face = rustybuzz::ttf_parser::Face::parse(face_data, font_index).unwrap();
                let rb_face = rustybuzz::Face::from_face(face).unwrap();

                let glyph_buffer = rustybuzz::shape(&rb_face, &[], buffer);

                let output_glyph_generator = glyph_buffer
                    .glyph_infos()
                    .iter()
                    .zip(glyph_buffer.glyph_positions().iter())
                    .map(|(info, position)| {
                        let mut out_glyph = Glyph::<PhysicalLength>::default();

                        out_glyph.glyph_id = core::num::NonZeroU16::new(info.glyph_id as u16);

                        out_glyph.offset_x =
                            (FontLength::new(position.x_offset).cast() * self.scale).cast();
                        out_glyph.offset_y =
                            (FontLength::new(position.y_offset).cast() * self.scale).cast();
                        out_glyph.advance =
                            (FontLength::new(position.x_advance).cast() * self.scale).cast();

                        out_glyph.text_byte_offset = info.cluster as usize;

                        out_glyph
                    });

                // Cannot return impl Iterator, so extend argument instead
                glyphs.extend(output_glyph_generator);
            })
            .unwrap()
    }

    fn glyph_for_char(&self, ch: char) -> Option<Glyph<PhysicalLength>> {
        self.db
            .borrow()
            .with_face_data(self.id, |face_data, font_index| {
                let face = rustybuzz::ttf_parser::Face::parse(face_data, font_index).unwrap();
                face.glyph_index(ch).map(|glyph_index| {
                    let mut out_glyph = Glyph::default();

                    out_glyph.glyph_id = core::num::NonZeroU16::new(glyph_index.0 as u16);

                    out_glyph.advance = (FontLength::new(
                        face.glyph_hor_advance(glyph_index).unwrap_or_default() as _,
                    )
                    .cast()
                        * self.scale)
                        .cast();

                    out_glyph
                })
            })
            .unwrap()
    }
}

impl crate::textlayout::FontMetrics<PhysicalLength> for VectorFont {
    fn ascent(&self) -> PhysicalLength {
        self.ascender
    }

    fn height(&self) -> PhysicalLength {
        self.height
    }

    fn descent(&self) -> PhysicalLength {
        self.descender
    }
}

impl super::GlyphRenderer for VectorFont {
    fn render_glyph(&self, glyph_id: core::num::NonZeroU16) -> super::RenderableGlyph {
        // FIXME: This is very slow, parses the font every time, and re-renders. Should cache the rasterized glyphs.
        self.db
            .borrow()
            .with_face_data(self.id, |face_data, font_index| {
                let font = fontdue::Font::from_bytes(
                    face_data,
                    fontdue::FontSettings { collection_index: font_index, scale: 40. },
                )
                .expect("fatal: fontdue is unable to parse truetype font");

                let (metrics, alpha_map) =
                    font.rasterize_indexed(glyph_id.get(), self.pixel_size.get() as _);

                let alpha_map: Rc<[u8]> = alpha_map.into();

                super::RenderableGlyph {
                    x: PhysicalLength::new(metrics.xmin.try_into().unwrap()),
                    y: PhysicalLength::new(metrics.ymin.try_into().unwrap()),
                    width: PhysicalLength::new(metrics.width.try_into().unwrap()),
                    height: PhysicalLength::new(metrics.height.try_into().unwrap()),
                    alpha_map: alpha_map.into(),
                }
            })
            .unwrap()
    }
}
