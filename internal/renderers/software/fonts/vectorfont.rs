// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::num::NonZeroU16;

use alloc::rc::Rc;
use skrifa::MetadataProvider;

use crate::PhysicalLength;
use crate::fixed::Fixed;
use i_slint_common::sharedfontique::fontique;
use i_slint_core::lengths::PhysicalPx;
use i_slint_core::textlayout::{Glyph, TextShaper};

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

i_slint_core::thread_local!(static GLYPH_CACHE: core::cell::RefCell<GlyphCache>  =
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
    swash_key: swash::CacheKey,
    swash_offset: u32,
    ascender: PhysicalLength,
    descender: PhysicalLength,
    height: PhysicalLength,
    pixel_size: PhysicalLength,
    x_height: PhysicalLength,
    cap_height: PhysicalLength,
}

impl VectorFont {
    fn swash_font_ref(&self) -> swash::FontRef<'_> {
        swash::FontRef {
            data: self.font_blob.data(),
            offset: self.swash_offset,
            key: self.swash_key,
        }
    }

    pub fn new(
        font: fontique::QueryFont,
        swash_key: swash::CacheKey,
        swash_offset: u32,
        pixel_size: PhysicalLength,
    ) -> Self {
        Self::new_from_blob_and_index(font.blob, font.index, swash_key, swash_offset, pixel_size)
    }

    pub fn new_from_blob_and_index(
        font_blob: fontique::Blob<u8>,
        font_index: u32,
        swash_key: swash::CacheKey,
        swash_offset: u32,
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
            swash_key,
            swash_offset,
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
        slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableVectorGlyph> {
        GLYPH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();

            let cache_key = (self.font_blob.id(), self.font_index, self.pixel_size, glyph_id);

            if let Some(entry) = cache.get(&cache_key) {
                return Some(entry.clone());
            }

            let glyph = {
                let font_ref = self.swash_font_ref();
                let mut ctx = slint_context.swash_scale_context().borrow_mut();
                let mut scaler = ctx.builder(font_ref).size(self.pixel_size.get() as f32).build();
                let image = swash::scale::Render::new(&[swash::scale::Source::Outline])
                    .format(swash::zeno::Format::Alpha)
                    .render(&mut scaler, glyph_id.get())?;

                let placement = image.placement;
                let alpha_map: Rc<[u8]> = image.data.into();

                Some(RenderableVectorGlyph {
                    x: Fixed::from_integer(placement.left),
                    y: Fixed::from_integer(placement.top - placement.height as i32),
                    width: PhysicalLength::new(placement.width.try_into().unwrap()),
                    height: PhysicalLength::new(placement.height.try_into().unwrap()),
                    alpha_map,
                    pixel_stride: placement.width.try_into().unwrap(),
                    glyph_origin_x: placement.left as f32,
                })
            };

            if let Some(ref glyph) = glyph {
                cache.put_with_weight(cache_key, glyph.clone()).ok();
            }
            glyph
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
        let font_ref = self.swash_font_ref();
        let charmap = font_ref.charmap();
        let gm = font_ref.glyph_metrics(&[]);
        let metrics = font_ref.metrics(&[]);
        let scale = self.pixel_size.get() as f32 / metrics.units_per_em as f32;

        glyphs.extend(text.char_indices().map(|(byte_offset, char)| {
            let glyph_id = NonZeroU16::try_from(charmap.map(char)).ok();
            let x_advance = glyph_id.map_or_else(
                || self.pixel_size.get(),
                |id| (gm.advance_width(id.get()) * scale) as _,
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
        let font_ref = self.swash_font_ref();
        let charmap = font_ref.charmap();
        let gm = font_ref.glyph_metrics(&[]);
        let metrics = font_ref.metrics(&[]);
        let scale = self.pixel_size.get() as f32 / metrics.units_per_em as f32;

        NonZeroU16::try_from(charmap.map(ch)).ok().map(|glyph_id| Glyph {
            glyph_id: Some(glyph_id),
            advance: PhysicalLength::new((gm.advance_width(glyph_id.get()) * scale) as _),
            ..Default::default()
        })
    }

    fn max_lines(&self, max_height: PhysicalLength) -> usize {
        (max_height / self.height).get() as _
    }
}

impl i_slint_core::textlayout::FontMetrics<PhysicalLength> for VectorFont {
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
    fn render_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        slint_context: &i_slint_core::SlintContext,
    ) -> Option<super::RenderableGlyph> {
        self.render_vector_glyph(glyph_id, slint_context).map(|glyph| super::RenderableGlyph {
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
