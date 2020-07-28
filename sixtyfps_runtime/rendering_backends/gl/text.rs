use super::texture::{AtlasAllocation, TextureAtlas};
use collections::hash_map::HashMap;
use sixtyfps_corelib::font::Font;
use std::{collections, rc::Rc};

pub struct PreRenderedGlyph {
    pub glyph_allocation: AtlasAllocation,
    pub advance: f32,
}

pub struct CachedFontGlyphs {
    pub font: Rc<Font>,
    glyphs: HashMap<u32, PreRenderedGlyph>,
}

impl CachedFontGlyphs {
    pub fn new(font: Rc<Font>) -> Self {
        let glyphs = HashMap::new();
        Self { font, glyphs }
    }

    pub fn string_to_glyphs(
        &mut self,
        gl: &glow::Context,
        atlas: &mut TextureAtlas,
        text: &str,
    ) -> Vec<u32> {
        self.font
            .clone()
            .string_to_glyphs(text)
            .into_iter()
            .inspect(|glyph| {
                if !self.glyphs.contains_key(&glyph) {
                    // ensure the glyph is cached
                    self.glyphs.insert(*glyph, self.render_glyph(gl, atlas, *glyph));
                }
            })
            .collect()
    }

    pub fn layout_glyphs<'a, I: std::iter::IntoIterator<Item = u32>>(
        &'a self,
        glyphs: I,
    ) -> GlyphIter<'a, I::IntoIter> {
        GlyphIter { gl_font: self, glyph_it: glyphs.into_iter() }
    }

    fn render_glyph(
        &self,
        gl: &glow::Context,
        atlas: &mut TextureAtlas,
        glyph_id: u32,
    ) -> PreRenderedGlyph {
        let advance = self.font.glyph_metrics(glyph_id).advance;
        let glyph_image = self.font.rasterize_glyph(glyph_id);

        let glyph_allocation = atlas.allocate_image_in_atlas(
            gl,
            image::ImageBuffer::<_, &[u8]>::from_raw(
                glyph_image.width(),
                glyph_image.height(),
                &glyph_image,
            )
            .unwrap(),
        );

        PreRenderedGlyph { glyph_allocation, advance }
    }
}

pub struct GlyphIter<'a, GlyphIterator> {
    gl_font: &'a CachedFontGlyphs,
    glyph_it: GlyphIterator,
}

impl<'a, GlyphIterator> Iterator for GlyphIter<'a, GlyphIterator>
where
    GlyphIterator: std::iter::Iterator<Item = u32>,
{
    type Item = &'a PreRenderedGlyph;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(glyph_id) = self.glyph_it.next() {
            Some(&self.gl_font.glyphs[&glyph_id])
        } else {
            None
        }
    }
}
