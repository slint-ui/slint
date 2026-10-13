// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Shows on a microcontroller the designs that the Slint live preview compiles on the
//! desktop and sends over USB.

#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

extern crate alloc;

use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use i_slint_compiler::llr::serialize;
use i_slint_core::InternalToken;
use i_slint_live_preview::serial::{
    DeviceEvent, DeviceMessage, DeviceSession, PROTOCOL_VERSION, SLINT_VERSION, encode_frame,
};
use mcu_board_support::HostLink;
// For the board's `entry` macro
#[allow(unused_imports)]
use mcu_board_support::prelude::*;
use slint::ComponentHandle;

slint::include_modules!();

/// The biggest unit, image or font the device accepts. The stored unit, its deserialized
/// form and the textures and fonts all live in the PSRAM heap, which has 16 MiB.
const MAX_BLOB_LEN: usize = 4 << 20;

/// How much one tick of the event loop reads from the host, so that it still gets to draw.
const READ_BUDGET: usize = 16 * 1024;

#[mcu_board_support::entry]
fn main() -> ! {
    let link = mcu_board_support::init_with_host_link();
    let viewer = Rc::new(Viewer {
        link: RefCell::new(link),
        session: RefCell::new(DeviceSession::new(MAX_BLOB_LEN)),
        idle: RefCell::default(),
        shown: RefCell::default(),
    });
    viewer.show_idle("Waiting for the Slint live preview", "");
    viewer.send(&viewer.ready());

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, core::time::Duration::from_millis(5), {
        let viewer = viewer.clone();
        move || viewer.poll()
    });
    slint::run_event_loop().unwrap();
    panic!("the event loop returned");
}

struct Viewer<Link> {
    link: RefCell<Link>,
    session: RefCell<DeviceSession>,
    idle: RefCell<Option<IdleScreen>>,
    shown: RefCell<Option<slint_interpreter::ComponentInstance>>,
}

impl<Link: HostLink> Viewer<Link> {
    /// Read what the desktop sent, and act on it.
    fn poll(&self) {
        let mut events = Vec::new();
        {
            let mut link = self.link.borrow_mut();
            let mut session = self.session.borrow_mut();
            let mut buffer = [0u8; 1024];
            let mut received = 0;
            while received < READ_BUDGET {
                let len = link.read(&mut buffer);
                if len == 0 {
                    break;
                }
                received += len;
                session.push(&buffer[..len], |event| events.push(event));
            }
        }
        for event in events {
            match event {
                DeviceEvent::Hello => self.send(&self.ready()),
                DeviceEvent::Resource { key, data } => {
                    let resource = serialize::resource_from_bytes(&data);
                    // Make room for the converted resource
                    drop(data);
                    match resource {
                        Ok(resource) => slint_interpreter::register_embedded_resource(
                            key,
                            resource,
                            InternalToken,
                        ),
                        Err(err) => self.send(&DeviceMessage::Error {
                            id: None,
                            message: alloc::format!("Cannot read an image or font: {err}"),
                        }),
                    }
                }
                DeviceEvent::Show { id, component, unit, resources } => {
                    match self.load(&component, unit, &resources) {
                        Ok(()) => {
                            self.send(&DeviceMessage::Loaded { id, heap_free: self.heap_free() })
                        }
                        Err(message) => {
                            self.show_idle("Cannot show the design", &message);
                            self.send(&DeviceMessage::Error { id: Some(id), message });
                        }
                    }
                }
                DeviceEvent::Clear => {
                    self.hide_shown();
                    self.show_idle("Waiting for the Slint live preview", "");
                }
                DeviceEvent::Reset => self.link.borrow_mut().reset(),
                DeviceEvent::Error(message) => {
                    log::warn!("{message}");
                    self.send(&DeviceMessage::Error { id: None, message });
                }
            }
        }
    }

    /// Show the component `component` of the compilation unit stored as `unit`, with the
    /// embedded resources registered under `resources`.
    fn load(&self, component: &str, unit: u64, resources: &[u64]) -> Result<(), String> {
        // Make room first: the device has no memory to spare for two designs
        self.hide_shown();
        self.hide_idle();
        slint_interpreter::retain_embedded_resources(resources, InternalToken);

        let session = self.session.borrow();
        let unit = session.blob(unit).ok_or("The design went missing")?;
        let unit = serialize::from_bytes_with_registered_resources(unit, resources.iter().copied())
            .map_err(|err| alloc::format!("Cannot read the design: {err}"))?;
        drop(session);
        let result =
            slint_interpreter::CompilationResult::from_compilation_unit(unit, InternalToken);
        let definition = result
            .component(component)
            .ok_or_else(|| alloc::format!("No component {component}"))?;
        let instance = definition.create().map_err(|err| err.to_string())?;
        instance.show().map_err(|err| err.to_string())?;
        log::info!("Showing {component}, {} KiB free", self.heap_free() / 1024);
        self.shown.replace(Some(instance));
        Ok(())
    }

    // A visible window keeps its component alive, so each is hidden before it's dropped.
    fn hide_shown(&self) {
        if let Some(shown) = self.shown.take() {
            let _ = shown.hide();
        }
    }

    fn hide_idle(&self) {
        if let Some(idle) = self.idle.take() {
            let _ = idle.hide();
        }
    }

    fn show_idle(&self, message: &str, detail: &str) {
        self.hide_idle();
        let idle = IdleScreen::new().unwrap();
        idle.set_message(message.into());
        idle.set_detail(if !detail.is_empty() {
            detail.into()
        } else if let Some(port_name) = self.link.borrow().port_name() {
            alloc::format!("Slint {SLINT_VERSION}, on {port_name}").into()
        } else {
            alloc::format!("Slint {SLINT_VERSION}, {} KiB free", self.heap_free() / 1024).into()
        });
        idle.show().unwrap();
        self.idle.replace(Some(idle));
    }

    /// Describes the device, with the size and scale factor of the window it shows
    /// designs in.
    fn ready(&self) -> DeviceMessage {
        let (size, scale_factor) = self.idle.borrow().as_ref().map_or_else(
            || (slint::PhysicalSize::default(), 1.),
            |idle| (idle.window().size(), idle.window().scale_factor()),
        );
        DeviceMessage::Ready {
            protocol: PROTOCOL_VERSION,
            slint_version: SLINT_VERSION.into(),
            format: serialize::FORMAT_ID,
            board: self.link.borrow().board_name().into(),
            width: size.width,
            height: size.height,
            scale_factor,
            heap_free: self.heap_free(),
        }
    }

    fn heap_free(&self) -> u32 {
        self.link.borrow().free_memory().unwrap_or(0) as u32
    }

    fn send(&self, message: &DeviceMessage) {
        self.link.borrow_mut().write(&encode_frame(message));
    }
}
