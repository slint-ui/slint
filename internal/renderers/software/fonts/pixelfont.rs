// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::PhysicalLength;
use crate::fixed::Fixed;
use alloc::rc::Rc;
use core::ops::Deref;
use i_slint_core::graphics::{BitmapFont, BitmapGlyphs, OwnedStorage, StaticStorage, Storage};
use i_slint_core::textlayout::{FontMetrics, Glyph, TextShaper};

use super::{GlyphAlphaMap, GlyphRenderer, RenderableGlyph};

pub trait FontStorage: Storage {
    type Font: Deref<Target = BitmapFont<Self>> + Clone;
    fn alpha_map(font: &Self::Font, glyph_set: usize, glyph: usize) -> GlyphAlphaMap;
}

impl FontStorage for StaticStorage {
    type Font = &'static BitmapFont;
    fn alpha_map(font: &Self::Font, glyph_set: usize, glyph: usize) -> GlyphAlphaMap {
        GlyphAlphaMap::Static(&font.glyphs[glyph_set].glyph_data[glyph].data)
    }
}

impl FontStorage for OwnedStorage {
    type Font = Rc<BitmapFont<OwnedStorage>>;
    fn alpha_map(font: &Self::Font, glyph_set: usize, glyph: usize) -> GlyphAlphaMap {
        GlyphAlphaMap::Shared {
            font: font.clone(),
            glyph_set: glyph_set as u16,
            glyph: glyph as u16,
        }
    }
}

/// A bitmap font that is resolved to a specific pixel size.
pub struct PixelFont<S: FontStorage = StaticStorage> {
    font: S::Font,
    glyph_set_index: usize,
    pixel_size: PhysicalLength,
}

impl<S: FontStorage> PixelFont<S> {
    /// `font` at `pixel_size`, from the glyph set with the largest pixel size up to it, or
    /// the smallest one.
    pub fn new(font: S::Font, pixel_size: PhysicalLength) -> Self {
        let glyph_set_index = font
            .glyphs
            .partition_point(|glyphs| PhysicalLength::new(glyphs.pixel_size) <= pixel_size)
            .saturating_sub(1);
        let pixel_size = if font.sdf {
            pixel_size
        } else {
            PhysicalLength::new(font.glyphs[glyph_set_index].pixel_size)
        };
        Self { font, glyph_set_index, pixel_size }
    }

    pub fn glyph_index_to_glyph_id(index: usize) -> core::num::NonZeroU16 {
        core::num::NonZeroU16::new(index as u16 + 1).unwrap()
    }
    pub fn glyph_id_to_glyph_index(id: core::num::NonZeroU16) -> usize {
        id.get() as usize - 1
    }

    fn glyph_set(&self) -> &BitmapGlyphs<S> {
        &self.font.glyphs[self.glyph_set_index]
    }

    fn glyph_index(&self, ch: char) -> Option<usize> {
        self.font.character_map.binary_search(&ch).ok()
    }

    fn x_advance(&self, glyph_index: usize) -> PhysicalLength {
        let glyphs = self.glyph_set();
        let x_advance = glyphs.glyph_data[glyph_index].x_advance;
        ((self.pixel_size.cast() * x_advance as i32 / glyphs.pixel_size as i32
            + euclid::Length::new(32))
            / 64)
            .cast()
    }

    /// `metric`, in the design units of the font, at the pixel size.
    fn scaled(&self, metric: f32) -> PhysicalLength {
        (self.pixel_size.cast() * metric / self.font.units_per_em).cast()
    }
}

impl<S: FontStorage> GlyphRenderer for PixelFont<S> {
    fn render_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        _slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableGlyph> {
        let glyph_index = Self::glyph_id_to_glyph_index(glyph_id);
        let glyph = &self.glyph_set().glyph_data[glyph_index];
        if glyph.data.is_empty() {
            // For example, ' ' has no glyph data
            return None;
        }
        // t represent the target coordinate system, and s the source glyph coordinate system.
        // We want to align the glyph such that Δ(hₜ+yₜ)+offset = hₛ+yₛ
        // where hₜ is the integer height of the glyph in the target coordinate system
        // and offset is smaller than Δ
        // We also want that Δ(hₜ-1)+offset ≤ hₛ-1
        // Similar for x but that's easier since x is not subtracted from the width
        let delta = Fixed::<i32, 8>::from_fixed(self.scale_delta());
        let src_x = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(glyph.x));
        let src_y = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(glyph.y));
        let h_plus_y = Fixed::<i32, 8>::from_integer(glyph.height as i32) + src_y;
        let h_plus_y = Fixed::<i32, 8>::from_fraction(h_plus_y.0, delta.0);
        let off_y = Fixed::<i32, 8>(h_plus_y.0 & 0xff);
        let height = (Fixed::from_integer(glyph.height as i32 - 1) - off_y) / delta + 1;
        let x = Fixed::from_fraction(src_x.0, delta.0);
        let off_x = Fixed::<i32, 8>(-x.0 & 0xff);
        let width = (Fixed::from_integer(glyph.width as i32 - 1) - off_x) / delta + 1;
        Some(RenderableGlyph {
            x,
            y: h_plus_y - Fixed::from_integer(height),
            width: PhysicalLength::new(width as i16),
            height: PhysicalLength::new(height as i16),
            alpha_map: S::alpha_map(&self.font, self.glyph_set_index, glyph_index),
            pixel_stride: glyph.width as u16,
            sdf: self.font.sdf,
        })
    }
    fn scale_delta(&self) -> Fixed<u16, 8> {
        Fixed::try_from_fixed(Fixed::<u32, 8>::from_fraction(
            self.glyph_set().pixel_size as u32,
            self.pixel_size.get() as u32,
        ))
        .unwrap()
    }
}

impl<S: FontStorage> TextShaper for PixelFont<S> {
    type LengthPrimitive = i16;
    type Length = PhysicalLength;
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<PhysicalLength>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let glyphs_iter = text.char_indices().map(|(byte_offset, char)| {
            let glyph_index = self.glyph_index(char);
            let x_advance = glyph_index
                .map_or_else(|| self.pixel_size, |glyph_index| self.x_advance(glyph_index));
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
        self.glyph_index(ch).map(|glyph_index| Glyph {
            glyph_id: Some(Self::glyph_index_to_glyph_id(glyph_index)),
            advance: self.x_advance(glyph_index),
            text_byte_offset: 0,
            ..Default::default()
        })
    }
}

impl<S: FontStorage> FontMetrics<PhysicalLength> for PixelFont<S> {
    fn ascent(&self) -> PhysicalLength {
        self.scaled(self.font.ascent)
    }

    fn descent(&self) -> PhysicalLength {
        self.scaled(self.font.descent)
    }

    fn height(&self) -> PhysicalLength {
        // The descent is negative (relative to the baseline)
        self.scaled(self.font.ascent - self.font.descent)
    }

    fn x_height(&self) -> PhysicalLength {
        self.scaled(self.font.x_height)
    }

    fn cap_height(&self) -> PhysicalLength {
        self.scaled(self.font.cap_height)
    }
}
