use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone)]
struct GlyphMetrics {
    advance: f32,
}

pub struct Font {
    pub pixel_size: f32,
    font: font_kit::font::Font,
    metrics: font_kit::metrics::Metrics,
    glyph_metrics_cache: RefCell<HashMap<u32, GlyphMetrics>>,
}

impl Font {
    pub fn string_to_glyphs<'a>(&'a self, text: &'a str) -> impl Iterator<Item = u32> + 'a {
        text.chars().map(move |ch| self.font.glyph_for_char(ch).unwrap())
    }

    pub fn text_width(&self, text: &str) -> f32 {
        self.string_to_glyphs(text)
            .map(|glyph| self.glyph_metrics(glyph))
            .fold(0., |width, glyph| width + glyph.advance)
    }

    pub fn font_height(&self) -> f32 {
        let scale_from_font_units = self.pixel_size / self.metrics.units_per_em as f32;

        (self.metrics.ascent - self.metrics.descent + 1.) * scale_from_font_units
    }

    fn glyph_metrics(&self, glyph: u32) -> GlyphMetrics {
        self.glyph_metrics_cache
            .borrow_mut()
            .entry(glyph)
            .or_insert_with(|| {
                let scale_from_font_units = self.pixel_size / self.metrics.units_per_em as f32;

                let advance = self.font.advance(glyph).unwrap().x() * scale_from_font_units;
                GlyphMetrics { advance }
            })
            .clone()
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
