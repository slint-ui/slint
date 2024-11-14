// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{
    graphics::{BitmapFont, BitmapGlyphs},
    software_renderer::PhysicalLength,
    textlayout::{FontMetrics, Glyph, TextShaper},
};

use super::{GlyphRenderer, RenderableGlyph};

impl BitmapGlyphs {
    /// Returns the size of the pre-rendered font in pixels.
    pub fn pixel_size(&self) -> PhysicalLength {
        PhysicalLength::new(self.pixel_size)
    }
}

// A font that is resolved to a specific pixel size.
pub struct PixelFont {
    pub bitmap_font: &'static BitmapFont,
    pub glyphs: &'static BitmapGlyphs,
    pub pixel_size: PhysicalLength,
}

impl PixelFont {
    pub fn glyph_index_to_glyph_id(index: usize) -> core::num::NonZeroU16 {
        core::num::NonZeroU16::new(index as u16 + 1).unwrap()
    }
    pub fn glyph_id_to_glyph_index(id: core::num::NonZeroU16) -> usize {
        id.get() as usize - 1
    }

    /// Convert from the glyph coordinate to the target coordinate
    pub fn scale_glyph_length(&self, v: i16) -> PhysicalLength {
        (self.pixel_size.cast() * v as i32 / self.glyphs.pixel_size as i32).cast()
    }
}

impl GlyphRenderer for PixelFont {
    fn render_glyph(&self, glyph_id: core::num::NonZeroU16) -> Option<RenderableGlyph> {
        let glyph_index = Self::glyph_id_to_glyph_index(glyph_id);
        let bitmap_glyph = &self.glyphs.glyph_data[glyph_index];
        if bitmap_glyph.data.len() == 0 {
            // For example, ' ' has no glyph data
            return None;
        }
        let width = self.scale_glyph_length(bitmap_glyph.width - 1) + PhysicalLength::new(1);
        let height = self.scale_glyph_length(bitmap_glyph.height - 1) + PhysicalLength::new(1);
        Some(RenderableGlyph {
            x: self.scale_glyph_length(bitmap_glyph.x) / 64,
            y: self.scale_glyph_length(bitmap_glyph.y + bitmap_glyph.height * 64) / 64 - height,
            width,
            height,
            alpha_map: bitmap_glyph.data.as_slice().into(),
            pixel_stride: bitmap_glyph.width as u16,
            sdf: self.bitmap_font.sdf,
        })
    }
    fn scale_delta(&self) -> super::Fixed<u16, 8> {
        super::Fixed::from_integer(self.glyphs.pixel_size as u16) / self.pixel_size.get() as u16
    }
}

impl TextShaper for PixelFont {
    type LengthPrimitive = i16;
    type Length = PhysicalLength;
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<PhysicalLength>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let glyphs_iter = text.char_indices().map(|(byte_offset, char)| {
            let glyph_index = self
                .bitmap_font
                .character_map
                .binary_search_by_key(&char, |char_map_entry| char_map_entry.code_point)
                .ok()
                .map(|char_map_index| {
                    self.bitmap_font.character_map[char_map_index].glyph_index as usize
                });
            let x_advance = glyph_index.map_or_else(
                || self.pixel_size,
                |glyph_index| {
                    (self.pixel_size.cast() * self.glyphs.glyph_data[glyph_index].x_advance as i32
                        / self.glyphs.pixel_size as i32
                        / 64)
                        .cast()
                },
            );
            Glyph {
                glyph_id: glyph_index.map(Self::glyph_index_to_glyph_id),
                advance: x_advance,
                text_byte_offset: byte_offset,
                ..Default::default()
            }
        });
        glyphs.extend(glyphs_iter);
    }

    fn glyph_for_char(&self, ch: char) -> Option<Glyph<PhysicalLength>> {
        self.bitmap_font
            .character_map
            .binary_search_by_key(&ch, |char_map_entry| char_map_entry.code_point)
            .ok()
            .map(|char_map_index| {
                let glyph_index =
                    self.bitmap_font.character_map[char_map_index].glyph_index as usize;
                let bitmap_glyph = &self.glyphs.glyph_data[glyph_index];
                let x_advance = PhysicalLength::new(bitmap_glyph.x_advance);
                Glyph {
                    glyph_id: Some(Self::glyph_index_to_glyph_id(glyph_index)),
                    advance: x_advance,
                    text_byte_offset: 0,
                    ..Default::default()
                }
            })
    }

    fn max_lines(&self, max_height: PhysicalLength) -> usize {
        (max_height / self.height()).get() as _
    }
}

impl FontMetrics<PhysicalLength> for PixelFont {
    fn ascent(&self) -> PhysicalLength {
        (self.pixel_size.cast() * self.bitmap_font.ascent / self.bitmap_font.units_per_em).cast()
    }

    fn descent(&self) -> PhysicalLength {
        (self.pixel_size.cast() * self.bitmap_font.descent / self.bitmap_font.units_per_em).cast()
    }

    fn height(&self) -> PhysicalLength {
        // The descent is negative (relative to the baseline)
        (self.pixel_size.cast() * (self.bitmap_font.ascent - self.bitmap_font.descent)
            / self.bitmap_font.units_per_em)
            .cast()
    }

    fn x_height(&self) -> PhysicalLength {
        (self.pixel_size.cast() * self.bitmap_font.x_height / self.bitmap_font.units_per_em).cast()
    }

    fn cap_height(&self) -> PhysicalLength {
        (self.pixel_size.cast() * self.bitmap_font.cap_height / self.bitmap_font.units_per_em)
            .cast()
    }
}
