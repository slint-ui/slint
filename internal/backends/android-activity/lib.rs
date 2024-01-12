// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(target_os = "android"), allow(rustdoc::broken_intra_doc_links))]
#![cfg(target_os = "android")]

use android_activity::input::{
    InputEvent, KeyAction, Keycode, MotionAction, MotionEvent, TextInputState, TextSpan,
};
pub use android_activity::{self, AndroidApp};
use android_activity::{InputStatus, MainEvent, PollEvent};
use core::ops::ControlFlow;
use i_slint_core::api::{EventLoopError, PhysicalPosition, PhysicalSize, PlatformError, Window};
use i_slint_core::platform::{Key, PointerEventButton, WindowAdapter, WindowEvent};
use i_slint_core::SharedString;
use raw_window_handle::HasRawWindowHandle;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct AndroidPlatform {
    app: AndroidApp,
    window: Rc<AndroidWindowAdapter>,
    event_listener: Option<Box<dyn Fn(&PollEvent<'_>)>>,
}

impl AndroidPlatform {
    /// Instantiate a new Android backend given the [`android_activity::AndroidApp`]
    ///
    /// Pass the returned value to [`slint::platform::set_platform()`](`i_slint_core::platform::set_platform()`)
    ///
    /// # Example
    /// ```
    /// #[cfg(target_os = "android")]
    /// #[no_mangle]
    /// fn android_main(app: i_slint_backend_android_activity::AndroidApp) {
    ///     slint::platform::set_platform(Box::new(
    ///         i_slint_backend_android_activity::AndroidPlatform::new(app),
    ///     ))
    ///     .unwrap();
    ///     // ... your slint application ...
    /// }
    /// ```
    pub fn new(app: AndroidApp) -> Self {
        let slint_java_helper = SlintJavaHelper::new(&app).unwrap();
        Self {
            app: app.clone(),
            window: Rc::<AndroidWindowAdapter>::new_cyclic(|w| AndroidWindowAdapter {
                app,
                window: Window::new(w.clone()),
                renderer: i_slint_renderer_skia::SkiaRenderer::default(),
                event_queue: Default::default(),
                pending_redraw: Default::default(),
                slint_java_helper,
            }),
            event_listener: None,
        }
    }

    /// Instantiate a new Android backend given the [`android_activity::AndroidApp`]
    /// and a function to process the events.
    ///
    /// This is the same as [`AndroidPlatform::new()`], but it allow you to get notified
    /// of events.
    ///
    /// Pass the returned value to [`slint::platform::set_platform()`](`i_slint_core::platform::set_platform()`)
    ///
    /// # Example
    /// ```
    /// #[cfg(target_os = "android")]
    /// #[no_mangle]
    /// fn android_main(app: i_slint_backend_android_activity::AndroidApp) {
    ///     slint::platform::set_platform(Box::new(
    ///         i_slint_backend_android_activity::AndroidPlatform::new_with_event_listener(
    ///             app,
    ///             |event| { eprintln!("got event {event:?}") }
    ///         ),
    ///     ))
    ///     .unwrap();
    ///     // ... your slint application ...
    /// }
    /// ```
    pub fn new_with_event_listener(
        app: AndroidApp,
        listener: impl Fn(&PollEvent<'_>) + 'static,
    ) -> Self {
        let mut this = Self::new(app);
        this.event_listener = Some(Box::new(listener));
        this
    }
}

impl i_slint_core::platform::Platform for AndroidPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
    fn run_event_loop(&self) -> Result<(), PlatformError> {
        loop {
            let mut timeout = i_slint_core::platform::duration_until_next_timer_update();
            if self.window.window.has_active_animations() {
                // FIXME: we should not hardcode a value here
                let frame_duration = std::time::Duration::from_millis(10);
                timeout = Some(match timeout {
                    Some(x) => x.min(frame_duration),
                    None => frame_duration,
                })
            }
            let mut r = Ok(ControlFlow::Continue(()));
            self.app.poll_events(timeout, |e| {
                i_slint_core::platform::update_timers_and_animations();
                r = self.window.process_event(&e);
                if let Some(event_listener) = &self.event_listener {
                    event_listener(&e)
                }
            });
            if matches!(r.map_err(|e| PlatformError::from(e.to_string()))?, ControlFlow::Break(()))
            {
                return Ok(());
            }
            if self.window.pending_redraw.take() && self.app.native_window().is_some() {
                self.window.renderer.render()?;
            }
        }
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        Some(Box::new(AndroidEventLoopProxy {
            event_queue: self.window.event_queue.clone(),
            waker: self.app.create_waker(),
        }))
    }
}

enum Event {
    Quit,
    Other(Box<dyn FnOnce() + Send + 'static>),
}

type EventQueue = Arc<Mutex<Vec<Event>>>;

struct AndroidEventLoopProxy {
    event_queue: EventQueue,
    waker: android_activity::AndroidAppWaker,
}

impl i_slint_core::platform::EventLoopProxy for AndroidEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), EventLoopError> {
        self.event_queue.lock().unwrap().push(Event::Quit);
        self.waker.wake();
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), EventLoopError> {
        self.event_queue.lock().unwrap().push(Event::Other(event));
        self.waker.wake();
        Ok(())
    }
}

