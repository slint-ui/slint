// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

use i_slint_core::api::euclid;
use i_slint_core::api::PhysicalPx;
use i_slint_core::api::Window;
use i_slint_core::graphics::{Point, Rect, Size};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::{PlatformWindow, WindowInner};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

#[derive(Default)]
pub struct TestingBackend {
    clipboard: Mutex<Option<String>>,
}

impl i_slint_core::backend::Backend for TestingBackend {
    fn create_window(&'static self) -> Window {
        WindowInner::new(|_| Rc::new(TestingWindow::default())).into()
    }

    fn run_event_loop(&'static self, _behavior: i_slint_core::backend::EventLoopQuitBehavior) {
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

    fn post_event(&'static self, _event: Box<dyn FnOnce() + Send>) {
        // The event will never be invoked
    }

    fn set_clipboard_text(&'static self, text: &str) {
        *self.clipboard.lock().unwrap() = Some(text.into());
    }

    fn clipboard_text(&'static self) -> Option<String> {
        self.clipboard.lock().unwrap().clone()
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

    fn register_component(&self) {}

    fn unregister_component<'a>(
        &self,
        _: i_slint_core::component::ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'a>>>,
    ) {
    }

    fn request_window_properties_update(&self) {}

    fn apply_window_properties(&self, _window_item: Pin<&i_slint_core::items::WindowItem>) {
        todo!()
    }

    fn apply_geometry_constraint(
        &self,
        _constraints_horizontal: i_slint_core::layout::LayoutInfo,
        _constraints_vertical: i_slint_core::layout::LayoutInfo,
    ) {
    }

    fn set_mouse_cursor(&self, _cursor: i_slint_core::items::MouseCursor) {}

    fn renderer(&self) -> &dyn Renderer {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn position(&self) -> euclid::Point2D<i32, PhysicalPx> {
        unimplemented!()
    }

    fn set_position(&self, _position: euclid::Point2D<i32, PhysicalPx>) {
        unimplemented!()
    }

    fn inner_size(&self) -> euclid::Size2D<u32, PhysicalPx> {
        unimplemented!()
    }

    fn set_inner_size(&self, _size: euclid::Size2D<u32, PhysicalPx>) {
        unimplemented!()
    }
}

impl Renderer for TestingWindow {
    fn text_size(
        &self,
        _font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        _max_width: Option<f32>,
        _scale_factor: f32,
    ) -> Size {
        Size::new(text.len() as f32 * 10., 10.)
    }

    fn text_input_byte_offset_for_position(
        &self,
        _text_input: Pin<&i_slint_core::items::TextInput>,
        _pos: Point,
    ) -> usize {
        0
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        _text_input: Pin<&i_slint_core::items::TextInput>,
        _byte_offset: usize,
    ) -> Rect {
        Default::default()
    }
}

/// Initialize the testing backend.
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will have no effects
pub fn init() {
    i_slint_core::backend::instance_or_init(|| Box::new(TestingBackend::default()));
}
