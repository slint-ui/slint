// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::rc::Rc;

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
    fn new(url: &str) -> Self {
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

                    /*
                    crate::event_loop::GLOBAL_PROXY.with(|global_proxy| {
                        let mut maybe_proxy = global_proxy.borrow_mut();
                        let proxy = maybe_proxy.get_or_insert_with(Default::default);
                        // Calling send_event is usually done by winit at the bottom of the stack,
                        // in event handlers, and thus winit might decide to process the event
                        // immediately within that stack.
                        // To prevent re-entrancy issues that might happen by getting the application
                        // event processed on top of the current stack, set winit in Poll mode so that
                        // events are queued and process on top of a clean stack during a requested animation
                        // frame a few moments later.
                        // This also allows batching multiple post_event calls and redraw their state changes
                        // all at once.
                        proxy.send_event(crate::event_loop::CustomEvent::RedrawAllWindows);
                    });
                    */
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
            false => Some(IntSize::new(self.dom_element.width(), self.dom_element.height())),
        }
    }

    pub fn source(&self) -> String {
        self.dom_element.src()
    }
}