struct AndroidWindowAdapter {
    app: AndroidApp,
    window: Window,
    renderer: i_slint_renderer_skia::SkiaRenderer,
    event_queue: EventQueue,
    pending_redraw: Cell<bool>,
    slint_java_helper: SlintJavaHelper,
}

impl WindowAdapter for AndroidWindowAdapter {
    fn window(&self) -> &Window {
        &self.window
    }
    fn size(&self) -> PhysicalSize {
        self.app.native_window().map_or_else(Default::default, |w| PhysicalSize {
            width: w.width() as u32,
            height: w.height() as u32,
        })
    }
    fn renderer(&self) -> &dyn i_slint_core::platform::Renderer {
        &self.renderer
    }

    fn request_redraw(&self) {
        self.pending_redraw.set(true);
    }

    fn internal(
        &self,
        _: i_slint_core::InternalToken,
    ) -> Option<&dyn i_slint_core::window::WindowAdapterInternal> {
        Some(self)
    }
}

impl i_slint_core::window::WindowAdapterInternal for AndroidWindowAdapter {
    fn input_method_request(&self, request: i_slint_core::window::InputMethodRequest) {
        let props = match request {
            i_slint_core::window::InputMethodRequest::Enable(props) => {
                #[cfg(not(feature = "native-activity"))]
                self.app.show_soft_input(true);
                #[cfg(feature = "native-activity")]
                show_or_hide_soft_input(&self.slint_java_helper, &self.app, true).unwrap();
                props
            }
            i_slint_core::window::InputMethodRequest::Update(props) => props,
            i_slint_core::window::InputMethodRequest::Disable => {
                #[cfg(not(feature = "native-activity"))]
                self.app.hide_soft_input(true);
                #[cfg(feature = "native-activity")]
                show_or_hide_soft_input(&self.slint_java_helper, &self.app, false).unwrap();
                return;
            }
            _ => return,
        };
        let mut text = props.text.to_string();
        if !props.preedit_text.is_empty() {
            text.insert_str(props.preedit_offset, props.preedit_text.as_str());
        }
        self.app.set_text_input_state(TextInputState {
            text,
            selection: TextSpan {
                start: props.anchor_position.unwrap_or(props.cursor_position),
                end: props.cursor_position,
            },
            compose_region: (!props.preedit_text.is_empty()).then_some(TextSpan {
                start: props.preedit_offset,
                end: props.preedit_offset + props.preedit_text.len(),
            }),
        });
    }
}

