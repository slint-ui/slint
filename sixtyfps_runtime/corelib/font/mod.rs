use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
mod fontkit;
#[cfg(not(target_arch = "wasm32"))]
use fontkit::*;

#[cfg(target_arch = "wasm32")]
mod canvasfont;
#[cfg(target_arch = "wasm32")]
use canvasfont::*;

#[derive(Default)]
struct FontMatch {
    fonts_per_pixel_size: Vec<Rc<Font>>,
}

#[derive(Default)]
pub struct FontCache {
    loaded_fonts: HashMap<FontHandle, FontMatch>,
}

impl FontCache {
    pub fn find_font(&mut self, family: &str, font_pixel_size: f32) -> Rc<Font> {
        let pixel_size = if font_pixel_size != 0. { font_pixel_size } else { 48.0 * 72. / 96. };

        let handle = FontHandle::new_from_match(family);

        let font_match = self.loaded_fonts.entry(handle.clone()).or_insert(FontMatch::default());

        font_match
            .fonts_per_pixel_size
            .iter()
            .find_map(|font| if font.pixel_size == pixel_size { Some(font.clone()) } else { None })
            .unwrap_or_else(|| {
                let fnt = Rc::new(handle.load(pixel_size).unwrap());
                font_match.fonts_per_pixel_size.push(fnt.clone());
                fnt
            })
    }
}

thread_local! {
    pub static FONT_CACHE: RefCell<FontCache> = RefCell::new(Default::default());
}
