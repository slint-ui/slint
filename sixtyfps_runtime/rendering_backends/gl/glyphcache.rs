use super::text::CachedFontGlyphs;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