impl AndroidWindowAdapter {
    fn process_event(&self, event: &PollEvent<'_>) -> Result<ControlFlow<()>, PlatformError> {
        match event {
            PollEvent::Wake => {
                let queue = std::mem::take(&mut *self.event_queue.lock().unwrap());
                for e in queue {
                    match e {
                        Event::Quit => return Ok(ControlFlow::Break(())),
                        Event::Other(o) => o(),
                    }
                }
            }
            PollEvent::Main(MainEvent::InputAvailable) => {
                self.process_inputs().map_err(|e| PlatformError::Other(e.to_string()))?
            }
            PollEvent::Main(MainEvent::InitWindow { .. }) => {
                if let Some(w) = self.app.native_window() {
                    let size = PhysicalSize { width: w.width() as u32, height: w.height() as u32 };

                    let scale_factor =
                        self.app.config().density().map(|dpi| dpi as f32 / 160.0).unwrap_or(1.0);

                    if (scale_factor - self.window.scale_factor()).abs() > f32::EPSILON {
                        self.window
                            .dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });
                        self.window.dispatch_event(WindowEvent::Resized {
                            size: size.to_logical(scale_factor),
                        });
                    }

                    // Safety: This is safe because the handle remains valid; the next rwh release provides `new()` without unsafe.
                    let window_handle = unsafe {
                        raw_window_handle::WindowHandle::borrow_raw(
                            w.raw_window_handle(),
                            raw_window_handle::ActiveHandle::new_unchecked(),
                        )
                    };
                    // Safety: The Android display handle is empty.
                    let display_handle = unsafe {
                        raw_window_handle::DisplayHandle::borrow_raw(
                            raw_window_handle::RawDisplayHandle::Android(
                                raw_window_handle::AndroidDisplayHandle::empty(),
                            ),
                        )
                    };
                    self.renderer.set_window_handle(window_handle, display_handle, size)?;
                }
            }
            PollEvent::Main(
                MainEvent::WindowResized { .. } | MainEvent::ContentRectChanged { .. },
            ) => {
                let size = self.size().to_logical(self.window.scale_factor());
                self.window.dispatch_event(WindowEvent::Resized { size })
            }
            PollEvent::Main(MainEvent::RedrawNeeded { .. }) => {
                self.pending_redraw.set(false);
                self.renderer.render()?;
            }
            PollEvent::Main(MainEvent::GainedFocus) => {
                self.window.dispatch_event(WindowEvent::WindowActiveChanged(true));
            }
            PollEvent::Main(MainEvent::LostFocus) => {
                self.window.dispatch_event(WindowEvent::WindowActiveChanged(true));
            }
            PollEvent::Main(MainEvent::ConfigChanged { .. }) => {
                let scale_factor =
                    self.app.config().density().map(|dpi| dpi as f32 / 160.0).unwrap_or(1.0);

                if (scale_factor - self.window.scale_factor()).abs() > f32::EPSILON {
                    self.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });
                    self.window.dispatch_event(WindowEvent::Resized {
                        size: self.size().to_logical(scale_factor),
                    });
                }
            }
            _ => (),
        }
        Ok(ControlFlow::Continue(()))
    }

    fn process_inputs(&self) -> Result<(), android_activity::error::AppError> {
        let mut iter = self.app.input_events_iter()?;

        loop {
            let read_input = iter.next(|event| match event {
                InputEvent::KeyEvent(key_event) => match map_key_event(key_event) {
                    Some(ev) => {
                        self.window.dispatch_event(ev);
                        InputStatus::Handled
                    }
                    None => InputStatus::Unhandled,
                },
                InputEvent::MotionEvent(motion_event) => match motion_event.action() {
                    MotionAction::Down | MotionAction::ButtonPress | MotionAction::PointerDown => {
                        self.window.dispatch_event(WindowEvent::PointerPressed {
                            position: position_for_event(motion_event)
                                .to_logical(self.window.scale_factor()),
                            button: PointerEventButton::Left,
                        });
                        InputStatus::Handled
                    }
                    MotionAction::ButtonRelease | MotionAction::PointerUp => {
                        self.window.dispatch_event(WindowEvent::PointerReleased {
                            position: position_for_event(motion_event)
                                .to_logical(self.window.scale_factor()),
                            button: PointerEventButton::Left,
                        });
                        InputStatus::Handled
                    }
                    MotionAction::Up => {
                        self.window.dispatch_event(WindowEvent::PointerReleased {
                            position: position_for_event(motion_event)
                                .to_logical(self.window.scale_factor()),
                            button: PointerEventButton::Left,
                        });
                        // Also send exit to avoid remaining hover state
                        self.window.dispatch_event(WindowEvent::PointerExited);
                        InputStatus::Handled
                    }
                    MotionAction::Move | MotionAction::HoverMove => {
                        self.window.dispatch_event(WindowEvent::PointerMoved {
                            position: position_for_event(motion_event)
                                .to_logical(self.window.scale_factor()),
                        });
                        InputStatus::Handled
                    }
                    MotionAction::Cancel | MotionAction::Outside => {
                        self.window.dispatch_event(WindowEvent::PointerExited);
                        InputStatus::Handled
                    }
                    MotionAction::Scroll => todo!(),
                    MotionAction::HoverEnter | MotionAction::HoverExit => InputStatus::Unhandled,
                    _ => InputStatus::Unhandled,
                },
                InputEvent::TextEvent(state) => {
                    let runtime_window = i_slint_core::window::WindowInner::from_pub(&self.window);
                    // remove the pre_edit
                    let event = if let Some(r) = state.compose_region {
                        let adjust =
                            |pos| if pos > r.start { pos - r.start + r.end } else { pos } as i32;
                        i_slint_core::input::KeyEvent {
                            event_type: i_slint_core::input::KeyEventType::UpdateComposition,
                            text: i_slint_core::format!(
                                "{}{}",
                                &state.text[..r.start],
                                &state.text[r.end..]
                            ),
                            preedit_text: state.text[r.start..r.end].into(),
                            preedit_selection: Some(0..(r.end - r.start) as i32),
                            replacement_range: Some(i32::MIN..i32::MAX),
                            cursor_position: Some(adjust(state.selection.end)),
                            anchor_position: Some(adjust(state.selection.start)),
                            ..Default::default()
                        }
                    } else {
                        i_slint_core::input::KeyEvent {
                            event_type: i_slint_core::input::KeyEventType::CommitComposition,
                            text: state.text.as_str().into(),
                            replacement_range: Some(i32::MIN..i32::MAX),
                            cursor_position: Some(state.selection.end as _),
                            anchor_position: Some(state.selection.start as _),
                            ..Default::default()
                        }
                    };
                    runtime_window.process_key_input(event);
                    InputStatus::Handled
                }
                _ => InputStatus::Unhandled,
            });

            if !read_input {
                return Ok(());
            }
        }
    }
}

