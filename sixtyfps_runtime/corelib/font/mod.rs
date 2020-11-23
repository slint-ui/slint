/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use crate::string::SharedString;
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

pub trait HasFont {
    fn font_family(&self) -> SharedString;
    fn font_pixel_size(&self, window: &crate::eventloop::ComponentWindow) -> f32;
    fn font(&self, window: &crate::eventloop::ComponentWindow) -> Rc<Font> {
        crate::font::FONT_CACHE
            .with(|fc| fc.find_font(&self.font_family(), self.font_pixel_size(window)))
    }
}

#[derive(Default)]
pub struct FontCache {
    // index by family name
    loaded_fonts: RefCell<HashMap<SharedString, FontMatch>>,
}

impl FontCache {
    pub fn find_font(&self, family: &SharedString, pixel_size: f32) -> Rc<Font> {
        assert_ne!(pixel_size, 0.0);

        let mut loaded_fonts = self.loaded_fonts.borrow_mut();
        let font_match = loaded_fonts.entry(family.clone()).or_insert_with(|| FontMatch {
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
