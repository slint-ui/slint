/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use image::{ImageBuffer, Pixel, Rgba};
use pathfinder_geometry::{
    transform2d::Transform2F,
    vector::{Vector2F, Vector2I},
};
use std::collections::HashMap;
use std::hash::Hash;
use std::{cell::RefCell, rc::Rc};

use super::FontRequest;

#[derive(Clone)]
pub struct GlyphMetrics {
    pub advance: f32,
}

pub struct Font {
    pub pixel_size: f32,
    handle: Rc<PlatformFont>,
    glyph_metrics_cache: RefCell<HashMap<u32, GlyphMetrics>>,
}

impl Font {
    pub fn string_to_glyphs<'a>(
        &'a self,
        text: &'a str,
    ) -> impl Iterator<Item = (usize, char, u32)> + 'a {
        text.char_indices().map(move |(offset, ch)| {
            (
                offset,
                ch,
                self.handle.font.glyph_for_char(ch).unwrap_or_else(|| {
                    self.handle
                        .font
                        .glyph_for_char('\u{FFFD}')
                        .unwrap_or_else(|| self.handle.font.glyph_for_char('?').unwrap())
                }),
            )
        })
    }

    pub fn text_width(&self, text: &str) -> f32 {
        self.string_to_glyphs(text)
            .map(|(_, _, glyph)| self.glyph_metrics(glyph))
            .fold(0., |width, glyph| width + glyph.advance)
    }

    pub fn text_offset_for_x_position<'a>(&self, text: &'a str, x: f32) -> usize {
        let mut current_x = 0.;
        // This assumes a 1:1 mapping between glyphs and characters right now -- this is wrong.
        for (offset, _, glyph_id) in self.string_to_glyphs(text) {
            let metrics = self.glyph_metrics(glyph_id);

            if current_x + metrics.advance / 2. >= x {
                return offset;
            }
            current_x += metrics.advance;
        }

        text.len()
    }

    pub fn glyph_metrics(&self, glyph: u32) -> GlyphMetrics {
        self.glyph_metrics_cache
            .borrow_mut()
            .entry(glyph)
            .or_insert_with(|| {
                let advance =
                    self.handle.font.advance(glyph).unwrap().x() * self.font_units_to_pixel_size();
                GlyphMetrics { advance }
            })
            .clone()
    }

    #[inline]
    fn font_units_to_pixel_size(&self) -> f32 {
        self.pixel_size / self.handle.metrics.units_per_em as f32
    }

    pub fn ascent(&self) -> f32 {
        self.handle.metrics.ascent * self.font_units_to_pixel_size()
    }

    pub fn descent(&self) -> f32 {
        self.handle.metrics.descent * self.font_units_to_pixel_size()
    }

    pub fn height(&self) -> f32 {
        (self.handle.metrics.ascent - self.handle.metrics.descent + 1.)
            * self.font_units_to_pixel_size()
    }

    pub fn rasterize_glyph(&self, glyph_id: u32) -> (f32, f32, ImageBuffer<Rgba<u8>, Vec<u8>>) {
        let hinting = font_kit::hinting::HintingOptions::None;
        let raster_opts = font_kit::canvas::RasterizationOptions::GrayscaleAa;

        let glyph_rect = self
            .handle
            .font
            .raster_bounds(glyph_id, self.pixel_size, Transform2F::default(), hinting, raster_opts)
            .unwrap();

        // With CoreText we oddly need an extra pixel on each side.
        let glyph_width = glyph_rect.width() + 2;
        let glyph_height = glyph_rect.height() + 2;

        let x = glyph_rect.origin_x() as f32;
        let y = glyph_rect.origin_y() as f32;
        let mut canvas = font_kit::canvas::Canvas::new(
            Vector2I::new(glyph_width, glyph_height),
            font_kit::canvas::Format::A8,
        );
        self.handle
            .font
            .rasterize_glyph(
                &mut canvas,
                glyph_id,
                self.pixel_size,
                Transform2F::from_translation(Vector2F::new(-x + 1., -y + 1.)),
                hinting,
                raster_opts,
            )
            .unwrap();

        (
            x,
            y,
            image::ImageBuffer::from_fn(canvas.size.x() as u32, canvas.size.y() as u32, |x, y| {
                let idx = (x as usize) + (y as usize) * canvas.stride;
                let alpha = canvas.pixels[idx];
                image::Rgba::<u8>::from_channels(0, 0, 0, alpha)
            }),
        )
    }

    pub fn handle(&self) -> Rc<PlatformFont> {
        self.handle.clone()
    }
}

pub struct PlatformFont {
    handle: font_kit::handle::Handle,
    font: font_kit::font::Font,
    metrics: font_kit::metrics::Metrics,
}

impl Hash for PlatformFont {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.handle {
            font_kit::handle::Handle::Path { path, font_index } => {
                path.hash(state);
                font_index.hash(state);
            }
            font_kit::handle::Handle::Memory { bytes, font_index } => {
                bytes.hash(state);
                font_index.hash(state);
            }
        }
    }
}

impl PartialEq for PlatformFont {
    fn eq(&self, other: &Self) -> bool {
        match &self.handle {
            font_kit::handle::Handle::Path { path, font_index } => match &other.handle {
                font_kit::handle::Handle::Path {
                    path: other_path,
                    font_index: other_font_index,
                } => path.eq(other_path) && font_index.eq(other_font_index),
                _ => false,
            },
            font_kit::handle::Handle::Memory { bytes, font_index } => match &other.handle {
                font_kit::handle::Handle::Memory {
                    bytes: other_bytes,
                    font_index: other_font_index,
                } => bytes.eq(other_bytes) && font_index.eq(other_font_index),
                _ => false,
            },
        }
    }
}

impl Eq for PlatformFont {}

impl PlatformFont {
    pub fn load(self: &Rc<Self>, pixel_size: f32) -> Font {
        Font { pixel_size, handle: self.clone(), glyph_metrics_cache: Default::default() }
    }

    pub fn new_from_request(
        request: &FontRequest,
    ) -> Result<Rc<Self>, font_kit::error::FontLoadingError> {
        let family_name = if request.family.len() == 0 {
            font_kit::family_name::FamilyName::SansSerif
        } else {
            font_kit::family_name::FamilyName::Title(request.family.to_string())
        };

        let handle = font_kit::source::SystemSource::new()
            .select_best_match(
                &[family_name, font_kit::family_name::FamilyName::SansSerif],
                &font_kit::properties::Properties::new()
                    .weight(font_kit::properties::Weight(request.weight as f32)),
            )
            .unwrap();

        let font = handle.load()?;
        let metrics = font.metrics();

        Ok(Rc::new(PlatformFont { handle, font, metrics }))
    }
}
