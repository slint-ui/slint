// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This module contains the code to receive input events from libinput

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::{fd::OwnedFd, unix::prelude::OpenOptionsExt};
use std::path::Path;

use i_slint_core::api::LogicalPosition;
use i_slint_core::platform::{PlatformError, PointerEventButton, WindowEvent};
use input::LibinputInterface;

use input::event::keyboard::KeyboardEventTrait;
use input::event::touch::TouchEventPosition;
use libc::{O_RDONLY, O_RDWR, O_WRONLY};
use xkbcommon::*;

struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read((flags & O_RDONLY != 0) | (flags & O_RDWR != 0))
            .write((flags & O_WRONLY != 0) | (flags & O_RDWR != 0))
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
    mouse_pos: LogicalPosition,
    last_touch_pos: LogicalPosition,
    window: &'a i_slint_core::api::Window,
    keystate: xkb::State,
}

impl<'a> LibInputHandler<'a> {
    pub fn init<T>(
        window: &'a i_slint_core::api::Window,
        event_loop_handle: &calloop::LoopHandle<'a, T>,
    ) -> Result<(), PlatformError> {
        let mut libinput = input::Libinput::new_with_udev(Interface);
        libinput.udev_assign_seat("seat0").unwrap();

        let xkb_context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(&xkb_context, "", "", "", "", None, 0)
            .ok_or_else(|| format!("Error compiling keymap"))?;
        let keystate = xkb::State::new(&keymap);

        let handler = Self {
            libinput,
            token: Default::default(),
            mouse_pos: Default::default(),
            last_touch_pos: Default::default(),
            window,
            keystate,
        };

        event_loop_handle
            .insert_source(handler, move |_, _, _| {})
            .map_err(|e| format!("Error registering libinput event source: {e}"))?;

        Ok(())
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

        for event in &mut self.libinput {
            match event {
                input::Event::Pointer(pointer_event) => {
                    match pointer_event {
                        input::event::PointerEvent::Motion(motion_event) => {
                            self.mouse_pos.x += motion_event.dx() as f32;
                            self.mouse_pos.y += motion_event.dy() as f32;
                            self.window.dispatch_event(WindowEvent::PointerMoved {
                                position: self.mouse_pos,
                            });
                        }
                        input::event::PointerEvent::Button(button_event) => {
                            // https://github.com/torvalds/linux/blob/0dd2a6fb1e34d6dcb96806bc6b111388ad324722/include/uapi/linux/input-event-codes.h#L355
                            let button = match button_event.button() {
                                0x110 => PointerEventButton::Left,
                                0x111 => PointerEventButton::Right,
                                0x112 => PointerEventButton::Middle,
                                _ => PointerEventButton::Other,
                            };
                            let event = match button_event.button_state() {
                                input::event::tablet_pad::ButtonState::Pressed => {
                                    WindowEvent::PointerPressed { position: self.mouse_pos, button }
                                }
                                input::event::tablet_pad::ButtonState::Released => {
                                    WindowEvent::PointerReleased {
                                        position: self.mouse_pos,
                                        button,
                                    }
                                }
                            };
                            self.window.dispatch_event(event);
                        }
                        input::event::PointerEvent::ScrollWheel(_) => todo!(),
                        input::event::PointerEvent::ScrollFinger(_) => todo!(),
                        input::event::PointerEvent::ScrollContinuous(_) => todo!(),
                        _ => {}
                    }
                }
                input::Event::Touch(touch_event) => {
                    let screen_size = self.window.size();
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
                        self.window.dispatch_event(event);
                    }
                }
                input::Event::Keyboard(input::event::KeyboardEvent::Key(key_event)) => {
                    // On Linux key codes have a fixed offset of 8: https://docs.rs/xkbcommon/0.5.0/xkbcommon/xkb/type.Keycode.html
                    let key_code = key_event.key() + 8;
                    let state = key_event.key_state();

                    let sym = self.keystate.key_get_one_sym(key_code);

                    self.keystate.update_key(
                        key_code,
                        match state {
                            input::event::tablet_pad::KeyState::Pressed => xkb::KeyDirection::Down,
                            input::event::tablet_pad::KeyState::Released => xkb::KeyDirection::Up,
                        },
                    );

                    let control = self
                        .keystate
                        .mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE);
                    let alt = self
                        .keystate
                        .mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE);

                    if state == input::event::keyboard::KeyState::Pressed {
                        //eprintln!(
                        //"key {} state {:#?} sym {:x} control {control} alt {alt}",
                        //key_code, state, sym
                        //);

                        if control && alt && sym == xkb::KEY_BackSpace
                            || control && alt && sym == xkb::KEY_Delete
                        {
                            i_slint_core::api::quit_event_loop()
                                .expect("Unable to quit event loop multiple times");
                        } else {
                            if (xkb::KEY_XF86Switch_VT_1..=xkb::KEY_XF86Switch_VT_12).contains(&sym)
                            {
                                // let target_vt = (sym - xkb::KEY_XF86Switch_VT_1 + 1) as i32;
                                // TODO: eprintln!("switch vt {target_vt}");
                            }
                        }
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
        poll.register(
            self.libinput.as_raw_fd(),
            calloop::Interest::READ,
            calloop::Mode::Level,
            self.token.unwrap(),
        )
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.token = Some(token_factory.token());
        poll.reregister(
            self.libinput.as_raw_fd(),
            calloop::Interest::READ,
            calloop::Mode::Level,
            self.token.unwrap(),
        )
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> calloop::Result<()> {
        self.token = None;
        poll.unregister(self.libinput.as_raw_fd())
    }
}
