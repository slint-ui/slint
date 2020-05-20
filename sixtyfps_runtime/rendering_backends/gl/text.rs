use super::texture::{AtlasAllocation, TextureAtlas};
use image::Pixel;
use pathfinder_geometry::{
    transform2d::Transform2F,
    vector::{Vector2F, Vector2I},
};

pub struct PreRenderedGlyph {
    pub glyph_allocation: AtlasAllocation,
    pub advance: f32,
}

pub struct GLFont {
    font: font_kit::font::Font,
    glyphs: std::collections::hash_map::HashMap<u32, PreRenderedGlyph>,
}

impl Default for GLFont {
    fn default() -> Self {
        let font = font_kit::source::SystemSource::new()
            .select_best_match(
                &[font_kit::family_name::FamilyName::SansSerif],
                &font_kit::properties::Properties::new(),
            )
            .unwrap()
            .load()
            .unwrap();
        let glyphs = std::collections::hash_map::HashMap::new();
        Self { font, glyphs }
    }
}

impl GLFont {
    pub fn layout_glyphs<'a>(
        &'a mut self,
        gl: &glow::Context,
        atlas: &mut TextureAtlas,
        text: &'a str,
    ) -> GlyphIter<'a> {
        let pixel_size: f32 = 48.0 * 72. / 96.;

        let font_metrics = self.font.metrics();

        let scale_from_font_units = pixel_size / font_metrics.units_per_em as f32;

        let baseline_y = font_metrics.ascent * scale_from_font_units;
        let hinting = font_kit::hinting::HintingOptions::None;
        let raster_opts = font_kit::canvas::RasterizationOptions::GrayscaleAa;

        text.chars().for_each(|ch| {
            let glyph_id = self.font.glyph_for_char(ch).unwrap();
            if self.glyphs.contains_key(&glyph_id) {
                return;
            }

            let advance = self.font.advance(glyph_id).unwrap().x() * scale_from_font_units;

            // ### TODO: #8 use tight bounding box for glyphs stored in texture atlas
            let glyph_height =
                (font_metrics.ascent - font_metrics.descent + 1.) * scale_from_font_units;
            let glyph_width = advance;
            let mut canvas = font_kit::canvas::Canvas::new(
                Vector2I::new(glyph_width.ceil() as i32, glyph_height.ceil() as i32),
                font_kit::canvas::Format::A8,
            );
            self.font
                .rasterize_glyph(
                    &mut canvas,
                    glyph_id,
                    pixel_size,
                    Transform2F::from_translation(Vector2F::new(0., baseline_y)),
                    hinting,
                    raster_opts,
                )
                .unwrap();

            let glyph_image = image::ImageBuffer::from_fn(
                canvas.size.x() as u32,
                canvas.size.y() as u32,
                |x, y| {
                    let idx = (x as usize) + (y as usize) * canvas.stride;
                    let alpha = canvas.pixels[idx];
                    image::Rgba::<u8>::from_channels(0, 0, 0, alpha)
                },
            );

            let glyph_allocation = atlas.allocate_image_in_atlas(gl, glyph_image);

            let glyph = PreRenderedGlyph { glyph_allocation, advance };

            self.glyphs.insert(glyph_id, glyph);
        });

        GlyphIter { gl_font: self, char_it: text.chars() }
    }
}

pub struct GlyphIter<'a> {
    gl_font: &'a GLFont,
    char_it: std::str::Chars<'a>,
}

impl<'a> Iterator for GlyphIter<'a> {
    type Item = &'a PreRenderedGlyph;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ch) = self.char_it.next() {
            let glyph_id = self.gl_font.font.glyph_for_char(ch).unwrap();
            let glyph = &self.gl_font.glyphs[&glyph_id];
            Some(glyph)
        } else {
            None
        }
    }
}
