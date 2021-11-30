/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
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
pub struct Backend;

impl sixtyfps_corelib::backend::Backend for Backend {
    fn create_window(&'static self) -> Rc<Window> {
        sixtyfps_corelib::window::Window::new(|window| SimulatorWindow::new(window))
    }

    fn run_event_loop(&'static self, behavior: sixtyfps_corelib::backend::EventLoopQuitBehavior) {
        simulator::event_loop::run(behavior);
    }

    fn quit_event_loop(&'static self) {
        crate::event_loop::with_window_target(|event_loop| {
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
        let e = crate::event_loop::CustomEvent::UserEvent(event);
        crate::event_loop::GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap().send_event(e);
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
}

pub type NativeWidgets = ();
pub type NativeGlobals = ();
pub mod native_widgets {}
pub const HAS_NATIVE_STYLE: bool = false;
pub const IS_AVAILABLE: bool = true;

pub fn init() {
    sixtyfps_corelib::backend::instance_or_init(|| alloc::boxed::Box::new(Backend));
}
