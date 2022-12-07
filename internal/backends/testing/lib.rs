// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint-ui.com/logo/slint-logo-square-light.svg")]

use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, ScaleFactor};
use i_slint_core::renderer::Renderer;
use i_slint_core::software_renderer::MinimalSoftwareWindow;
use i_slint_core::window::WindowAdapter;
use i_slint_core::window::WindowAdapterSealed;

use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

pub struct SwrTestingBackend {
    window: Rc<MinimalSoftwareWindow<1>>,
}

impl i_slint_core::platform::Platform for SwrTestingBackend {
    fn create_window_adapter(&self) -> Rc<dyn i_slint_core::platform::WindowAdapter> {
        self.window.clone()
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

#[derive(Default)]
pub struct TestingBackend {
    clipboard: Mutex<Option<String>>,
}

impl i_slint_core::platform::Platform for TestingBackend {
    fn create_window_adapter(&self) -> Rc<dyn WindowAdapter> {
        Rc::new_cyclic(|self_weak| TestingWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            shown: false.into(),
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
    shown: core::cell::Cell<bool>,
}

impl WindowAdapterSealed for TestingWindow {
    fn show(&self) {
        self.shown.set(true);
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

    fn text_input_byte_offset_for_position(
        &self,
        _text_input: Pin<&i_slint_core::items::TextInput>,
        _pos: LogicalPoint,
    ) -> usize {
        0
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        _text_input: Pin<&i_slint_core::items::TextInput>,
        _byte_offset: usize,
    ) -> LogicalRect {
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

/// This module contains functions useful for unit tests
mod for_unit_test {
    use i_slint_core::api::ComponentHandle;
    use i_slint_core::graphics::euclid::{Box2D, Point2D};
    use i_slint_core::graphics::{Rgb8Pixel, SharedPixelBuffer};
    use i_slint_core::renderer::Renderer;
    use i_slint_core::software_renderer::{LineBufferProvider, MinimalSoftwareWindow};
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
        let dyn_rc = vtable::VRc::into_dyn(rc.clone());
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

    pub fn init_swr() -> std::rc::Rc<MinimalSoftwareWindow<1>> {
        let window = MinimalSoftwareWindow::new();

        i_slint_core::platform::set_platform(Box::new(crate::SwrTestingBackend {
            window: window.clone(),
        }))
        .unwrap();

        window
    }

    pub fn image_buffer(path: &str) -> SharedPixelBuffer<Rgb8Pixel> {
        let image = image::open(path).expect("Cannot open image.").into_rgb8();

        SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
            image.as_raw(),
            image.width(),
            image.height(),
        )
    }

    pub fn screenshot(
        window: std::rc::Rc<MinimalSoftwareWindow<1>>,
    ) -> SharedPixelBuffer<Rgb8Pixel> {
        let size = window.size();
        let width = size.width;
        let height = size.height;

        let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);

        // render to buffer
        window.request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.mark_dirty_region(Box2D::new(
                Point2D::new(0., 0.),
                Point2D::new(width as f32, height as f32),
            ));
            renderer.render(buffer.make_mut_slice(), width as usize);
        });

        buffer
    }

    struct TestingLineBuffer<'a> {
        buffer: &'a mut [Rgb8Pixel],
    }

    impl<'a> LineBufferProvider for TestingLineBuffer<'a> {
        type TargetPixel = Rgb8Pixel;

        fn process_line(
            &mut self,
            line: usize,
            range: core::ops::Range<usize>,
            render_fn: impl FnOnce(&mut [Self::TargetPixel]),
        ) {
            let start = line * range.len();
            let end = start + range.len();
            render_fn(&mut self.buffer[(start..end)]);
        }
    }

    pub fn assert_with_render(path: &str, window: std::rc::Rc<MinimalSoftwareWindow<1>>) {
        assert_eq!(image_buffer(path).as_bytes(), screenshot(window).as_bytes());
    }

    pub fn assert_with_render_by_line(path: &str, window: std::rc::Rc<MinimalSoftwareWindow<1>>) {
        assert_eq!(image_buffer(path).as_bytes(), screenshot_render_by_line(window).as_bytes());
    }

    pub fn screenshot_render_by_line(
        window: std::rc::Rc<MinimalSoftwareWindow<1>>,
    ) -> SharedPixelBuffer<Rgb8Pixel> {
        let size = window.size();
        let width = size.width;
        let height = size.height;

        let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width as u32, height as u32);

        // render to buffer
        window.request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.mark_dirty_region(Box2D::new(
                Point2D::new(0., 0.),
                Point2D::new(width as f32, height as f32),
            ));
            renderer.render_by_line(TestingLineBuffer { buffer: buffer.make_mut_slice() });
        });

        buffer
    }

    pub fn save_screenshot(path: &str, window: std::rc::Rc<MinimalSoftwareWindow<1>>) {
        let buffer = screenshot(window.clone());
        image::save_buffer(
            path,
            buffer.as_bytes(),
            window.size().width,
            window.size().height,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
}

pub use for_unit_test::*;
