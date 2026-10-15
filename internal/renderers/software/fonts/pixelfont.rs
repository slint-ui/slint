// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::PhysicalLength;
use crate::fixed::Fixed;
use i_slint_core::graphics::{BitmapFont, BitmapGlyphs};
use i_slint_core::textlayout::{FontMetrics, Glyph, TextShaper};

use super::{GlyphRenderer, RenderableGlyph};

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

    fn glyph_advance(&self, advance: i16) -> PhysicalLength {
        if self.pixel_size.get() == self.glyphs.pixel_size {
            PhysicalLength::new(((advance as i32 + 32) / 64) as i16)
        } else {
            ((self.pixel_size.cast() * advance as i32 / self.glyphs.pixel_size as i32
                + euclid::Length::new(32))
                / 64)
                .cast()
        }
    }

    pub(super) fn render_bitmap_glyph(
        &self,
        bitmap_glyph: &i_slint_core::graphics::BitmapGlyph,
    ) -> Option<RenderableGlyph> {
        if bitmap_glyph.data.is_empty() {
            // For example, ' ' has no glyph data.
            return None;
        }
        let src_x = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(bitmap_glyph.x));
        let src_y = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(bitmap_glyph.y));
        let h_plus_y = Fixed::<i32, 8>::from_integer(bitmap_glyph.height as i32) + src_y;
        let (x, h_plus_y, width, height) = if self.pixel_size.get() == self.glyphs.pixel_size {
            let off_y = h_plus_y.0 & 0xff;
            let off_x = -src_x.0 & 0xff;
            let height = ((bitmap_glyph.height as i32 - 1) * 256 - off_y) / 256 + 1;
            let width = ((bitmap_glyph.width as i32 - 1) * 256 - off_x) / 256 + 1;
            (src_x, h_plus_y, width, height)
        } else {
            // Align the scaled glyph while keeping the source sampling offsets fractional.
            let delta = Fixed::<i32, 8>::from_fixed(self.scale_delta());
            let h_plus_y = Fixed::<i32, 8>::from_fraction(h_plus_y.0, delta.0);
            let off_y = Fixed::<i32, 8>(h_plus_y.0 & 0xff);
            let height = (Fixed::from_integer(bitmap_glyph.height as i32 - 1) - off_y) / delta + 1;
            let x = Fixed::from_fraction(src_x.0, delta.0);
            let off_x = Fixed::<i32, 8>(-x.0 & 0xff);
            let width = (Fixed::from_integer(bitmap_glyph.width as i32 - 1) - off_x) / delta + 1;
            (x, h_plus_y, width, height)
        };
        Some(RenderableGlyph {
            x,
            y: h_plus_y - Fixed::from_integer(height),
            width: PhysicalLength::new(width as i16),
            height: PhysicalLength::new(height as i16),
            alpha_map: bitmap_glyph.data.as_slice().into(),
            pixel_stride: bitmap_glyph.width as u16,
            sdf: self.bitmap_font.sdf,
        })
    }
}

