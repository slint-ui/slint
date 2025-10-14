// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::num::NonZeroU16;

use alloc::rc::Rc;
use skrifa::MetadataProvider;

use crate::lengths::PhysicalPx;
use crate::software_renderer::fixed::Fixed;
use crate::software_renderer::PhysicalLength;
use crate::textlayout::{Glyph, TextShaper};
use i_slint_common::sharedfontique::fontique;

use super::RenderableVectorGlyph;

// A length in font design space.
struct FontUnit;
type FontLength = euclid::Length<i32, FontUnit>;
type FontScaleFactor = euclid::Scale<f32, FontUnit, PhysicalPx>;

type GlyphCacheKey = (u64, u32, PhysicalLength, core::num::NonZeroU16);

struct RenderableGlyphWeightScale;

impl clru::WeightScale<GlyphCacheKey, RenderableVectorGlyph> for RenderableGlyphWeightScale {
    fn weight(&self, _: &GlyphCacheKey, value: &RenderableVectorGlyph) -> usize {
        value.alpha_map.len()
    }
}

type GlyphCache = clru::CLruCache<
    GlyphCacheKey,
    RenderableVectorGlyph,
    std::collections::hash_map::RandomState,
    RenderableGlyphWeightScale,
>;

crate::thread_local!(static GLYPH_CACHE: core::cell::RefCell<GlyphCache>  =
    core::cell::RefCell::new(
        clru::CLruCache::with_config(
            clru::CLruCacheConfig::new(core::num::NonZeroUsize::new(1024 * 1024).unwrap())
                .with_scale(RenderableGlyphWeightScale)
        )
    )
);

pub struct VectorFont {
    font_index: u32,
    font_blob: fontique::Blob<u8>,
    fontdue_font: Rc<fontdue::Font>,
    ascender: PhysicalLength,
    descender: PhysicalLength,
    height: PhysicalLength,
    pixel_size: PhysicalLength,
    x_height: PhysicalLength,
    cap_height: PhysicalLength,
}

impl VectorFont {
    pub fn new(
        font: fontique::QueryFont,
        fontdue_font: Rc<fontdue::Font>,
        pixel_size: PhysicalLength,
    ) -> Self {
        Self::new_from_blob_and_index(font.blob, font.index, fontdue_font, pixel_size)
    }

    pub fn new_from_blob_and_index(
        font_blob: fontique::Blob<u8>,
        font_index: u32,
        fontdue_font: Rc<fontdue::Font>,
        pixel_size: PhysicalLength,
    ) -> Self {
        let face = skrifa::FontRef::from_index(font_blob.data(), font_index).unwrap();

        let metrics = face
            .metrics(skrifa::instance::Size::unscaled(), skrifa::instance::LocationRef::new(&[]));

        let ascender = FontLength::new(metrics.ascent as _);
        let descender = FontLength::new(metrics.descent as _);
        let height = FontLength::new((metrics.ascent - metrics.descent) as _);
        let x_height = FontLength::new(metrics.x_height.unwrap_or_default() as _);
        let cap_height = FontLength::new(metrics.cap_height.unwrap_or_default() as _);
        let units_per_em = metrics.units_per_em;
        let scale = FontScaleFactor::new(pixel_size.get() as f32 / units_per_em as f32);
        Self {
            font_index,
            font_blob,
            fontdue_font,
            ascender: (ascender.cast() * scale).cast(),
            descender: (descender.cast() * scale).cast(),
            height: (height.cast() * scale).cast(),
            pixel_size,
            x_height: (x_height.cast() * scale).cast(),
            cap_height: (cap_height.cast() * scale).cast(),
        }
    }

    pub fn render_vector_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
    ) -> Option<RenderableVectorGlyph> {
        GLYPH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();

            let cache_key = (self.font_blob.id(), self.font_index, self.pixel_size, glyph_id);

            if let Some(entry) = cache.get(&cache_key) {
                Some(entry.clone())
            } else {
                let (metrics, alpha_map) =
                    self.fontdue_font.rasterize_indexed(glyph_id.get(), self.pixel_size.get() as _);

                let alpha_map: Rc<[u8]> = alpha_map.into();

                let glyph = super::RenderableVectorGlyph {
                    x: Fixed::from_integer(metrics.xmin.try_into().unwrap()),
                    y: Fixed::from_integer(metrics.ymin.try_into().unwrap()),
                    width: PhysicalLength::new(metrics.width.try_into().unwrap()),
                    height: PhysicalLength::new(metrics.height.try_into().unwrap()),
                    alpha_map,
                    pixel_stride: metrics.width.try_into().unwrap(),
                    bounds: metrics.bounds,
                };

                cache.put_with_weight(cache_key, glyph.clone()).ok();
                Some(glyph)
            }
        })
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
        glyphs.extend(text.char_indices().map(|(byte_offset, char)| {
            let glyph_id = NonZeroU16::try_from(self.fontdue_font.lookup_glyph_index(char)).ok();
            let x_advance = glyph_id.map_or_else(
                || self.pixel_size.get(),
                |id| {
                    self.fontdue_font
                        .metrics_indexed(id.get(), self.pixel_size.get() as _)
                        .advance_width as _
                },
            );

            Glyph {
                glyph_id,
                advance: PhysicalLength::new(x_advance),
                text_byte_offset: byte_offset,
                ..Default::default()
            }
        }));
    }

    fn glyph_for_char(&self, ch: char) -> Option<Glyph<PhysicalLength>> {
        NonZeroU16::try_from(self.fontdue_font.lookup_glyph_index(ch)).ok().map(|glyph_id| {
            let mut out_glyph = Glyph::default();
            out_glyph.glyph_id = Some(glyph_id);
            out_glyph.advance = PhysicalLength::new(
                self.fontdue_font
                    .metrics_indexed(glyph_id.get(), self.pixel_size.get() as _)
                    .advance_width as _,
            );
            out_glyph
        })
    }

    fn max_lines(&self, max_height: PhysicalLength) -> usize {
        (max_height / self.height).get() as _
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

    fn x_height(&self) -> PhysicalLength {
        self.x_height
    }

    fn cap_height(&self) -> PhysicalLength {
        self.cap_height
    }
}

impl super::GlyphRenderer for VectorFont {
    fn render_glyph(&self, glyph_id: core::num::NonZeroU16) -> Option<super::RenderableGlyph> {
        self.render_vector_glyph(glyph_id).map(|glyph| super::RenderableGlyph {
            x: glyph.x,
            y: glyph.y,
            width: glyph.width,
            height: glyph.height,
            alpha_map: glyph.alpha_map.into(),
            pixel_stride: glyph.pixel_stride,
            sdf: false,
        })
    }

    fn scale_delta(&self) -> super::Fixed<u16, 8> {
        super::Fixed::from_integer(1)
    }
}
