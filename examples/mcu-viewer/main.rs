// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore rposition

//! Shows on a microcontroller the designs that the Slint live preview compiles on the
//! desktop and sends over USB.

#![no_std]
#![cfg_attr(not(feature = "simulator"), no_main)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use i_slint_compiler::llr::serialize;
use i_slint_core::InternalToken;
use i_slint_live_preview::SLINT_VERSION;
use i_slint_live_preview::serial::{
    DeviceEvent, DeviceMessage, DeviceSession, PROTOCOL_VERSION, encode_frame,
};
use mcu_board_support::HostLink;
// For the board's `entry` macro
#[allow(unused_imports)]
use mcu_board_support::prelude::*;
use slint::ComponentHandle;
use slint_interpreter::LoadedResource;

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
        resources: RefCell::default(),
        idle: RefCell::default(),
        shown: RefCell::default(),
    });
    viewer.show_idle("Waiting for the Slint live preview", "");
    viewer.send(&viewer.ready(true));

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, core::time::Duration::from_millis(5), {
        let viewer = viewer.clone();
        move || viewer.poll()
    });
    // Showing a design hides the idle screen first, leaving no window visible for a moment
    slint::run_event_loop_until_quit().unwrap();
    panic!("the event loop returned");
}

struct Viewer<Link> {
    link: RefCell<Link>,
    session: RefCell<DeviceSession>,
    /// The images and fonts that the desktop sent, by key, see [`DeviceEvent::Resource`].
    resources: RefCell<BTreeMap<u64, LoadedResource>>,
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
        // A later `Show` dropped the unit of an earlier one, and the host waits for the last
        let last_show = events.iter().rposition(|event| matches!(event, DeviceEvent::Show { .. }));
        for (index, event) in events.into_iter().enumerate() {
            match event {
                DeviceEvent::Show { .. } if Some(index) != last_show => {}
                DeviceEvent::Hello => self.send(&self.ready(false)),
                DeviceEvent::Resource { key, data } => {
                    match serialize::resource_from_bytes(&data) {
                        Ok(resource) => {
                            let resource = LoadedResource::new(resource, InternalToken);
                            self.resources.borrow_mut().insert(key, resource);
                        }
                        Err(err) => self.send(&DeviceMessage::Error {
                            id: None,
                            message: alloc::format!(
                                "Cannot read an image or font ({err}): the firmware needs the exact same Slint version as the editor"
                            ),
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
                DeviceEvent::Error(message) => {
                    log::warn!("{message}");
                    self.send(&DeviceMessage::Error { id: None, message });
                }
            }
        }
    }

    /// Show the component `component` of the compilation unit stored as `unit`, with the
    /// images and fonts stored under `keys`.
    fn load(&self, component: &str, unit: u64, keys: &[u64]) -> Result<(), String> {
        let session = self.session.borrow();
        let unit = session.blob(unit).ok_or("The design went missing")?;
        let mut stored = self.resources.borrow_mut();
        let resources = keys
            .iter()
            .map(|key| stored.get(key).cloned().ok_or("An image or font went missing"))
            .collect::<Result<Vec<_>, _>>()?;
        stored.retain(|key, _| keys.contains(key));
        drop(stored);

        // Make room first: the device has no memory to spare for two designs
        self.hide_shown();
        self.hide_idle();
        let unit = serialize::from_bytes(unit)
            .map_err(|err| {
                alloc::format!(
                    "Cannot read the design ({err}): the firmware needs the exact same Slint version as the editor"
                )
            })?;
        drop(session);
        let result = slint_interpreter::CompilationResult::from_compilation_unit(
            unit,
            resources,
            InternalToken,
        );
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

    /// Describes the device, with the scale factor of the window it shows designs in.
    fn ready(&self, boot: bool) -> DeviceMessage {
        let idle = self.idle.borrow();
        let shown = self.shown.borrow();
        let window = idle.as_ref().map(|idle| idle.window());
        let window = window.or_else(|| shown.as_ref().map(|shown| shown.window()));
        DeviceMessage::Ready {
            boot,
            protocol: PROTOCOL_VERSION,
            slint_version: SLINT_VERSION.into(),
            board: self.link.borrow().board_name().into(),
            scale_factor: window.map_or(1., |window| window.scale_factor()),
        }
    }

    fn heap_free(&self) -> u32 {
        self.link.borrow().free_memory().unwrap_or(0) as u32
    }

    fn send(&self, message: &DeviceMessage) {
        self.link.borrow_mut().write(&encode_frame(message));
    }
}