impl GlyphRenderer for PixelFont {
    fn render_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        _slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableGlyph> {
        let glyph_index = Self::glyph_id_to_glyph_index(glyph_id);
        let bitmap_glyph = &self.glyphs.glyph_data[glyph_index];
        self.render_bitmap_glyph(bitmap_glyph)
    }
    fn scale_delta(&self) -> Fixed<u16, 8> {
        if self.pixel_size.get() == self.glyphs.pixel_size {
            return Fixed::from_integer(1);
        }
        Fixed::try_from_fixed(Fixed::<u32, 8>::from_fraction(
            self.glyphs.pixel_size as u32,
            self.pixel_size.get() as u32,
        ))
        .unwrap()
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
            let glyph_index = self.bitmap_font.character_map.binary_search(&char).ok();
            let x_advance = glyph_index.map_or_else(
                || self.pixel_size,
                |glyph_index| self.glyph_advance(self.glyphs.glyph_data[glyph_index].x_advance),
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
        self.bitmap_font.character_map.binary_search(&ch).ok().map(|glyph_index| {
            let bitmap_glyph = &self.glyphs.glyph_data[glyph_index];
            let x_advance = self.glyph_advance(bitmap_glyph.x_advance);
            Glyph {
                glyph_id: Some(Self::glyph_index_to_glyph_id(glyph_index)),
                advance: x_advance,
                text_byte_offset: 0,
                ..Default::default()
            }
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::graphics::BitmapGlyph;
    use i_slint_core::slice::Slice;

    static FONT: BitmapFont = BitmapFont {
        family_name: Slice::from_slice(b"Glyph arithmetic test"),
        character_map: Slice::from_slice(&[]),
        units_per_em: 1000.,
        ascent: 800.,
        descent: -200.,
        x_height: 500.,
        cap_height: 700.,
        glyphs: Slice::from_slice(&[]),
        weight: 400,
        italic: false,
        sdf: false,
    };
    static GLYPHS: BitmapGlyphs =
        BitmapGlyphs { pixel_size: 12, glyph_data: Slice::from_slice(&[]) };
    static COVERAGE: [u8; 1024] = [173; 1024];

    fn reference_bounds(font: &PixelFont, glyph: &BitmapGlyph) -> (i32, i32, i16, i16) {
        let delta = Fixed::<i32, 8>::from_fraction(
            font.glyphs.pixel_size as i32,
            font.pixel_size.get() as i32,
        );
        let src_x = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(glyph.x));
        let src_y = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(glyph.y));
        let h_plus_y = Fixed::<i32, 8>::from_integer(glyph.height as i32) + src_y;
        let h_plus_y = Fixed::<i32, 8>::from_fraction(h_plus_y.0, delta.0);
        let off_y = Fixed::<i32, 8>(h_plus_y.0 & 0xff);
        let height = (Fixed::from_integer(glyph.height as i32 - 1) - off_y) / delta + 1;
        let x = Fixed::<i32, 8>::from_fraction(src_x.0, delta.0);
        let off_x = Fixed::<i32, 8>(-x.0 & 0xff);
        let width = (Fixed::from_integer(glyph.width as i32 - 1) - off_x) / delta + 1;
        (x.0, (h_plus_y - Fixed::from_integer(height)).0, width as i16, height as i16)
    }

    #[test]
    fn exact_size_glyphs_keep_fractional_bearings_and_coverage() {
        let font =
            PixelFont { bitmap_font: &FONT, glyphs: &GLYPHS, pixel_size: PhysicalLength::new(12) };
        assert_eq!(font.scale_delta(), Fixed::from_integer(1));
        for (width, height) in [(1, 1), (2, 17), (23, 2), (32, 32)] {
            for x in -128..128 {
                for y in -128..128 {
                    let glyph = BitmapGlyph {
                        x,
                        y,
                        width,
                        height,
                        x_advance: 12 * 64,
                        data: Slice::from_slice(&COVERAGE[..width as usize * height as usize]),
                    };
                    let actual = font.render_bitmap_glyph(&glyph).unwrap();
                    assert_eq!(
                        (actual.x.0, actual.y.0, actual.width.get(), actual.height.get()),
                        reference_bounds(&font, &glyph),
                        "bearing {x}/{y}, size {width}/{height}"
                    );
                    let super::super::GlyphAlphaMap::Static(coverage) = actual.alpha_map else {
                        panic!("bitmap glyph coverage must remain borrowed");
                    };
                    assert_eq!(coverage, glyph.data.as_slice());
                    assert_eq!(actual.pixel_stride, width as u16);
                    assert!(!actual.sdf);
                }
            }
        }
    }

    #[test]
    fn glyph_advances_keep_scaled_rounding() {
        for pixel_size in [6, 9, 12, 15, 24] {
            let font = PixelFont {
                bitmap_font: &FONT,
                glyphs: &GLYPHS,
                pixel_size: PhysicalLength::new(pixel_size),
            };
            for advance in i16::MIN..=i16::MAX {
                let expected = ((pixel_size as i32 * advance as i32 / 12 + 32) / 64) as i16;
                assert_eq!(font.glyph_advance(advance).get(), expected);
            }
        }
    }

    #[test]
    fn scaled_and_empty_glyphs_keep_existing_behavior() {
        for pixel_size in [6, 9, 15, 24] {
            let font = PixelFont {
                bitmap_font: &FONT,
                glyphs: &GLYPHS,
                pixel_size: PhysicalLength::new(pixel_size),
            };
            for bearing in [-127, -64, -1, 0, 1, 63, 127] {
                let mut glyph = BitmapGlyph {
                    x: bearing,
                    y: bearing,
                    width: 23,
                    height: 17,
                    x_advance: 12 * 64,
                    data: Slice::from_slice(&COVERAGE[..23 * 17]),
                };
                let actual = font.render_bitmap_glyph(&glyph).unwrap();
                assert_eq!(
                    (actual.x.0, actual.y.0, actual.width.get(), actual.height.get()),
                    reference_bounds(&font, &glyph)
                );
                glyph.data = Slice::from_slice(&[]);
                assert!(font.render_bitmap_glyph(&glyph).is_none());
            }
        }
    }
}
