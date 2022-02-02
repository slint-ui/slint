// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!

**NOTE**: This library is an **internal** crate for the [Slint project](https://sixtyfps.io).
This crate should **not be used directly** by applications using Slint.
You should use the `slint` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]
#![cfg_attr(not(feature = "simulator"), no_std)]
#![cfg_attr(feature = "pico-st7789", feature(alloc_error_handler))]

extern crate alloc;

use alloc::boxed::Box;
use core::cell::RefCell;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use slint_core_internal::graphics::{IntRect, IntSize};

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use slint_core_internal::unsafe_single_core;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use slint_core_internal::thread_local_ as thread_local;

#[cfg(feature = "simulator")]
mod simulator;

#[cfg(feature = "simulator")]
use simulator::event_loop;

mod renderer;

pub trait Devices {
    fn screen_size(&self) -> IntSize;
    fn fill_region(&mut self, region: IntRect, pixels: &[Rgb888]);
    fn read_touch_event(&mut self) -> Option<slint_core_internal::input::MouseEvent> {
        None
    }
    fn debug(&mut self, _: &str);
    fn time(&mut self) -> core::time::Duration {
        core::time::Duration::ZERO
    }
}

impl<T: embedded_graphics::draw_target::DrawTarget> crate::Devices for T
where
    T::Error: core::fmt::Debug,
    T::Color: core::convert::From<embedded_graphics::pixelcolor::Rgb888>,
{
    fn screen_size(&self) -> slint_core_internal::graphics::IntSize {
        let s = self.bounding_box().size;
        slint_core_internal::graphics::IntSize::new(s.width, s.height)
    }

    fn fill_region(&mut self, region: slint_core_internal::graphics::IntRect, pixels: &[Rgb888]) {
        self.color_converted()
            .fill_contiguous(
                &embedded_graphics::primitives::Rectangle::new(
                    Point::new(region.origin.x, region.origin.y),
                    Size::new(region.size.width as u32, region.size.height as u32),
                ),
                pixels.iter().copied(),
            )
            .unwrap()
    }

    fn debug(&mut self, text: &str) {
        use embedded_graphics::{
            mono_font::{ascii::FONT_6X10, MonoTextStyle},
            text::Text,
        };
        let style = MonoTextStyle::new(&FONT_6X10, Rgb888::RED.into());
        Text::new(text, Point::new(20, 30), style).draw(self).unwrap();
    }
}

