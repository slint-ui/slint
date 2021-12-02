// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!

**NOTE**: This library is an **internal** crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should **not be used directly** by applications using SixtyFPS.
You should use the `sixtyfps` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]
#![cfg_attr(not(feature = "simulator"), no_std)]

extern crate alloc;

#[cfg(feature = "simulator")]
mod simulator;

#[cfg(feature = "simulator")]
use simulator::event_loop;

mod renderer;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use sixtyfps_corelib::unsafe_single_core;

#[cfg(all(not(feature = "std"), feature = "unsafe_single_core"))]
use sixtyfps_corelib::thread_local_ as thread_local;

#[cfg(feature = "snapshot_renderer")]
// Backend to render one snapshot frame
mod snapshotbackend {
    use super::*;
    use alloc::boxed::Box;
    use alloc::rc::{Rc, Weak};
    use alloc::string::String;
    use core::cell::{Cell, RefCell};
    use core::pin::Pin;
    use sixtyfps_corelib::component::ComponentRc;
    use sixtyfps_corelib::graphics::{Color, Image, Point, Size};
    use sixtyfps_corelib::window::PlatformWindow;
    use sixtyfps_corelib::window::Window;
    use sixtyfps_corelib::ImageInner;

    thread_local! {
        static CUSTOM_WINDOW: RefCell<Option<Box<dyn FnOnce(&Weak<sixtyfps_corelib::window::Window>) -> Rc<dyn PlatformWindow>>>> = RefCell::new(None)
    }

    pub struct SingleFrameWindow<Display: 'static> {
        self_weak: Weak<Window>,
        display: RefCell<Display>,
        background_color: Cell<Color>,
    }
    impl<Display: 'static> SingleFrameWindow<Display> {
        pub fn new(self_weak: &Weak<Window>, display: Display) -> Rc<Self> {
            Self {
                self_weak: self_weak.clone(),
                display: RefCell::new(display),
                background_color: Color::from_rgb_u8(0, 0, 0).into(),
            }
            .into()
        }
    }
    impl<Display, DisplayColor, DisplayError> sixtyfps_corelib::window::PlatformWindow
        for SingleFrameWindow<Display>
    where
        Display:
            embedded_graphics::draw_target::DrawTarget<Color = DisplayColor, Error = DisplayError>,
        DisplayColor: core::convert::From<embedded_graphics::pixelcolor::Rgb888>,
        DisplayError: core::fmt::Debug,
    {
        fn show(self: Rc<Self>) {
            use embedded_graphics::draw_target::DrawTargetExt;
            let runtime_window = self.self_weak.upgrade().unwrap();

            runtime_window.update_window_properties();

            let mut display = self.display.borrow_mut();

            let size = display.bounding_box().size;
            runtime_window.set_window_item_geometry(size.width as _, size.height as _);

            let background =
                crate::renderer::to_rgb888_color_discard_alpha(self.background_color.get());

            let mut display = display.color_converted();
            crate::renderer::render_window_frame(runtime_window, background, &mut display);
        }
        fn hide(self: Rc<Self>) {}
        fn request_redraw(&self) {}
        fn free_graphics_resources<'a>(
            &self,
            _items: &mut dyn Iterator<Item = Pin<sixtyfps_corelib::items::ItemRef<'a>>>,
        ) {
        }
        fn show_popup(&self, _popup: &ComponentRc, _position: sixtyfps_corelib::graphics::Point) {
            todo!()
        }
        fn request_window_properties_update(&self) {}
        fn apply_window_properties(&self, window_item: Pin<&sixtyfps_corelib::items::WindowItem>) {
            //todo!()
            self.background_color.set(window_item.background());
        }
        fn apply_geometry_constraint(
            &self,
            _constraints_horizontal: sixtyfps_corelib::layout::LayoutInfo,
            _constraints_vertical: sixtyfps_corelib::layout::LayoutInfo,
        ) {
        }
        fn set_mouse_cursor(&self, _cursor: sixtyfps_corelib::items::MouseCursor) {}
        fn text_size(
            &self,
            _font_request: sixtyfps_corelib::graphics::FontRequest,
            text: &str,
            _max_width: Option<f32>,
        ) -> Size {
            Size::new(text.len() as f32 * 10., 10.)
        }

        fn text_input_byte_offset_for_position(
            &self,
            _text_input: Pin<&sixtyfps_corelib::items::TextInput>,
            _pos: Point,
        ) -> usize {
            0
        }
        fn text_input_position_for_byte_offset(
            &self,
            _text_input: Pin<&sixtyfps_corelib::items::TextInput>,
            _byte_offset: usize,
        ) -> Point {
            Default::default()
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    pub struct SnapshotBackend;

    impl SnapshotBackend {
        pub fn new_with_display<Display, DisplayColor, DisplayError>(display: Display) -> Self
        where
            Display: embedded_graphics::draw_target::DrawTarget<
                    Color = DisplayColor,
                    Error = DisplayError,
                > + 'static,
            DisplayColor: core::convert::From<embedded_graphics::pixelcolor::Rgb888>,
            DisplayError: core::fmt::Debug,
        {
            CUSTOM_WINDOW.with(|window_factory| {
                *window_factory.borrow_mut() =
                    Some(Box::new(move |window| SingleFrameWindow::new(window, display)))
            });

            Self
        }
    }

    impl sixtyfps_corelib::backend::Backend for SnapshotBackend {
        fn create_window(&'static self) -> Rc<Window> {
            sixtyfps_corelib::window::Window::new(|window| {
                CUSTOM_WINDOW.with(|window_factory| match window_factory.borrow_mut().take() {
                    Some(f) => f(window),
                    None => {
                        todo!()
                    }
                })
            })
        }

        fn run_event_loop(
            &'static self,
            _behavior: sixtyfps_corelib::backend::EventLoopQuitBehavior,
        ) {
        }

        fn quit_event_loop(&'static self) {}

        fn set_clipboard_text(&'static self, _text: String) {
            unimplemented!()
        }

        fn clipboard_text(&'static self) -> Option<String> {
            unimplemented!()
        }

        fn post_event(&'static self, _event: Box<dyn FnOnce() + Send>) {
            unimplemented!()
        }

        fn image_size(&'static self, image: &Image) -> Size {
            let inner: &ImageInner = image.into();
            match inner {
                ImageInner::None => Default::default(),
                ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => {
                    unimplemented!()
                }
                ImageInner::EmbeddedImage(buffer) => {
                    [buffer.width() as f32, buffer.height() as f32].into()
                }
                ImageInner::StaticTextures { size, .. } => size.cast(),
            }
        }

        fn duration_since_start(&'static self) -> core::time::Duration {
            todo!()
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
    sixtyfps_corelib::backend::instance_or_init(|| {
        alloc::boxed::Box::new(simulator::SimulatorBackend)
    });
}

#[cfg(feature = "snapshot_renderer")]
pub fn init_with_display<Display, DisplayColor, DisplayError>(display: Display)
where
    Display: embedded_graphics::draw_target::DrawTarget<Color = DisplayColor, Error = DisplayError>
        + 'static,
    DisplayColor: core::convert::From<embedded_graphics::pixelcolor::Rgb888>,
    DisplayError: core::fmt::Debug,
{
    sixtyfps_corelib::backend::instance_or_init(|| {
        alloc::boxed::Box::new(snapshotbackend::SnapshotBackend::new_with_display(display))
    });
}

#[cfg(not(feature = "simulator"))]
pub fn init_with_mock_display() {
    init_with_display(embedded_graphics::mock_display::MockDisplay::<
        embedded_graphics::pixelcolor::Rgb888,
    >::new());
}
