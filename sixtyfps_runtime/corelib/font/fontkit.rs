use image::{ImageBuffer, Pixel, Rgba};
use pathfinder_geometry::{
    transform2d::Transform2F,
    vector::{Vector2F, Vector2I},
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone)]
pub struct GlyphMetrics {
    pub advance: f32,
}

pub struct Font {
    pub pixel_size: f32,
    font: font_kit::font::Font,
    metrics: font_kit::metrics::Metrics,
    glyph_metrics_cache: RefCell<HashMap<u32, GlyphMetrics>>,
}

impl Font {
    pub fn string_to_glyphs<'a>(&'a self, text: &'a str) -> impl Iterator<Item = u32> + 'a {
        text.chars().map(move |ch| {
            self.font
                .glyph_for_char(ch)
                .unwrap_or_else(|| self.font.glyph_for_char('\u{FFFD}').unwrap())
        })
    }

    pub fn text_width(&self, text: &str) -> f32 {
        self.string_to_glyphs(text)
            .map(|glyph| self.glyph_metrics(glyph))
            .fold(0., |width, glyph| width + glyph.advance)
    }

    pub fn font_height(&self) -> f32 {
        (self.metrics.ascent - self.metrics.descent + 1.) * self.font_units_to_pixel_size()
    }

    pub fn glyph_metrics(&self, glyph: u32) -> GlyphMetrics {
        self.glyph_metrics_cache
            .borrow_mut()
            .entry(glyph)
            .or_insert_with(|| {
                let advance =
                    self.font.advance(glyph).unwrap().x() * self.font_units_to_pixel_size();
                GlyphMetrics { advance }
            })
            .clone()
    }

    #[inline]
    fn font_units_to_pixel_size(&self) -> f32 {
        self.pixel_size / self.metrics.units_per_em as f32
    }

    pub fn ascent(&self) -> f32 {
        self.metrics.ascent * self.font_units_to_pixel_size()
    }

    pub fn descent(&self) -> f32 {
        self.metrics.descent * self.font_units_to_pixel_size()
    }

    pub fn height(&self) -> f32 {
        (self.metrics.ascent - self.metrics.descent + 1.) * self.font_units_to_pixel_size()
    }

    pub fn rasterize_glyph(&self, glyph_id: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let baseline_y = self.ascent();
        let hinting = font_kit::hinting::HintingOptions::None;
        let raster_opts = font_kit::canvas::RasterizationOptions::GrayscaleAa;

        // ### TODO: #8 use tight bounding box for glyphs stored in texture atlas
        let glyph_height = self.height();
        let glyph_width = self.glyph_metrics(glyph_id).advance;
        let mut canvas = font_kit::canvas::Canvas::new(
            Vector2I::new(glyph_width.ceil() as i32, glyph_height.ceil() as i32),
            font_kit::canvas::Format::A8,
        );
        self.font
            .rasterize_glyph(
                &mut canvas,
                glyph_id,
                self.pixel_size,
                Transform2F::from_translation(Vector2F::new(0., baseline_y)),
                hinting,
                raster_opts,
            )
            .unwrap();

        image::ImageBuffer::from_fn(canvas.size.x() as u32, canvas.size.y() as u32, |x, y| {
            let idx = (x as usize) + (y as usize) * canvas.stride;
            let alpha = canvas.pixels[idx];
            image::Rgba::<u8>::from_channels(0, 0, 0, alpha)
        })
    }

    pub fn handle(&self) -> FontHandle {
        FontHandle(self.font.handle().unwrap())
    }
}

#[derive(Clone)]
pub struct FontHandle(font_kit::handle::Handle);

impl Hash for FontHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.0 {
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

impl PartialEq for FontHandle {
    fn eq(&self, other: &Self) -> bool {
        match &self.0 {
            font_kit::handle::Handle::Path { path, font_index } => match &other.0 {
                font_kit::handle::Handle::Path {
                    path: other_path,
                    font_index: other_font_index,
                } => path.eq(other_path) && font_index.eq(other_font_index),
                _ => false,
            },
            font_kit::handle::Handle::Memory { bytes, font_index } => match &other.0 {
                font_kit::handle::Handle::Memory {
                    bytes: other_bytes,
                    font_index: other_font_index,
                } => bytes.eq(other_bytes) && font_index.eq(other_font_index),
                _ => false,
            },
        }
    }
}

impl Eq for FontHandle {}

impl FontHandle {
    pub fn load(&self, pixel_size: f32) -> Result<Font, font_kit::error::FontLoadingError> {
        let font = self.0.load()?;
        let metrics = font.metrics();
        Ok(Font { pixel_size, font, metrics, glyph_metrics_cache: Default::default() })
    }

    pub fn new_from_match(family: &str) -> Self {
        let family_name = if family.len() == 0 {
            font_kit::family_name::FamilyName::SansSerif
        } else {
            font_kit::family_name::FamilyName::Title(family.into())
        };

        font_kit::source::SystemSource::new()
            .select_best_match(
                &[family_name, font_kit::family_name::FamilyName::SansSerif],
                &font_kit::properties::Properties::new(),
            )
            .unwrap()
            .into()
    }
}

impl From<font_kit::handle::Handle> for FontHandle {
    fn from(h: font_kit::handle::Handle) -> Self {
        Self(h)
    }
}
