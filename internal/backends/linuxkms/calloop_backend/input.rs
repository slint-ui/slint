// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the code to receive input events from libinput

use std::cell::RefCell;
#[cfg(feature = "libseat")]
use std::collections::HashMap;
#[cfg(not(feature = "libseat"))]
use std::fs::{File, OpenOptions};
use std::os::fd::OwnedFd;
#[cfg(feature = "libseat")]
use std::os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, RawFd};
#[cfg(not(feature = "libseat"))]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;

use i_slint_core::api::LogicalPosition;
use i_slint_core::platform::{PlatformError, PointerEventButton, WindowEvent};
use i_slint_core::window::WindowAdapter;
use i_slint_core::{Property, SharedString};
use input::LibinputInterface;

use input::event::keyboard::{KeyState, KeyboardEventTrait};
use input::event::touch::TouchEventPosition;
use xkbcommon::*;

use crate::fullscreenwindowadapter::FullscreenWindowAdapter;

#[cfg(feature = "libseat")]
struct SeatWrap {
    seat: Rc<RefCell<libseat::Seat>>,
    device_for_fd: HashMap<RawFd, libseat::Device>,
}

#[cfg(feature = "libseat")]
impl SeatWrap {
    pub fn new(seat: &Rc<RefCell<libseat::Seat>>) -> input::Libinput {
        let seat_name = seat.borrow_mut().name().to_string();
        let mut libinput = input::Libinput::new_with_udev(Self {
            seat: seat.clone(),
            device_for_fd: Default::default(),
        });
        libinput.udev_assign_seat(&seat_name).unwrap();
        libinput
    }
}

#[cfg(feature = "libseat")]
impl<'a> LibinputInterface for SeatWrap {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        self.seat
            .borrow_mut()
            .open_device(&path)
            .map(|device| {
                let flags = nix::fcntl::OFlag::from_bits_retain(flags);
                let fd = device.as_fd();
                nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_SETFL(flags))
                    .map_err(|e| format!("Error applying libinput provided open fd flags: {e}"))
                    .unwrap();

                let raw_fd = fd.as_raw_fd();
                self.device_for_fd.insert(raw_fd, device);
                // Safety: API requires us to own it, but in close_restricted() we'll take it back.
                unsafe { OwnedFd::from_raw_fd(raw_fd) }
            })
            .map_err(|e| e.0.into())
    }
    fn close_restricted(&mut self, fd: OwnedFd) {
        // Transfer ownership back to libseat
        let fd = fd.into_raw_fd();
        if let Some(device_id) = self.device_for_fd.remove(&fd) {
            let _ = self.seat.borrow_mut().close_device(device_id);
        }
    }
}

#[cfg(not(feature = "libseat"))]
struct DirectDeviceAccess {}

#[cfg(not(feature = "libseat"))]
impl DirectDeviceAccess {
    pub fn new() -> input::Libinput {
        let mut libinput = input::Libinput::new_with_udev(Self {});
        libinput.udev_assign_seat("seat0").unwrap();
        libinput
    }
}

#[cfg(not(feature = "libseat"))]
impl<'a> LibinputInterface for DirectDeviceAccess {
    fn open_restricted(&mut self, path: &Path, flags_raw: i32) -> Result<OwnedFd, i32> {
        let flags = nix::fcntl::OFlag::from_bits_retain(flags_raw);
        OpenOptions::new()
            .custom_flags(flags_raw)
            .read(
                flags.contains(nix::fcntl::OFlag::O_RDONLY)
                    | flags.contains(nix::fcntl::OFlag::O_RDWR),
            )
            .write(
                flags.contains(nix::fcntl::OFlag::O_WRONLY)
                    | flags.contains(nix::fcntl::OFlag::O_RDWR),
            )
            .open(path)
            .map(|file| file.into())
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(File::from(fd));
    }
}

pub struct LibInputHandler<'a> {
    libinput: input::Libinput,
    token: Option<calloop::Token>,
    mouse_pos: Pin<Rc<Property<Option<LogicalPosition>>>>,
    last_touch_pos: LogicalPosition,
    window: &'a RefCell<Option<Rc<FullscreenWindowAdapter>>>,
    keystate: Option<xkb::State>,
}