fn position_for_event(motion_event: &MotionEvent) -> PhysicalPosition {
    motion_event
        .pointers()
        .next()
        .map_or_else(Default::default, |p| PhysicalPosition { x: p.x() as i32, y: p.y() as i32 })
}

fn map_key_event(key_event: &android_activity::input::KeyEvent) -> Option<WindowEvent> {
    let text = map_key_code(key_event.key_code())?;
    let repeat = key_event.repeat_count() > 0;
    match key_event.action() {
        KeyAction::Down if repeat => Some(WindowEvent::KeyPressRepeated { text }),
        KeyAction::Down => Some(WindowEvent::KeyPressed { text }),
        KeyAction::Up => Some(WindowEvent::KeyReleased { text }),
        KeyAction::Multiple if repeat => Some(WindowEvent::KeyPressRepeated { text }),
        KeyAction::Multiple => Some(WindowEvent::KeyPressed { text }),
        _ => None,
    }
}

fn map_key_code(code: android_activity::input::Keycode) -> Option<SharedString> {
    match code {
        Keycode::Unknown => None,
        Keycode::SoftLeft => None,
        Keycode::SoftRight => None,
        Keycode::Home => None,
        Keycode::Back => None,
        Keycode::Call => None,
        Keycode::Endcall => None,
        Keycode::Keycode0 => Some("0".into()),
        Keycode::Keycode1 => Some("1".into()),
        Keycode::Keycode2 => Some("2".into()),
        Keycode::Keycode3 => Some("3".into()),
        Keycode::Keycode4 => Some("4".into()),
        Keycode::Keycode5 => Some("5".into()),
        Keycode::Keycode6 => Some("6".into()),
        Keycode::Keycode7 => Some("7".into()),
        Keycode::Keycode8 => Some("8".into()),
        Keycode::Keycode9 => Some("9".into()),
        Keycode::Star => Some("*".into()),
        Keycode::Pound => Some("#".into()),
        Keycode::DpadUp => Some(Key::UpArrow.into()),
        Keycode::DpadDown => Some(Key::DownArrow.into()),
        Keycode::DpadLeft => Some(Key::LeftArrow.into()),
        Keycode::DpadRight => Some(Key::RightArrow.into()),
        Keycode::DpadCenter => Some(Key::Return.into()),
        Keycode::VolumeUp => None,
        Keycode::VolumeDown => None,
        Keycode::Power => None,
        Keycode::Camera => None,
        Keycode::Clear => None,
        Keycode::A => Some("a".into()),
        Keycode::B => Some("b".into()),
        Keycode::C => Some("c".into()),
        Keycode::D => Some("d".into()),
        Keycode::E => Some("e".into()),
        Keycode::F => Some("f".into()),
        Keycode::G => Some("g".into()),
        Keycode::H => Some("h".into()),
        Keycode::I => Some("i".into()),
        Keycode::J => Some("j".into()),
        Keycode::K => Some("k".into()),
        Keycode::L => Some("l".into()),
        Keycode::M => Some("m".into()),
        Keycode::N => Some("n".into()),
        Keycode::O => Some("o".into()),
        Keycode::P => Some("p".into()),
        Keycode::Q => Some("q".into()),
        Keycode::R => Some("r".into()),
        Keycode::S => Some("s".into()),
        Keycode::T => Some("t".into()),
        Keycode::U => Some("u".into()),
        Keycode::V => Some("v".into()),
        Keycode::W => Some("w".into()),
        Keycode::X => Some("x".into()),
        Keycode::Y => Some("y".into()),
        Keycode::Z => Some("z".into()),
        Keycode::Comma => Some(",".into()),
        Keycode::Period => Some(".".into()),
        Keycode::AltLeft => Some(Key::Alt.into()),
        Keycode::AltRight => Some(Key::AltGr.into()),
        Keycode::ShiftLeft => Some(Key::Shift.into()),
        Keycode::ShiftRight => Some(Key::ShiftR.into()),
        Keycode::Tab => Some("\t".into()),
        Keycode::Space => Some(" ".into()),
        Keycode::Sym => None,
        Keycode::Explorer => None,
        Keycode::Envelope => None,
        Keycode::Enter => Some(Key::Return.into()),
        Keycode::Del => Some(Key::Backspace.into()),
        Keycode::Grave => Some("`".into()),
        Keycode::Minus => Some("-".into()),
        Keycode::Equals => Some("=".into()),
        Keycode::LeftBracket => Some("[".into()),
        Keycode::RightBracket => Some("]".into()),
        Keycode::Backslash => Some("\\".into()),
        Keycode::Semicolon => Some(";".into()),
        Keycode::Apostrophe => Some("'".into()),
        Keycode::Slash => Some("/".into()),
        Keycode::At => Some("@".into()),
        Keycode::Num => None,
        Keycode::Headsethook => None,
        Keycode::Focus => None,
        Keycode::Plus => Some("+".into()),
        Keycode::Menu => Some(Key::Menu.into()),
        Keycode::Notification => None,
        Keycode::Search => None,
        Keycode::MediaPlayPause => None,
        Keycode::MediaStop => None,
        Keycode::MediaNext => None,
        Keycode::MediaPrevious => None,
        Keycode::MediaRewind => None,
        Keycode::MediaFastForward => None,
        Keycode::Mute => None,
        Keycode::PageUp => Some(Key::PageUp.into()),
        Keycode::PageDown => Some(Key::PageDown.into()),
        Keycode::Pictsymbols => None,
        Keycode::SwitchCharset => None,
        Keycode::ButtonA => None,
        Keycode::ButtonB => None,
        Keycode::ButtonC => None,
        Keycode::ButtonX => None,
        Keycode::ButtonY => None,
        Keycode::ButtonZ => None,
        Keycode::ButtonL1 => None,
        Keycode::ButtonR1 => None,
        Keycode::ButtonL2 => None,
        Keycode::ButtonR2 => None,
        Keycode::ButtonThumbl => None,
        Keycode::ButtonThumbr => None,
        Keycode::ButtonStart => None,
        Keycode::ButtonSelect => None,
        Keycode::ButtonMode => None,
        Keycode::Escape => Some(Key::Escape.into()),
        Keycode::ForwardDel => Some(Key::Delete.into()),
        Keycode::CtrlLeft => Some(Key::Control.into()),
        Keycode::CtrlRight => Some(Key::ControlR.into()),
        Keycode::CapsLock => None,
        Keycode::ScrollLock => Some(Key::ScrollLock.into()),
        Keycode::MetaLeft => Some(Key::Meta.into()),
        Keycode::MetaRight => Some(Key::MetaR.into()),
        Keycode::Function => None,
        Keycode::Sysrq => Some(Key::SysReq.into()),
        Keycode::Break => None,
        Keycode::MoveHome => Some(Key::Home.into()),
        Keycode::MoveEnd => Some(Key::End.into()),
        Keycode::Insert => Some(Key::Insert.into()),
        Keycode::Forward => None,
        Keycode::MediaPlay => None,
        Keycode::MediaPause => None,
        Keycode::MediaClose => None,
        Keycode::MediaEject => None,
        Keycode::MediaRecord => None,
        Keycode::F1 => Some(Key::F1.into()),
        Keycode::F2 => Some(Key::F2.into()),
        Keycode::F3 => Some(Key::F3.into()),
        Keycode::F4 => Some(Key::F4.into()),
        Keycode::F5 => Some(Key::F5.into()),
        Keycode::F6 => Some(Key::F6.into()),
        Keycode::F7 => Some(Key::F7.into()),
        Keycode::F8 => Some(Key::F8.into()),
        Keycode::F9 => Some(Key::F9.into()),
        Keycode::F10 => Some(Key::F10.into()),
        Keycode::F11 => Some(Key::F11.into()),
        Keycode::F12 => Some(Key::F12.into()),
        Keycode::NumLock => None,
        Keycode::Numpad0 => Some("0".into()),
        Keycode::Numpad1 => Some("1".into()),
        Keycode::Numpad2 => Some("2".into()),
        Keycode::Numpad3 => Some("3".into()),
        Keycode::Numpad4 => Some("4".into()),
        Keycode::Numpad5 => Some("5".into()),
        Keycode::Numpad6 => Some("6".into()),
        Keycode::Numpad7 => Some("7".into()),
        Keycode::Numpad8 => Some("8".into()),
        Keycode::Numpad9 => Some("9".into()),
        Keycode::NumpadDivide => Some("/".into()),
        Keycode::NumpadMultiply => Some("*".into()),
        Keycode::NumpadSubtract => Some("-".into()),
        Keycode::NumpadAdd => Some("+".into()),
        Keycode::NumpadDot => Some(".".into()),
        Keycode::NumpadComma => Some(",".into()),
        Keycode::NumpadEnter => Some("\n".into()),
        Keycode::NumpadEquals => Some("=".into()),
        Keycode::NumpadLeftParen => Some("(".into()),
        Keycode::NumpadRightParen => Some(")".into()),
        Keycode::VolumeMute => None,
        Keycode::Info => None,
        Keycode::ChannelUp => None,
        Keycode::ChannelDown => None,
        Keycode::ZoomIn => None,
        Keycode::ZoomOut => None,
        Keycode::Tv => None,
        Keycode::Window => None,
        Keycode::Guide => None,
        Keycode::Dvr => None,
        Keycode::Bookmark => None,
        Keycode::Captions => None,
        Keycode::Settings => None,
        Keycode::TvPower => None,
        Keycode::TvInput => None,
        Keycode::StbPower => None,
        Keycode::StbInput => None,
        Keycode::AvrPower => None,
        Keycode::AvrInput => None,
        Keycode::ProgRed => None,
        Keycode::ProgGreen => None,
        Keycode::ProgYellow => None,
        Keycode::ProgBlue => None,
        Keycode::AppSwitch => None,
        Keycode::Button1 => None,
        Keycode::Button2 => None,
        Keycode::Button3 => None,
        Keycode::Button4 => None,
        Keycode::Button5 => None,
        Keycode::Button6 => None,
        Keycode::Button7 => None,
        Keycode::Button8 => None,
        Keycode::Button9 => None,
        Keycode::Button10 => None,
        Keycode::Button11 => None,
        Keycode::Button12 => None,
        Keycode::Button13 => None,
        Keycode::Button14 => None,
        Keycode::Button15 => None,
        Keycode::Button16 => None,
        Keycode::LanguageSwitch => None,
        Keycode::MannerMode => None,
        Keycode::Keycode3dMode => None,
        Keycode::Contacts => None,
        Keycode::Calendar => None,
        Keycode::Music => None,
        Keycode::Calculator => None,
        Keycode::ZenkakuHankaku => None,
        Keycode::Eisu => None,
        Keycode::Muhenkan => None,
        Keycode::Henkan => None,
        Keycode::KatakanaHiragana => None,
        Keycode::Yen => None,
        Keycode::Ro => None,
        Keycode::Kana => None,
        Keycode::Assist => None,
        Keycode::BrightnessDown => None,
        Keycode::BrightnessUp => None,
        Keycode::MediaAudioTrack => None,
        Keycode::Sleep => None,
        Keycode::Wakeup => None,
        Keycode::Pairing => None,
        Keycode::MediaTopMenu => None,
        Keycode::Keycode11 => None,
        Keycode::Keycode12 => None,
        Keycode::LastChannel => None,
        Keycode::TvDataService => None,
        Keycode::VoiceAssist => None,
        Keycode::TvRadioService => None,
        Keycode::TvTeletext => None,
        Keycode::TvNumberEntry => None,
        Keycode::TvTerrestrialAnalog => None,
        Keycode::TvTerrestrialDigital => None,
        Keycode::TvSatellite => None,
        Keycode::TvSatelliteBs => None,
        Keycode::TvSatelliteCs => None,
        Keycode::TvSatelliteService => None,
        Keycode::TvNetwork => None,
        Keycode::TvAntennaCable => None,
        Keycode::TvInputHdmi1 => None,
        Keycode::TvInputHdmi2 => None,
        Keycode::TvInputHdmi3 => None,
        Keycode::TvInputHdmi4 => None,
        Keycode::TvInputComposite1 => None,
        Keycode::TvInputComposite2 => None,
        Keycode::TvInputComponent1 => None,
        Keycode::TvInputComponent2 => None,
        Keycode::TvInputVga1 => None,
        Keycode::TvAudioDescription => None,
        Keycode::TvAudioDescriptionMixUp => None,
        Keycode::TvAudioDescriptionMixDown => None,
        Keycode::TvZoomMode => None,
        Keycode::TvContentsMenu => None,
        Keycode::TvMediaContextMenu => None,
        Keycode::TvTimerProgramming => None,
        Keycode::Help => None,
        Keycode::NavigatePrevious => None,
        Keycode::NavigateNext => None,
        Keycode::NavigateIn => None,
        Keycode::NavigateOut => None,
        Keycode::StemPrimary => None,
        Keycode::Stem1 => None,
        Keycode::Stem2 => None,
        Keycode::Stem3 => None,
        Keycode::DpadUpLeft => None,
        Keycode::DpadDownLeft => None,
        Keycode::DpadUpRight => None,
        Keycode::DpadDownRight => None,
        Keycode::MediaSkipForward => None,
        Keycode::MediaSkipBackward => None,
        Keycode::MediaStepForward => None,
        Keycode::MediaStepBackward => None,
        Keycode::SoftSleep => None,
        Keycode::Cut => None,
        Keycode::Copy => None,
        Keycode::Paste => None,
        Keycode::SystemNavigationUp => None,
        Keycode::SystemNavigationDown => None,
        Keycode::SystemNavigationLeft => None,
        Keycode::SystemNavigationRight => None,
        Keycode::AllApps => None,
        Keycode::Refresh => None,
        Keycode::ThumbsUp => None,
        Keycode::ThumbsDown => None,
        Keycode::ProfileSwitch => None,
        _ => None,
    }
}

