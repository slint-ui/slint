// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!

*NOTE*: This library is an internal crate for the [Slint project](https://sixtyfps.io).
This crate should not be used directly by application using Slint.
You should use the `slint` crate instead.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

use image::GenericImageView;
use slint_core_internal::component::ComponentRc;
use slint_core_internal::graphics::{Image, IntSize, Point, Size};
use slint_core_internal::window::{PlatformWindow, Window};
use slint_core_internal::ImageInner;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

#[derive(Default)]
pub struct TestingBackend {
    clipboard: Mutex<Option<String>>,
}

impl slint_core_internal::backend::Backend for TestingBackend {
    fn create_window(&'static self) -> Rc<Window> {
        Window::new(|_| Rc::new(TestingWindow::default()))
    }

    fn run_event_loop(
        &'static self,
        _behavior: slint_core_internal::backend::EventLoopQuitBehavior,
    ) {
        unimplemented!("running an event loop with the testing backend");
    }

    fn quit_event_loop(&'static self) {}

    fn register_font_from_memory(
        &'static self,
        _data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn register_font_from_path(
        &'static self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn set_clipboard_text(&'static self, text: String) {
        *self.clipboard.lock().unwrap() = Some(text);
    }

    fn clipboard_text(&'static self) -> Option<String> {
        self.clipboard.lock().unwrap().clone()
    }

    fn post_event(&'static self, _event: Box<dyn FnOnce() + Send>) {
        // The event will never be invoked
    }

    fn image_size(&'static self, image: &Image) -> IntSize {
        let inner: &ImageInner = image.into();
        match inner {
            ImageInner::None => Default::default(),
            ImageInner::EmbeddedImage(buffer) => buffer.size(),
            ImageInner::AbsoluteFilePath(path) => image::open(Path::new(path.as_str()))
                .map(|img| img.dimensions().into())
                .unwrap_or_default(),
            ImageInner::EmbeddedData { data, format } => image::load_from_memory_with_format(
                data.as_slice(),
                image::ImageFormat::from_extension(std::str::from_utf8(format.as_slice()).unwrap())
                    .unwrap(),
            )
            .map(|img| img.dimensions().into())
            .unwrap_or_default(),
            ImageInner::StaticTextures { size, .. } => *size,
        }
    }
}

#[derive(Default)]
pub struct TestingWindow {}

impl PlatformWindow for TestingWindow {
    fn show(self: Rc<Self>) {
        unimplemented!("showing a testing window")
    }

    fn hide(self: Rc<Self>) {}

    fn request_redraw(&self) {}

    fn free_graphics_resources<'a>(
        &self,
        _items: &mut dyn Iterator<Item = Pin<slint_core_internal::items::ItemRef<'a>>>,
    ) {
    }

    fn show_popup(&self, _popup: &ComponentRc, _position: slint_core_internal::graphics::Point) {
        todo!()
    }

    fn request_window_properties_update(&self) {}

    fn apply_window_properties(&self, _window_item: Pin<&slint_core_internal::items::WindowItem>) {
        todo!()
    }

    fn apply_geometry_constraint(
        &self,
        _constraints_horizontal: slint_core_internal::layout::LayoutInfo,
        _constraints_vertical: slint_core_internal::layout::LayoutInfo,
    ) {
    }

    fn set_mouse_cursor(&self, _cursor: slint_core_internal::items::MouseCursor) {}

    fn text_size(
        &self,
        _font_request: slint_core_internal::graphics::FontRequest,
        text: &str,
        _max_width: Option<f32>,
    ) -> Size {
        Size::new(text.len() as f32 * 10., 10.)
    }

    fn text_input_byte_offset_for_position(
        &self,
        _text_input: Pin<&slint_core_internal::items::TextInput>,
        _pos: Point,
    ) -> usize {
        0
    }

    fn text_input_position_for_byte_offset(
        &self,
        _text_input: Pin<&slint_core_internal::items::TextInput>,
        _byte_offset: usize,
    ) -> Point {
        Default::default()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Initialize the testing backend.
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will have no effects
pub fn init() {
    slint_core_internal::backend::instance_or_init(|| Box::new(TestingBackend::default()));
}
