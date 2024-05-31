// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::rc::Rc;

use super::ImageCacheKey;
use crate::graphics::IntSize;
use crate::Property;

pub struct HTMLImage {
    pub dom_element: web_sys::HtmlImageElement,
    /// If present, this boolean property indicates whether the image has been uploaded yet or
    /// if that operation is still pending. If not present, then the image *is* available. This is
    /// used for remote HTML image loading and the property will be used to correctly track dependencies
    /// to graphics items that query for the size.
    image_load_pending: core::pin::Pin<Rc<Property<bool>>>,
}

impl HTMLImage {
    pub fn new(url: &str) -> Self {
        let dom_element = web_sys::HtmlImageElement::new().unwrap();

        let image_load_pending = Rc::pin(Property::new(true));

        dom_element.set_cross_origin(Some("anonymous"));
        dom_element.set_onload(Some(
            &wasm_bindgen::closure::Closure::once_into_js({
                let image_load_pending = image_load_pending.clone();
                move || {
                    image_load_pending.as_ref().set(false);

                    // As you can paint on a HTML canvas at any point in time, request_redraw()
                    // on a winit window only queues an additional internal event, that'll be
                    // be dispatched as the next event. We are however not in an event loop
                    // call, so we also need to wake up the event loop and redraw then.
                    let _ = crate::api::invoke_from_event_loop(|| {});
                }
            })
            .into(),
        ));
        dom_element.set_src(&url);

        Self { dom_element, image_load_pending }
    }

    pub fn size(&self) -> Option<IntSize> {
        match self.image_load_pending.as_ref().get() {
            true => None,
            false => Some(IntSize::new(
                self.dom_element.natural_width(),
                self.dom_element.natural_height(),
            )),
        }
    }

    pub fn source(&self) -> String {
        self.dom_element.src()
    }

    pub fn is_svg(&self) -> bool {
        self.dom_element.current_src().ends_with(".svg")
    }
}

impl super::OpaqueImage for HTMLImage {
    fn size(&self) -> IntSize {
        self.size().unwrap_or_default()
    }
    fn cache_key(&self) -> ImageCacheKey {
        ImageCacheKey::URL(self.source().into())
    }
}
