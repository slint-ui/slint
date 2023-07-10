// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use crate::{
    graphics::{BitmapFont, BitmapGlyphs},
    software_renderer::PhysicalLength,
    textlayout::{Glyph, TextShaper},
};

use super::{GlyphRenderer, RenderableGlyph};

impl BitmapGlyphs {
    fn ascent(&self, font: &BitmapFont) -> PhysicalLength {
        (PhysicalLength::new(self.pixel_size).cast() * font.ascent / font.units_per_em).cast()
    }
    fn descent(&self, font: &BitmapFont) -> PhysicalLength {
        (PhysicalLength::new(self.pixel_size).cast() * font.descent / font.units_per_em).cast()
    }
    fn height(&self, font: &BitmapFont) -> PhysicalLength {
        // The descent is negative (relative to the baseline)
        (PhysicalLength::new(self.pixel_size).cast() * (font.ascent - font.descent)
            / font.units_per_em)
            .cast()
    }
    /// Returns the size of the pre-rendered font in pixels.
    pub fn pixel_size(&self) -> PhysicalLength {
        PhysicalLength::new(self.pixel_size)
    }
}

// A font that is resolved to a specific pixel size.
pub struct PixelFont {
    pub bitmap_font: &'static BitmapFont,
    pub glyphs: &'static BitmapGlyphs,
}

impl PixelFont {
    pub fn pixel_size(&self) -> PhysicalLength {
        self.glyphs.pixel_size()
    }
    pub fn glyph_index_to_glyph_id(index: usize) -> core::num::NonZeroU16 {
        core::num::NonZeroU16::new(index as u16 + 1).unwrap()
    }
    pub fn glyph_id_to_glyph_index(id: core::num::NonZeroU16) -> usize {
        id.get() as usize - 1
    }
}

impl GlyphRenderer for PixelFont {
    fn render_glyph(&self, glyph_id: core::num::NonZeroU16) -> RenderableGlyph {
        let glyph_index = Self::glyph_id_to_glyph_index(glyph_id);
        let bitmap_glyph = &self.glyphs.glyph_data[glyph_index];
        RenderableGlyph {
            x: PhysicalLength::new(bitmap_glyph.x),
            y: PhysicalLength::new(bitmap_glyph.y),
            width: PhysicalLength::new(bitmap_glyph.width),
            height: PhysicalLength::new(bitmap_glyph.height),
            alpha_map: bitmap_glyph.data.as_slice().into(),
        }
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
                || self.pixel_size(),
                |glyph_index| PhysicalLength::new(self.glyphs.glyph_data[glyph_index].x_advance),
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
}

impl crate::textlayout::FontMetrics<PhysicalLength> for PixelFont {
    fn ascent(&self) -> PhysicalLength {
        self.glyphs.ascent(self.bitmap_font)
    }

    fn descent(&self) -> PhysicalLength {
        self.glyphs.descent(self.bitmap_font)
    }

    fn height(&self) -> PhysicalLength {
        self.glyphs.height(self.bitmap_font)
    }
}