impl<'a> LibInputHandler<'a> {
    pub fn init<T>(
        window: &'a RefCell<Option<Rc<FullscreenWindowAdapter>>>,
        event_loop_handle: &calloop::LoopHandle<'a, T>,
        #[cfg(feature = "libseat")] seat: &'a Rc<RefCell<libseat::Seat>>,
    ) -> Result<Pin<Rc<Property<Option<LogicalPosition>>>>, PlatformError> {
        #[cfg(feature = "libseat")]
        let libinput = SeatWrap::new(seat);
        #[cfg(not(feature = "libseat"))]
        let libinput = DirectDeviceAccess::new();

        let mouse_pos_property = Rc::pin(Property::new(None));

        let handler = Self {
            libinput,
            token: Default::default(),
            mouse_pos: mouse_pos_property.clone(),
            last_touch_pos: Default::default(),
            window,
            keystate: Default::default(),
        };

        event_loop_handle
            .insert_source(handler, move |_, _, _| {})
            .map_err(|e| format!("Error registering libinput event source: {e}"))?;

        Ok(mouse_pos_property)
    }
}

impl<'a> calloop::EventSource for LibInputHandler<'a> {
    type Event = i_slint_core::platform::WindowEvent;
    type Metadata = ();
    type Ret = ();
    type Error = std::io::Error;

    fn process_events<F>(
        &mut self,
        _readiness: calloop::Readiness,
        token: calloop::Token,
        _callback: F,
    ) -> Result<calloop::PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        if Some(token) != self.token {
            return Ok(calloop::PostAction::Continue);
        }

        self.libinput.dispatch()?;

        let Some(adapter) = self.window.borrow().clone() else {
            return Ok(calloop::PostAction::Continue);
        };
        let window = adapter.window();
        let screen_size = window.size().to_logical(window.scale_factor());

