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

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use sixtyfps_corelib::{
    graphics::{Image, Size},
    window::Window,
    ImageInner,
};

#[cfg(feature = "simulator")]
mod simulator;

#[cfg(feature = "simulator")]
use simulator::*;

mod renderer;

#[cfg(not(feature = "simulator"))]
mod dummy {
    use super::*;
    use alloc::rc::Weak;
    use core::pin::Pin;
    use sixtyfps_corelib::component::ComponentRc;
    use sixtyfps_corelib::graphics::Point;
    use sixtyfps_corelib::window::Window;

    #[derive(Default)]
    pub struct SimulatorWindow {
        self_weak: Weak<Window>,
    }
    impl SimulatorWindow {
        pub fn new(self_weak: &Weak<Window>) -> Rc<Self> {
            Self { self_weak: self_weak.clone() }.into()
        }
    }
    impl sixtyfps_corelib::window::PlatformWindow for SimulatorWindow {
        fn show(self: Rc<Self>) {
            let runtime_window = self.self_weak.upgrade().unwrap();
            let mut display = embedded_graphics::mock_display::MockDisplay::new();
            crate::renderer::render_window_frame(runtime_window, Default::default(), &mut display);
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
        fn apply_window_properties(&self, _window_item: Pin<&sixtyfps_corelib::items::WindowItem>) {
            //todo!()
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
}
#[cfg(not(feature = "simulator"))]
use dummy::*;

pub struct Backend;

impl sixtyfps_corelib::backend::Backend for Backend {
    fn create_window(&'static self) -> Rc<Window> {
        sixtyfps_corelib::window::Window::new(|window| SimulatorWindow::new(window))
    }

    fn run_event_loop(&'static self, behavior: sixtyfps_corelib::backend::EventLoopQuitBehavior) {
        #[cfg(feature = "simulator")]
        simulator::event_loop::run(behavior);
    }

    fn quit_event_loop(&'static self) {
        #[cfg(feature = "simulator")]
        simulator::event_loop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(simulator::event_loop::CustomEvent::Exit).ok();
        })
    }

    #[cfg(feature = "simulator")]
    fn register_font_from_memory(
        &'static self,
        _data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        //TODO
        Err("Not implemented".into())
    }

    #[cfg(feature = "simulator")]
    fn register_font_from_path(
        &'static self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unimplemented!()
    }

    fn set_clipboard_text(&'static self, _text: String) {
        unimplemented!()
    }

    fn clipboard_text(&'static self) -> Option<String> {
        unimplemented!()
    }

    fn post_event(&'static self, event: Box<dyn FnOnce() + Send>) {
        #[cfg(feature = "simulator")]
        simulator::event_loop::GLOBAL_PROXY
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .send_event(simulator::event_loop::CustomEvent::UserEvent(event));
    }

    fn image_size(&'static self, image: &Image) -> Size {
        let inner: &ImageInner = image.into();
        match inner {
            ImageInner::None => Default::default(),
            ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => unimplemented!(),
            ImageInner::EmbeddedImage(buffer) => {
                [buffer.width() as f32, buffer.height() as f32].into()
            }
            ImageInner::StaticTextures { size, .. } => size.cast(),
        }
    }

    #[cfg(not(feature = "simulator"))]
    fn duration_since_start(&'static self) -> core::time::Duration {
        todo!()
    }
}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub mod native_widgets {}
pub const HAS_NATIVE_STYLE: bool = false;
pub const IS_AVAILABLE: bool = true;

pub fn init() {
    sixtyfps_corelib::backend::instance_or_init(|| alloc::boxed::Box::new(Backend));
}
