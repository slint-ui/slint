// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::PhysicalLength;
use crate::fixed::Fixed;
use alloc::rc::Rc;
use i_slint_core::graphics::{BitmapFont, SharedBitmapFont};
use i_slint_core::textlayout::{FontMetrics, Glyph, TextShaper};

use super::{GlyphAlphaMap, GlyphRenderer, RenderableGlyph};

/// A bitmap font that generated code keeps in a static, or one built at run time.
#[derive(Clone)]
pub enum BitmapFontRef {
    Static(&'static BitmapFont),
    Shared(Rc<SharedBitmapFont>),
}

/// Evaluates `$body` with `$font` bound to the [`BitmapFont`] or the [`SharedBitmapFont`],
/// whose fields have the same names.
macro_rules! with_bitmap_font {
    ($font:expr, |$f:ident| $body:expr) => {
        match $font {
            BitmapFontRef::Static($f) => $body,
            BitmapFontRef::Shared($f) => $body,
        }
    };
}

impl BitmapFontRef {
    pub fn family_name(&self) -> &str {
        match self {
            Self::Static(font) => core::str::from_utf8(font.family_name.as_slice()).unwrap(),
            Self::Shared(font) => font.family_name.as_str(),
        }
    }

    pub fn italic(&self) -> bool {
        with_bitmap_font!(self, |font| font.italic)
    }

    pub fn weight(&self) -> u16 {
        with_bitmap_font!(self, |font| font.weight)
    }

    pub fn sdf(&self) -> bool {
        with_bitmap_font!(self, |font| font.sdf)
    }

    /// The index of the glyph set with the largest pixel size up to `pixel_size`, or of the
    /// smallest one.
    pub fn glyphs_for_pixel_size(&self, pixel_size: PhysicalLength) -> usize {
        with_bitmap_font!(self, |font| font
            .glyphs
            .partition_point(|glyphs| PhysicalLength::new(glyphs.pixel_size) <= pixel_size)
            .saturating_sub(1))
    }

    pub fn glyphs_pixel_size(&self, glyphs: usize) -> PhysicalLength {
        PhysicalLength::new(with_bitmap_font!(self, |font| font.glyphs[glyphs].pixel_size))
    }
}

// A font that is resolved to a specific pixel size.
pub struct PixelFont {
    pub font: BitmapFontRef,
    /// The index of the glyph set in the font
    pub glyphs: usize,
    pub pixel_size: PhysicalLength,
}

impl PixelFont {
    pub fn glyph_index_to_glyph_id(index: usize) -> core::num::NonZeroU16 {
        core::num::NonZeroU16::new(index as u16 + 1).unwrap()
    }
    pub fn glyph_id_to_glyph_index(id: core::num::NonZeroU16) -> usize {
        id.get() as usize - 1
    }

    fn glyph_index(&self, ch: char) -> Option<usize> {
        with_bitmap_font!(&self.font, |font| font.character_map.binary_search(&ch).ok())
    }

    fn x_advance(&self, glyph_index: usize) -> PhysicalLength {
        let (x_advance, glyphs_pixel_size) = with_bitmap_font!(&self.font, |font| {
            let glyphs = &font.glyphs[self.glyphs];
            (glyphs.glyph_data[glyph_index].x_advance, glyphs.pixel_size)
        });
        ((self.pixel_size.cast() * x_advance as i32 / glyphs_pixel_size as i32
            + euclid::Length::new(32))
            / 64)
            .cast()
    }

    /// `metric`, in the design units of the font, at the pixel size.
    fn scaled(&self, metric: f32) -> PhysicalLength {
        let units_per_em = with_bitmap_font!(&self.font, |font| font.units_per_em);
        (self.pixel_size.cast() * metric / units_per_em).cast()
    }
}

impl GlyphRenderer for PixelFont {
    fn render_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        _slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableGlyph> {
        let glyph_index = Self::glyph_id_to_glyph_index(glyph_id);
        let (x, y, glyph_width, glyph_height, empty) = with_bitmap_font!(&self.font, |font| {
            let glyph = &font.glyphs[self.glyphs].glyph_data[glyph_index];
            (glyph.x, glyph.y, glyph.width, glyph.height, glyph.data.is_empty())
        });
        if empty {
            // For example, ' ' has no glyph data
            return None;
        }
        let alpha_map: GlyphAlphaMap = match &self.font {
            BitmapFontRef::Static(font) => {
                font.glyphs[self.glyphs].glyph_data[glyph_index].data.as_slice().into()
            }
            BitmapFontRef::Shared(font) => {
                font.glyphs[self.glyphs].glyph_data[glyph_index].data.clone().into()
            }
        };
        // t represent the target coordinate system, and s the source glyph coordinate system.
        // We want to align the glyph such that Δ(hₜ+yₜ)+offset = hₛ+yₛ
        // where hₜ is the integer height of the glyph in the target coordinate system
        // and offset is smaller than Δ
        // We also want that Δ(hₜ-1)+offset ≤ hₛ-1
        // Similar for x but that's easier since x is not subtracted from the width
        let delta = Fixed::<i32, 8>::from_fixed(self.scale_delta());
        let src_x = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(x));
        let src_y = Fixed::<i32, 8>::from_fixed(Fixed::<_, 6>(y));
        let h_plus_y = Fixed::<i32, 8>::from_integer(glyph_height as i32) + src_y;
        let h_plus_y = Fixed::<i32, 8>::from_fraction(h_plus_y.0, delta.0);
        let off_y = Fixed::<i32, 8>(h_plus_y.0 & 0xff);
        let height = (Fixed::from_integer(glyph_height as i32 - 1) - off_y) / delta + 1;
        let x = Fixed::from_fraction(src_x.0, delta.0);
        let off_x = Fixed::<i32, 8>(-x.0 & 0xff);
        let width = (Fixed::from_integer(glyph_width as i32 - 1) - off_x) / delta + 1;
        Some(RenderableGlyph {
            x,
            y: h_plus_y - Fixed::from_integer(height),
            width: PhysicalLength::new(width as i16),
            height: PhysicalLength::new(height as i16),
            alpha_map,
            pixel_stride: glyph_width as u16,
            sdf: self.font.sdf(),
        })
    }
    fn scale_delta(&self) -> Fixed<u16, 8> {
        Fixed::try_from_fixed(Fixed::<u32, 8>::from_fraction(
            self.font.glyphs_pixel_size(self.glyphs).get() as u32,
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

impl FontMetrics<PhysicalLength> for PixelFont {
    fn ascent(&self) -> PhysicalLength {
        self.scaled(with_bitmap_font!(&self.font, |font| font.ascent))
    }

    fn descent(&self) -> PhysicalLength {
        self.scaled(with_bitmap_font!(&self.font, |font| font.descent))
    }

    fn height(&self) -> PhysicalLength {
        // The descent is negative (relative to the baseline)
        self.scaled(with_bitmap_font!(&self.font, |font| font.ascent - font.descent))
    }

    fn x_height(&self) -> PhysicalLength {
        self.scaled(with_bitmap_font!(&self.font, |font| font.x_height))
    }

    fn cap_height(&self) -> PhysicalLength {
        self.scaled(with_bitmap_font!(&self.font, |font| font.cap_height))
    }
}
