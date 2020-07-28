use super::texture::{AtlasAllocation, TextureAtlas};
use collections::hash_map::HashMap;
use sixtyfps_corelib::font::Font;
use std::cell::RefCell;
use std::{collections, rc::Rc};

use sixtyfps_corelib::font::FontHandle;

type GlyphsByPixelSize = Vec<Rc<RefCell<CachedFontGlyphs>>>;

#[derive(Default)]
pub(crate) struct GlyphCache {
    glyphs_by_font: HashMap<FontHandle, GlyphsByPixelSize>,
}

impl GlyphCache {
    pub fn find_font(
        &mut self,
        font_family: &str,
        pixel_size: f32,
    ) -> Rc<RefCell<CachedFontGlyphs>> {
        let font =
            sixtyfps_corelib::font::FONT_CACHE.with(|fc| fc.find_font(font_family, pixel_size));

        let font_handle = font.handle();

        let glyphs_by_pixel_size =
            self.glyphs_by_font.entry(font_handle.clone()).or_insert(GlyphsByPixelSize::default());

        glyphs_by_pixel_size
            .iter()
            .find_map(|gl_font| {
                if gl_font.borrow().font.pixel_size == font.pixel_size {
                    Some(gl_font.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                let fnt = Rc::new(RefCell::new(CachedFontGlyphs::new(font.clone())));
                glyphs_by_pixel_size.push(fnt.clone());
                fnt
            })
    }
}

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

    pub fn layout_glyphs<'a>(
        &'a mut self,
        gl: &'a glow::Context,
        atlas: &'a mut TextureAtlas,
        text: &'a str,
    ) -> impl Iterator<Item = &PreRenderedGlyph> + 'a {
        let glyphs =
            self.font.clone().string_to_glyphs(text).collect::<smallvec::SmallVec<[_; 32]>>();

        glyphs.iter().for_each(|glyph| {
            if !self.glyphs.contains_key(&glyph) {
                // ensure the glyph is cached
                self.glyphs.insert(*glyph, self.render_glyph(gl, atlas, *glyph));
            }
        });

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
