// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

use i_slint_core::graphics::euclid::{Point2D, Size2D};
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, ScaleFactor};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_core::window::WindowAdapterSealed;

use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

#[derive(Default)]
pub struct TestingBackend {
    clipboard: Mutex<Option<String>>,
}

impl i_slint_core::platform::Platform for TestingBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, i_slint_core::platform::PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| TestingWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            shown: false.into(),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        // The slint::testing::mock_elapsed_time updates the animation tick directly
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }

    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        if clipboard == i_slint_core::platform::Clipboard::DefaultClipboard {
            *self.clipboard.lock().unwrap() = Some(text.into());
        }
    }

    fn clipboard_text(&self, clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
        if clipboard == i_slint_core::platform::Clipboard::DefaultClipboard {
            self.clipboard.lock().unwrap().clone()
        } else {
            None
        }
    }
}

pub struct TestingWindow {
    window: i_slint_core::api::Window,
    shown: core::cell::Cell<bool>,
}

impl WindowAdapterSealed for TestingWindow {
    fn show(&self) -> Result<(), PlatformError> {
        self.shown.set(true);
        Ok(())
    }

    fn hide(&self) {
        self.shown.set(false);
    }

    fn renderer(&self) -> &dyn Renderer {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn position(&self) -> i_slint_core::api::PhysicalPosition {
        unimplemented!()
    }

    fn set_position(&self, _position: i_slint_core::api::WindowPosition) {
        unimplemented!()
    }

    fn is_visible(&self) -> bool {
        self.shown.get()
    }
}

impl WindowAdapter for TestingWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }
}

impl Renderer for TestingWindow {
    fn text_size(
        &self,
        _font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        _max_width: Option<LogicalLength>,
        _scale_factor: ScaleFactor,
    ) -> LogicalSize {
        LogicalSize::new(text.len() as f32 * 10., 10.)
    }

    // this works only for single line text
    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
    ) -> usize {
        let text_len = text_input.text().len();
        let result = pos.x / 10.;
        result.min(text_len as f32).max(0.) as usize
    }

    // this works only for single line text
    fn text_input_cursor_rect_for_byte_offset(
        &self,
        _text_input: Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
    ) -> LogicalRect {
        LogicalRect::new(Point2D::new(byte_offset as f32 * 10., 0.), Size2D::new(1., 10.))
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

    fn default_font_size(&self) -> LogicalLength {
        LogicalLength::new(10.)
    }
}

/// Initialize the testing backend.
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will have no effects
pub fn init() {
    i_slint_core::platform::set_platform(Box::new(TestingBackend::default()))
        .expect("platform already initialized");
}

/// This module contains functions useful for unit tests
mod for_unit_test {
    use i_slint_core::api::ComponentHandle;
    pub use i_slint_core::tests::slint_mock_elapsed_time as mock_elapsed_time;
    use i_slint_core::window::WindowInner;
    use i_slint_core::SharedString;

    /// Simulate a mouse click
    pub fn send_mouse_click<
        X: vtable::HasStaticVTable<i_slint_core::component::ComponentVTable> + 'static,
        Component: Into<vtable::VRc<i_slint_core::component::ComponentVTable, X>> + ComponentHandle,
    >(
        component: &Component,
        x: f32,
        y: f32,
    ) {
        let rc = component.clone_strong().into();
        let dyn_rc = vtable::VRc::into_dyn(rc);
        i_slint_core::tests::slint_send_mouse_click(
            &dyn_rc,
            x,
            y,
            &WindowInner::from_pub(component.window()).window_adapter(),
        );
    }

    /// Simulate entering a sequence of ascii characters key by (pressed or released).
    pub fn send_keyboard_char<
        X: vtable::HasStaticVTable<i_slint_core::component::ComponentVTable>,
        Component: Into<vtable::VRc<i_slint_core::component::ComponentVTable, X>> + ComponentHandle,
    >(
        component: &Component,
        string: char,
        pressed: bool,
    ) {
        i_slint_core::tests::slint_send_keyboard_char(
            &SharedString::from(string),
            pressed,
            &WindowInner::from_pub(component.window()).window_adapter(),
        )
    }

    /// Simulate entering a sequence of ascii characters key by key.
    pub fn send_keyboard_string_sequence<
        X: vtable::HasStaticVTable<i_slint_core::component::ComponentVTable>,
        Component: Into<vtable::VRc<i_slint_core::component::ComponentVTable, X>> + ComponentHandle,
    >(
        component: &Component,
        sequence: &str,
    ) {
        i_slint_core::tests::send_keyboard_string_sequence(
            &SharedString::from(sequence),
            &WindowInner::from_pub(component.window()).window_adapter(),
        )
    }

    /// Applies the specified scale factor to the window that's associated with the given component.
    /// This overrides the value provided by the windowing system.
    pub fn set_window_scale_factor<
        X: vtable::HasStaticVTable<i_slint_core::component::ComponentVTable>,
        Component: Into<vtable::VRc<i_slint_core::component::ComponentVTable, X>> + ComponentHandle,
    >(
        component: &Component,
        factor: f32,
    ) {
        WindowInner::from_pub(component.window()).set_scale_factor(factor)
    }
}

pub use for_unit_test::*;
use i_slint_core::platform::PlatformError;