        for event in &mut self.libinput {
            match event {
                input::Event::Pointer(pointer_event) => {
                    match pointer_event {
                        input::event::PointerEvent::Motion(motion_event) => {
                            let mut mouse_pos =
                                self.mouse_pos.as_ref().get().unwrap_or(LogicalPosition {
                                    x: screen_size.width / 2.,
                                    y: screen_size.height / 2.,
                                });
                            mouse_pos.x = (mouse_pos.x + motion_event.dx() as f32)
                                .clamp(0., screen_size.width);
                            mouse_pos.y = (mouse_pos.y + motion_event.dy() as f32)
                                .clamp(0., screen_size.height);
                            self.mouse_pos.set(Some(mouse_pos));
                            let event = WindowEvent::PointerMoved { position: mouse_pos };
                            window.try_dispatch_event(event).map_err(Self::Error::other)?;
                        }
                        input::event::PointerEvent::MotionAbsolute(abs_motion_event) => {
                            let mouse_pos = LogicalPosition {
                                x: abs_motion_event.absolute_x_transformed(screen_size.width as u32)
                                    as _,
                                y: abs_motion_event
                                    .absolute_y_transformed(screen_size.height as u32)
                                    as _,
                            };
                            self.mouse_pos.set(Some(mouse_pos));
                            let event = WindowEvent::PointerMoved { position: mouse_pos };
                            window.try_dispatch_event(event).map_err(Self::Error::other)?;
                        }
                        input::event::PointerEvent::Button(button_event) => {
                            // https://github.com/torvalds/linux/blob/0dd2a6fb1e34d6dcb96806bc6b111388ad324722/include/uapi/linux/input-event-codes.h#L355
                            let button = match button_event.button() {
                                0x110 => PointerEventButton::Left,
                                0x111 => PointerEventButton::Right,
                                0x112 => PointerEventButton::Middle,
                                0x116 => PointerEventButton::Back,
                                0x115 => PointerEventButton::Forward,
                                _ => PointerEventButton::Other,
                            };
                            let mouse_pos = self.mouse_pos.as_ref().get().unwrap_or_default();
                            let event = match button_event.button_state() {
                                input::event::tablet_pad::ButtonState::Pressed => {
                                    WindowEvent::PointerPressed { position: mouse_pos, button }
                                }
                                input::event::tablet_pad::ButtonState::Released => {
                                    WindowEvent::PointerReleased { position: mouse_pos, button }
                                }
                            };
                            window.try_dispatch_event(event).map_err(Self::Error::other)?;
                        }
                        _ => {}
                    }
                }
                input::Event::Touch(touch_event) => {
                    if let Some(event) = match touch_event {
                        input::event::TouchEvent::Down(touch_down_event) => {
                            self.last_touch_pos = LogicalPosition::new(
                                touch_down_event.x_transformed(screen_size.width as u32) as _,
                                touch_down_event.y_transformed(screen_size.height as u32) as _,
                            );
                            Some(WindowEvent::PointerPressed {
                                position: self.last_touch_pos,
                                button: PointerEventButton::Left,
                            })
                        }
                        input::event::TouchEvent::Up(..) => Some(WindowEvent::PointerReleased {
                            position: self.last_touch_pos,
                            button: PointerEventButton::Left,
                        }),
                        input::event::TouchEvent::Motion(touch_motion_event) => {
                            self.last_touch_pos = LogicalPosition::new(
                                touch_motion_event.x_transformed(screen_size.width as u32) as _,
                                touch_motion_event.y_transformed(screen_size.height as u32) as _,
                            );
                            Some(WindowEvent::PointerMoved { position: self.last_touch_pos })
                        }
                        _ => None,
                    } {
                        window.try_dispatch_event(event).map_err(Self::Error::other)?;
                    }
                }
                input::Event::Keyboard(input::event::KeyboardEvent::Key(key_event)) => {
                    // On Linux key codes have a fixed offset of 8: https://docs.rs/xkbcommon/0.6.0/xkbcommon/xkb/struct.Keycode.html
                    let key_code = xkb::Keycode::new(key_event.key() + 8);
                    let state = key_event.key_state();

                    let xkb_key_state = self.keystate.get_or_insert_with(|| {
                        let xkb_context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
                        let keymap =
                            xkb::Keymap::new_from_names(&xkb_context, "", "", "", "", None, 0)
                                .expect("Error compiling keymap");
                        xkb::State::new(&keymap)
                    });

                    let sym = xkb_key_state.key_get_one_sym(key_code);

                    xkb_key_state.update_key(
                        key_code,
                        match state {
                            input::event::tablet_pad::KeyState::Pressed => xkb::KeyDirection::Down,
                            input::event::tablet_pad::KeyState::Released => xkb::KeyDirection::Up,
                        },
                    );

                    let control = xkb_key_state
                        .mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE);
                    let alt = xkb_key_state
                        .mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE);

                    if state == KeyState::Pressed {
                        //eprintln!(
                        //"key {} state {:#?} sym {:x} control {control} alt {alt}",
                        //key_code, state, sym
                        //);

                        if control && alt && sym == xkb::Keysym::BackSpace
                            || control && alt && sym == xkb::Keysym::Delete
                        {
                            i_slint_core::api::quit_event_loop()
                                .expect("Unable to quit event loop multiple times");
                        } else if (xkb::Keysym::XF86_Switch_VT_1..=xkb::Keysym::XF86_Switch_VT_12)
                            .contains(&sym)
                        {
                            // let target_vt = (sym - xkb::KEY_XF86Switch_VT_1 + 1) as i32;
                            // TODO: eprintln!("switch vt {target_vt}");
                        }
                    }

                    if let Some(text) = map_key_sym(sym) {
                        let event = match state {
                            KeyState::Pressed => WindowEvent::KeyPressed { text },
                            KeyState::Released => WindowEvent::KeyReleased { text },
                        };
                        window.try_dispatch_event(event).map_err(Self::Error::other)?;
                    }
                }
                _ => {}
            }
            //println!("Got event: {:?}", event);
        }

        Ok(calloop::PostAction::Continue)
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.token = Some(token_factory.token());
        unsafe {
            poll.register(
                &self.libinput,
                calloop::Interest::READ,
                calloop::Mode::Level,
                self.token.unwrap(),
            )
        }
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.token = Some(token_factory.token());
        poll.reregister(
            &self.libinput,
            calloop::Interest::READ,
            calloop::Mode::Level,
            self.token.unwrap(),
        )
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> calloop::Result<()> {
        self.token = None;
        poll.unregister(&self.libinput)
    }
}

fn map_key_sym(sym: xkb::Keysym) -> Option<SharedString> {
    macro_rules! keysym_to_string {
        ($($char:literal # $name:ident # $($_qt:ident)|* # $($_winit:ident $(($_pos:ident))?)|* # $($xkb:ident)|*;)*) => {
            match(sym) {
                $($(xkb::Keysym::$xkb => $char,)*)*
                _ => std::char::from_u32(xkbcommon::xkb::keysym_to_utf32(sym))?,
            }
        };
    }
    let char = i_slint_common::for_each_special_keys!(keysym_to_string);
    Some(char.into())
}
