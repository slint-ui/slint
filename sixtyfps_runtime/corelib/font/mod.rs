/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
Font abstraction for the run-time library.
*/
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
    handle: Rc<PlatformFont>,
    fonts_per_pixel_size: Vec<Rc<Font>>,
}

/// FontRequest collects all the developer-configurable properties for fonts, such as family, weight, etc.
/// It is submitted as a request to the platform font system (i.e. CoreText on macOS) and in exchange we
/// store a Rc<FontHandle>
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub struct FontRequest {
    family: SharedString,
    weight: i32,
    pixel_size: f32,
}

/// HasFont is a convenience trait for items holding font properties, such as Text or TextInput.
pub trait HasFont {
    /// Return the value of the font-family property.
    fn font_family(&self) -> SharedString;
    /// Return the value of the font-weight property.
    fn font_weight(&self) -> i32;
    /// Return the value if the font-size property converted to window specific pixels, respecting
    /// the window scale factor.
    fn font_pixel_size(&self, window: &crate::eventloop::ComponentWindow) -> f32;
    /// Translates the values of the different font related properties into a FontRequest object.
    fn font_request(&self, window: &crate::eventloop::ComponentWindow) -> FontRequest {
        FontRequest {
            family: self.font_family(),
            weight: self.font_weight(),
            pixel_size: self.font_pixel_size(window),
        }
    }
    /// Returns a Font object that matches the requested font properties of this trait object (item).
    fn font(&self, window: &crate::eventloop::ComponentWindow) -> Rc<Font> {
        crate::font::FONT_CACHE.with(|fc| fc.find_font(&self.font_request(window)))
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FontCacheKey {
    family: SharedString,
    weight: i32,
}

impl FontCacheKey {
    fn new(request: &FontRequest) -> Self {
        Self { family: request.family.clone(), weight: request.weight }
    }
}

/// FontCache caches the expensive process of looking up fonts by family, weight, style, etc. (FontRequest)
#[derive(Default)]
pub struct FontCache {
    // index by family name
    loaded_fonts: RefCell<HashMap<FontCacheKey, FontMatch>>,
}

impl FontCache {
    /// Submits the given FontRequest to the platform's font system (i.e. CoreText) and returns the font found.
    /// The result is cached, so this function should be cheap to call.
    pub fn find_font(&self, request: &FontRequest) -> Rc<Font> {
        assert_ne!(request.pixel_size, 0.0);

        let mut loaded_fonts = self.loaded_fonts.borrow_mut();
        let font_match =
            loaded_fonts.entry(FontCacheKey::new(request)).or_insert_with(|| FontMatch {
                handle: PlatformFont::new_from_request(&request).unwrap(),
                fonts_per_pixel_size: Vec::new(),
            });

        font_match
            .fonts_per_pixel_size
            .iter()
            .find_map(
                |font| {
                    if font.pixel_size == request.pixel_size {
                        Some(font.clone())
                    } else {
                        None
                    }
                },
            )
            .unwrap_or_else(|| {
                let fnt = Rc::new(font_match.handle.load(request.pixel_size));
                font_match.fonts_per_pixel_size.push(fnt.clone());
                fnt
            })
    }
}

thread_local! {
    /// The thread-local font-cache holding references to resolved font requests
    pub static FONT_CACHE: FontCache = Default::default();
}