struct SlintJavaHelper(#[cfg(feature = "native-activity")] jni::objects::GlobalRef);

impl SlintJavaHelper {
    fn new(_app: &AndroidApp) -> Result<Self, jni::errors::Error> {
        Ok(Self(
            #[cfg(feature = "native-activity")]
            load_java_helper(_app)?,
        ))
    }
}

#[cfg(feature = "native-activity")]
/// Unfortunately, the way that the android-activity crate uses to show or hide the virtual keyboard doesn't
/// work with native-activity. So do it manually with JNI
fn show_or_hide_soft_input(
    helper: &SlintJavaHelper,
    app: &AndroidApp,
    show: bool,
) -> Result<(), jni::errors::Error> {
    // Safety: as documented in android-activity to obtain a jni::JavaVM
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _) }?;
    let mut env = vm.attach_current_thread()?;
    let helper = helper.0.as_obj();
    if show {
        env.call_method(helper, "show_keyboard", "()V", &[])?;
    } else {
        env.call_method(helper, "hide_keyboard", "()V", &[])?;
    };
    Ok(())
}

#[cfg(feature = "native-activity")]
fn load_java_helper(app: &AndroidApp) -> Result<jni::objects::GlobalRef, jni::errors::Error> {
    use jni::objects::{JObject, JValue};
    // Safety: as documented in android-activity to obtain a jni::JavaVM
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _) }?;
    let native_activity = unsafe { JObject::from_raw(app.activity_as_ptr() as *mut _) };

    let mut env = vm.attach_current_thread()?;

    let dex_data = include_bytes!(concat!(env!("OUT_DIR"), "/classes.dex"));

    // Safety: dex_data is 'static and the InMemoryDexClassLoader will not mutate it it
    let dex_buffer =
        unsafe { env.new_direct_byte_buffer(dex_data.as_ptr() as *mut _, dex_data.len()).unwrap() };

    let dex_loader = env.new_object(
        "dalvik/system/InMemoryDexClassLoader",
        "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
        &[JValue::Object(&dex_buffer), JValue::Object(&JObject::null())],
    )?;

    let class_name = env.new_string("SlintAndroidJavaHelper").unwrap();
    let helper_class = env
        .call_method(
            dex_loader,
            "findClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&class_name)],
        )?
        .l()?;

    let helper_class: jni::objects::JClass = helper_class.into();
    let helper_instance = env.new_object(
        helper_class,
        "(Landroid/app/Activity;)V",
        &[JValue::Object(&native_activity)],
    )?;
    Ok(env.new_global_ref(&helper_instance)?)
}
