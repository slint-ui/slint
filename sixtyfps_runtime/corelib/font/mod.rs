use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
mod fontkit;
#[cfg(not(target_arch = "wasm32"))]
pub use fontkit::*;

#[cfg(target_arch = "wasm32")]
mod canvasfont;
#[cfg(target_arch = "wasm32")]
pub use canvasfont::*;

struct FontMatch {
    handle: FontHandle,
    fonts_per_pixel_size: Vec<Rc<Font>>,
}

#[derive(Default)]
pub struct FontCache {
    // index by family name
    loaded_fonts: RefCell<HashMap<String, FontMatch>>,
}

impl FontCache {
    pub fn find_font(&self, family: &str, font_pixel_size: f32) -> Rc<Font> {
        let pixel_size = if font_pixel_size != 0. { font_pixel_size } else { 48.0 * 72. / 96. };

        let mut loaded_fonts = self.loaded_fonts.borrow_mut();
        let font_match = loaded_fonts.entry(family.to_owned()).or_insert_with(|| FontMatch {
            handle: FontHandle::new_from_match(family),
            fonts_per_pixel_size: Vec::new(),
        });

        font_match
            .fonts_per_pixel_size
            .iter()
            .find_map(|font| if font.pixel_size == pixel_size { Some(font.clone()) } else { None })
            .unwrap_or_else(|| {
                let fnt = Rc::new(font_match.handle.load(pixel_size).unwrap());
                font_match.fonts_per_pixel_size.push(fnt.clone());
                fnt
            })
    }
}

thread_local! {
    pub static FONT_CACHE: FontCache = Default::default();
}