thread_local! { static DEVICES: RefCell<Option<Box<dyn Devices + 'static>>> = RefCell::new(None) }

mod the_backend {
    use super::*;
    use alloc::boxed::Box;
    use alloc::collections::VecDeque;
    use alloc::rc::{Rc, Weak};
    use alloc::string::String;
    use core::cell::{Cell, RefCell};
    use core::pin::Pin;
    use slint_core_internal::component::ComponentRc;
    use slint_core_internal::graphics::{Color, Point, Size};
    use slint_core_internal::window::PlatformWindow;
    use slint_core_internal::window::Window;
    use slint_core_internal::ImageInner;

    thread_local! { static WINDOWS: RefCell<Option<Rc<McuWindow>>> = RefCell::new(None) }

    pub struct McuWindow {
        backend: &'static MCUBackend,
        self_weak: Weak<Window>,
        background_color: Cell<Color>,
    }

    impl PlatformWindow for McuWindow {
        fn show(self: Rc<Self>) {
            self.self_weak.upgrade().unwrap().set_scale_factor(
                option_env!("SLINT_SCALE_FACTOR").and_then(|x| x.parse().ok()).unwrap_or(1.),
            );
            WINDOWS.with(|x| *x.borrow_mut() = Some(self))
        }
        fn hide(self: Rc<Self>) {
            WINDOWS.with(|x| *x.borrow_mut() = None)
        }
        fn request_redraw(&self) {
            self.backend.with_inner(|inner| inner.post_event(McuEvent::Repaint))
        }
        fn free_graphics_resources<'a>(
            &self,
            _items: &mut dyn Iterator<Item = Pin<slint_core_internal::items::ItemRef<'a>>>,
        ) {
        }
        fn show_popup(
            &self,
            _popup: &ComponentRc,
            _position: slint_core_internal::graphics::Point,
        ) {
            todo!()
        }
        fn request_window_properties_update(&self) {}
        fn apply_window_properties(
            &self,
            window_item: Pin<&slint_core_internal::items::WindowItem>,
        ) {
            self.background_color.set(window_item.background());
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
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    enum McuEvent {
        Custom(Box<dyn FnOnce() + Send>),
        Quit,
        Repaint,
    }

    #[derive(Default)]
    struct MCUBackendInner {
        event_queue: VecDeque<McuEvent>,
        clipboard: String,
    }

    impl MCUBackendInner {
        fn post_event(&mut self, event: McuEvent) {
            self.event_queue.push_back(event);
            // TODO! wake
        }
    }

    #[derive(Default)]
    pub struct MCUBackend {
        #[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
        inner: RefCell<MCUBackendInner>,
        #[cfg(feature = "std")]
        inner: std::sync::Mutex<MCUBackendInner>,
    }

    #[cfg(feature = "unsafe_single_core")]
    unsafe impl Sync for MCUBackend {}
    #[cfg(feature = "unsafe_single_core")]
    unsafe impl Send for MCUBackend {}

    impl MCUBackend {
        fn with_inner<T>(&self, f: impl FnOnce(&mut MCUBackendInner) -> T) -> T {
            f(
                #[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
                &mut self.inner.borrow_mut(),
                #[cfg(feature = "std")]
                &mut self.inner.lock().unwrap(),
            )
        }

        fn draw(&self, window: Rc<McuWindow>) {
            let runtime_window = window.self_weak.upgrade().unwrap();
            runtime_window.update_window_properties();

            DEVICES.with(|devices| {
                let mut devices = devices.borrow_mut();
                let devices = devices.as_mut().unwrap();
                let size = devices.screen_size().to_f32() / runtime_window.scale_factor();
                runtime_window.set_window_item_geometry(size.width as _, size.height as _);
                let background =
                    crate::renderer::to_rgb888_color_discard_alpha(window.background_color.get());
                crate::renderer::render_window_frame(runtime_window, background, &mut **devices);
            });
        }
    }

    impl slint_core_internal::backend::Backend for MCUBackend {
        fn create_window(&'static self) -> Rc<slint_core_internal::window::Window> {
            slint_core_internal::window::Window::new(|window| {
                Rc::new(McuWindow {
                    backend: self,
                    self_weak: window.clone(),
                    background_color: Color::from_rgb_u8(0, 0, 0).into(),
                })
            })
        }

        fn run_event_loop(
            &'static self,
            behavior: slint_core_internal::backend::EventLoopQuitBehavior,
        ) {
            loop {
                slint_core_internal::animations::update_animations();
                match self.with_inner(|inner| inner.event_queue.pop_front()) {
                    Some(McuEvent::Quit) => break,
                    Some(McuEvent::Custom(e)) => e(),
                    Some(McuEvent::Repaint) => {
                        if let Some(window) = WINDOWS.with(|x| x.borrow().clone()) {
                            self.draw(window)
                        }
                    }
                    None => {
                        // TODO: sleep();
                    }
                }
                DEVICES.with(|devices| {
                    let e = devices.borrow_mut().as_mut().unwrap().read_touch_event();
                    if let Some(mut event) = e {
                        if let Some(window) = WINDOWS.with(|x| x.borrow().clone()) {
                            let w = window.self_weak.upgrade().unwrap();
                            // scale the event by the scale factor:
                            if let Some(p) = event.pos() {
                                event.translate(p / w.scale_factor() - p);
                            }
                            w.process_mouse_input(event);
                        }
                    }
                });
                match behavior {
                    slint_core_internal::backend::EventLoopQuitBehavior::QuitOnLastWindowClosed => {
                        if WINDOWS.with(|x| x.borrow().is_none()) {
                            break;
                        }
                    }
                    slint_core_internal::backend::EventLoopQuitBehavior::QuitOnlyExplicitly => (),
                }
            }
        }

        fn quit_event_loop(&'static self) {
            self.with_inner(|inner| inner.post_event(McuEvent::Quit))
        }

        fn set_clipboard_text(&'static self, text: String) {
            self.with_inner(|inner| inner.clipboard = text)
        }

        fn clipboard_text(&'static self) -> Option<String> {
            let c = self.with_inner(|inner| inner.clipboard.clone());
            c.is_empty().then(|| c)
        }

        fn post_event(&'static self, event: Box<dyn FnOnce() + Send>) {
            self.with_inner(|inner| inner.post_event(McuEvent::Custom(event)));
        }

        fn image_size(
            &'static self,
            image: &slint_core_internal::graphics::Image,
        ) -> slint_core_internal::graphics::IntSize {
            let inner: &ImageInner = image.into();
            match inner {
                ImageInner::None => Default::default(),
                ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => {
                    unimplemented!()
                }
                ImageInner::EmbeddedImage(buffer) => buffer.size(),
                ImageInner::StaticTextures { size, .. } => *size,
            }
        }

        #[cfg(feature = "std")]
        fn register_font_from_memory(
            &'static self,
            data: &'static [u8],
        ) -> Result<(), Box<dyn std::error::Error>> {
            unimplemented!()
        }
        #[cfg(feature = "std")]
        fn register_font_from_path(
            &'static self,
            path: &std::path::Path,
        ) -> Result<(), Box<dyn std::error::Error>> {
            unimplemented!()
        }

        fn duration_since_start(&'static self) -> core::time::Duration {
            DEVICES.with(|devices| devices.borrow_mut().as_mut().unwrap().time())
        }
    }
}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub mod native_widgets {}
pub const HAS_NATIVE_STYLE: bool = false;
pub const IS_AVAILABLE: bool = true;

#[cfg(feature = "simulator")]
pub fn init_simulator() {
    slint_core_internal::backend::instance_or_init(|| {
        alloc::boxed::Box::new(simulator::SimulatorBackend)
    });
}

pub fn init_with_display<Display: Devices + 'static>(display: Display) {
    DEVICES.with(|d| *d.borrow_mut() = Some(Box::new(display)));
    slint_core_internal::backend::instance_or_init(|| {
        alloc::boxed::Box::new(the_backend::MCUBackend::default())
    });
}

#[cfg(not(feature = "pico-st7789"))]
pub fn init_with_mock_display() {
    struct EmptyDisplay;
    impl embedded_graphics::draw_target::DrawTarget for EmptyDisplay {
        type Color = embedded_graphics::pixelcolor::Rgb888;
        type Error = ();
        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
        {
            let _ = pixels.into_iter().count();
            Ok(())
        }
    }
    impl embedded_graphics::geometry::OriginDimensions for EmptyDisplay {
        fn size(&self) -> embedded_graphics::geometry::Size {
            embedded_graphics::geometry::Size::new(320, 240)
        }
    }
    init_with_display(EmptyDisplay);
}

#[cfg(feature = "pico-st7789")]
mod pico_st7789;

#[cfg(feature = "pico-st7789")]
pub use pico_st7789::*;
