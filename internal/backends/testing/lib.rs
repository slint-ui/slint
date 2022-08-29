// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

use i_slint_core::api::euclid;
use i_slint_core::api::PhysicalPx;
use i_slint_core::graphics::{Point, Rect, Size};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::PlatformWindow;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

#[derive(Default)]
pub struct TestingBackend {
    clipboard: Mutex<Option<String>>,
}

impl i_slint_core::platform::PlatformAbstraction for TestingBackend {
    fn create_window(&self) -> Rc<dyn PlatformWindow> {
        Rc::new_cyclic(|self_weak| TestingWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
        })
    }

    fn duration_since_start(&self) -> core::time::Duration {
        // The slint::testing::mock_elapsed_time updates the animation tick directly
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }

    fn set_clipboard_text(&self, text: &str) {
        *self.clipboard.lock().unwrap() = Some(text.into());
    }

    fn clipboard_text(&self) -> Option<String> {
        self.clipboard.lock().unwrap().clone()
    }
}

pub struct TestingWindow {
    window: i_slint_core::api::Window,
}

impl PlatformWindow for TestingWindow {
    fn show(&self) {
        unimplemented!("showing a testing window")
    }

    fn hide(&self) {}

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

    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
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

    fn register_font_from_memory(
        &self,
        _data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn register_font_from_path(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Initialize the testing backend.
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will have no effects
pub fn init() {
    i_slint_core::platform::set_platform(Box::new(TestingBackend::default()))
        .expect("platform already initialized");
}
