// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::javahelper::{print_jni_error, JavaHelper};
use android_activity::input::{
    ButtonState, InputEvent, KeyAction, Keycode, MotionAction, MotionEvent,
};
use android_activity::{InputStatus, MainEvent, PollEvent};
use i_slint_core::api::{LogicalPosition, PhysicalPosition, PhysicalSize, PlatformError, Window};
use i_slint_core::items::ColorScheme;
use i_slint_core::platform::{
    Key, PointerEventButton, WindowAdapter, WindowEvent, WindowProperties,
};
use i_slint_core::timers::{Timer, TimerMode};
use i_slint_core::window::{InputMethodRequest, WindowInner};
use i_slint_core::{Property, SharedString};
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

struct LongPressDetection {
    _timer: Timer,
    position: LogicalPosition,
}

pub struct AndroidWindowAdapter {
    app: AndroidApp,
    pub(crate) window: Window,
    pub(crate) renderer: i_slint_renderer_skia::SkiaRenderer,
    pub(crate) event_queue: EventQueue,
    pub(crate) pending_redraw: Cell<bool>,
    pub(super) java_helper: JavaHelper,
    pub(crate) color_scheme: core::pin::Pin<Box<Property<ColorScheme>>>,
    pub(crate) fullscreen: Cell<bool>,
    /// The offset at which the Slint view is drawn in the native window (account for status bar)
    pub offset: Cell<PhysicalPosition>,

    /// Whether the cursor handle should be shown.
    /// They are shown when taping, but hidden whenever keys are pressed
    pub(crate) show_cursor_handles: Cell<bool>,

    long_press: RefCell<Option<LongPressDetection>>,
    last_pressed_state: Cell<ButtonState>,
}

impl WindowAdapter for AndroidWindowAdapter {
    fn window(&self) -> &Window {
        &self.window
    }
    fn size(&self) -> PhysicalSize {
        if self.fullscreen.get() {
            self.app.native_window().map_or_else(Default::default, |w| PhysicalSize {
                width: w.width() as u32,
                height: w.height() as u32,
            })
        } else {
            self.java_helper.get_view_rect().unwrap_or_else(|e| print_jni_error(&self.app, e)).1
        }
    }
    fn renderer(&self) -> &dyn i_slint_core::platform::Renderer {
        &self.renderer
    }

    fn request_redraw(&self) {
        self.pending_redraw.set(true);
    }

    fn update_window_properties(&self, properties: WindowProperties<'_>) {
        let f = properties.is_fullscreen();
        if self.fullscreen.replace(f) != f {
            self.resize().unwrap();
        }
    }

    fn internal(
        &self,
        _: i_slint_core::InternalToken,
    ) -> Option<&dyn i_slint_core::window::WindowAdapterInternal> {
        Some(self)
    }
}

impl i_slint_core::window::WindowAdapterInternal for AndroidWindowAdapter {
    #[cfg(feature = "native-activity")]
    fn input_method_request(&self, request: InputMethodRequest) {
        match request {
            InputMethodRequest::Enable(props) => {
                self.java_helper
                    .set_imm_data(
                        &props,
                        self.window.scale_factor(),
                        self.show_cursor_handles.get(),
                    )
                    .unwrap_or_else(|e| print_jni_error(&self.app, e));
                self.java_helper
                    .show_or_hide_soft_input(true)
                    .unwrap_or_else(|e| print_jni_error(&self.app, e));

                if let Some(focus_item) =
                    WindowInner::from_pub(&self.window).focus_item.borrow().upgrade()
                {
                    if let Some(text_input) =
                        focus_item.downcast::<i_slint_core::items::TextInput>()
                    {
                        let color = text_input.as_pin_ref().selection_background_color();
                        self.java_helper
                            .set_handle_color(color.with_alpha(1.))
                            .unwrap_or_else(|e| print_jni_error(&self.app, e));
                    }
                }
            }
            InputMethodRequest::Update(props) => {
                self.java_helper
                    .set_imm_data(
                        &props,
                        self.window.scale_factor(),
                        self.show_cursor_handles.get(),
                    )
                    .unwrap_or_else(|e| print_jni_error(&self.app, e));
            }
            InputMethodRequest::Disable => {
                self.java_helper
                    .show_or_hide_soft_input(false)
                    .unwrap_or_else(|e| print_jni_error(&self.app, e));
            }
            _ => (),
        };
    }

    #[cfg(not(feature = "native-activity"))]
    fn input_method_request(&self, request: InputMethodRequest) {
        use android_activity::input::{TextInputState, TextSpan};

        let props = match request {
            InputMethodRequest::Enable(props) => {
                self.app.show_soft_input(true);
                props
            }
            InputMethodRequest::Update(props) => props,
            InputMethodRequest::Disable => {
                self.app.hide_soft_input(true);
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

    fn color_scheme(&self) -> ColorScheme {
        self.color_scheme.as_ref().get()
    }
}

impl AndroidWindowAdapter {
    pub fn new(app: AndroidApp) -> Rc<Self> {
        let java_helper = JavaHelper::new(&app).unwrap_or_else(|e| print_jni_error(&app, e));
        let color_scheme = Box::pin(Property::new(
            match java_helper.color_scheme().unwrap_or_else(|e| print_jni_error(&app, e)) {
                0x10 => ColorScheme::Light,  // UI_MODE_NIGHT_NO(0x10)
                0x20 => ColorScheme::Dark,   // UI_MODE_NIGHT_YES(0x20)
                0x0 => ColorScheme::Unknown, // UI_MODE_NIGHT_UNDEFINED
                _ => ColorScheme::Unknown,
            },
        ));
        Rc::<Self>::new_cyclic(|w| Self {
            app,
            window: Window::new(w.clone()),
            renderer: SkiaRenderer::default(&SkiaSharedContext::default()),
            event_queue: Default::default(),
            pending_redraw: Default::default(),
            color_scheme,
            java_helper,
            fullscreen: Cell::new(false),
            offset: Default::default(),
            show_cursor_handles: Cell::new(false),
            long_press: RefCell::default(),
            last_pressed_state: Cell::new(ButtonState(0)),
        })
    }

    pub fn process_event(&self, event: &PollEvent<'_>) -> Result<ControlFlow<()>, PlatformError> {
        let queue = std::mem::take(&mut *self.event_queue.lock().unwrap());
        for e in queue {
            match e {
                Event::Quit => return Ok(ControlFlow::Break(())),
                Event::Other(o) => o(),
            }
        }
        match event {
            PollEvent::Main(MainEvent::InputAvailable) => self.process_inputs()?,
            PollEvent::Main(MainEvent::InitWindow { .. }) => {
                if let Some(w) = self.app.native_window() {
                    let size = PhysicalSize { width: w.width() as u32, height: w.height() as u32 };

                    let scale_factor =
                        self.app.config().density().map(|dpi| dpi as f32 / 160.0).unwrap_or(1.0);

                    if (scale_factor - self.window.scale_factor()).abs() > f32::EPSILON {
                        self.window
                            .try_dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor })?;
                    }

                    self.renderer.set_window_handle(
                        Arc::new(w),
                        Arc::new(DummyDisplayHandle),
                        size,
                        None,
                    )?;
                    self.resize()?;

                    // Fixes a problem for old Android versions: the soft input always prompt out on startup.
                    #[cfg(feature = "native-activity")]
                    self.java_helper
                        .show_or_hide_soft_input(false)
                        .unwrap_or_else(|e| print_jni_error(&self.app, e));
                }
            }
            PollEvent::Main(
                MainEvent::WindowResized { .. } | MainEvent::ContentRectChanged { .. },
            ) => self.resize()?,
            PollEvent::Main(MainEvent::RedrawNeeded { .. }) => {
                self.pending_redraw.set(false);
                self.do_render()?;
            }
            PollEvent::Main(MainEvent::GainedFocus) => {
                self.window.try_dispatch_event(WindowEvent::WindowActiveChanged(true))?;
            }
            PollEvent::Main(MainEvent::LostFocus) => {
                self.window.try_dispatch_event(WindowEvent::WindowActiveChanged(true))?;
            }
            PollEvent::Main(MainEvent::ConfigChanged { .. }) => {
                let scale_factor =
                    self.app.config().density().map(|dpi| dpi as f32 / 160.0).unwrap_or(1.0);

                if (scale_factor - self.window.scale_factor()).abs() > f32::EPSILON {
                    self.window
                        .try_dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor })?;
                    self.window.try_dispatch_event(WindowEvent::Resized {
                        size: self.size().to_logical(scale_factor),
                    })?;
                }
            }
            PollEvent::Main(MainEvent::Destroy) => {
                return Ok(ControlFlow::Break(()));
            }
            _ => (),
        }
        Ok(ControlFlow::Continue(()))
    }

    fn try_dispatch_key_event(&self, ev: WindowEvent) -> i_slint_core::input::KeyEventResult {
        match ev {
            WindowEvent::KeyPressed { text } => WindowInner::from_pub(&self.window)
                .process_key_input(i_slint_core::input::KeyEvent {
                    text,
                    repeat: false,
                    event_type: i_slint_core::input::KeyEventType::KeyPressed,
                    ..Default::default()
                }),
            WindowEvent::KeyPressRepeated { text } => WindowInner::from_pub(&self.window)
                .process_key_input(i_slint_core::input::KeyEvent {
                    text,
                    repeat: true,
                    event_type: i_slint_core::input::KeyEventType::KeyPressed,
                    ..Default::default()
                }),
            WindowEvent::KeyReleased { text } => WindowInner::from_pub(&self.window)
                .process_key_input(i_slint_core::input::KeyEvent {
                    text,
                    event_type: i_slint_core::input::KeyEventType::KeyReleased,
                    ..Default::default()
                }),
            _ => i_slint_core::input::KeyEventResult::EventIgnored,
        }
    }

    fn process_inputs(&self) -> Result<(), PlatformError> {
        let mut iter =
            self.app.input_events_iter().map_err(|e| PlatformError::Other(e.to_string()))?;
        loop {
            let mut result = Ok(());
            let read_input = iter.next(|event| match event {
                InputEvent::KeyEvent(key_event) => match map_key_event(key_event) {
                    Some(ev) => {
                        if self.try_dispatch_key_event(ev)
                            == i_slint_core::input::KeyEventResult::EventAccepted
                        {
                            InputStatus::Handled
                        } else {
                            InputStatus::Unhandled
                        }
                    }
                    None => InputStatus::Unhandled,
                },
                InputEvent::MotionEvent(motion_event) => match motion_event.action() {
                    MotionAction::ButtonPress => {
                        result = self.window.try_dispatch_event(WindowEvent::PointerPressed {
                            position: position_for_event(motion_event, self.offset.get())
                                .to_logical(self.window.scale_factor()),
                            button: button_for_event(motion_event, &self.last_pressed_state),
                        });
                        InputStatus::Handled
                    }
                    MotionAction::Down => {
                        let position = position_for_event(motion_event, self.offset.get())
                            .to_logical(self.window.scale_factor());
                        self.show_cursor_handles.set(true);
                        let _timer = Timer::default();
                        _timer.start(
                            TimerMode::SingleShot,
                            self.java_helper
                                .long_press_timeout()
                                .unwrap_or_else(|e| print_jni_error(&self.app, e)),
                            long_press_timeout,
                        );
                        self.long_press.replace(Some(LongPressDetection { position, _timer }));
                        result = self.window.try_dispatch_event(WindowEvent::PointerPressed {
                            position,
                            button: PointerEventButton::Left,
                        });
                        InputStatus::Handled
                    }
                    MotionAction::ButtonRelease => {
                        result = self.window.try_dispatch_event(WindowEvent::PointerReleased {
                            position: position_for_event(motion_event, self.offset.get())
                                .to_logical(self.window.scale_factor()),
                            button: button_for_event(motion_event, &self.last_pressed_state),
                        });
                        InputStatus::Handled
                    }
                    MotionAction::Up => {
                        self.long_press.take();
                        result = self
                            .window
                            .try_dispatch_event(WindowEvent::PointerReleased {
                                position: position_for_event(motion_event, self.offset.get())
                                    .to_logical(self.window.scale_factor()),
                                button: PointerEventButton::Left,
                            })
                            .and_then(|_| {
                                // Also send exit to avoid remaining hover state
                                self.window.try_dispatch_event(WindowEvent::PointerExited)
                            });

                        InputStatus::Handled
                    }
                    MotionAction::Move | MotionAction::HoverMove => {
                        let position = position_for_event(motion_event, self.offset.get())
                            .to_logical(self.window.scale_factor());
                        let mut lp = self.long_press.borrow_mut();
                        let sq = |x| x * x;
                        if lp.as_ref().map_or(false, |lp| {
                            sq(lp.position.x - position.x) + sq(lp.position.y - position.y) > 100.
                        }) {
                            *lp = None;
                        }
                        result =
                            self.window.try_dispatch_event(WindowEvent::PointerMoved { position });
                        InputStatus::Handled
                    }
                    MotionAction::Cancel | MotionAction::Outside => {
                        self.long_press.take();
                        result = self.window.try_dispatch_event(WindowEvent::PointerExited);
                        InputStatus::Handled
                    }
                    MotionAction::Scroll => todo!(),
                    MotionAction::HoverEnter | MotionAction::HoverExit => InputStatus::Unhandled,
                    _ => InputStatus::Unhandled,
                },
                InputEvent::TextEvent(state) => {
                    self.show_cursor_handles.set(false);
                    let runtime_window = WindowInner::from_pub(&self.window);
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

            result?;

            if !read_input {
                return Ok(());
            }
        }
    }

    fn resize(&self) -> Result<(), PlatformError> {
        let Some(win) = self.app.native_window() else { return Ok(()) };
        let (offset, size) = if self.fullscreen.get() {
            (
                Default::default(),
                PhysicalSize { width: win.width() as u32, height: win.height() as u32 },
            )
        } else {
            self.java_helper.get_view_rect().unwrap_or_else(|e| print_jni_error(&self.app, e))
        };

        self.window.try_dispatch_event(WindowEvent::Resized {
            size: size.to_logical(self.window.scale_factor()),
        })?;
        self.offset.set(offset);
        Ok(())
    }

    pub fn do_render(&self) -> Result<(), PlatformError> {
        if let Some(win) = self.app.native_window() {
            let o = self.offset.get();
            self.renderer.render_transformed_with_post_callback(
                0.,
                (o.x as f32, o.y as f32),
                PhysicalSize { width: win.width() as _, height: win.height() as _ },
                None,
            )?;
        }
        Ok(())
    }
}

fn long_press_timeout() {
    let Some(adaptor) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) else { return };
    let Some(current) = adaptor.long_press.take() else { return };
    if let Some(focus_item) =
        i_slint_core::window::WindowInner::from_pub(&adaptor.window).focus_item.borrow().upgrade()
    {
        if let Some(text_input) = focus_item.downcast::<i_slint_core::items::TextInput>() {
            let text_input = text_input.as_pin_ref();
            let geometry = focus_item
                .geometry()
                .translate(focus_item.map_to_window(Default::default()).to_vector());
            if !geometry.contains(i_slint_core::lengths::logical_point_from_api(current.position)) {
                return;
            };
            let (cursor, anchor) = text_input.selection_anchor_and_cursor();
            if cursor == anchor {
                let text = text_input.text();
                if text.len() > cursor && text.as_bytes()[cursor] != b'\n' {
                    let adaptor = adaptor.clone() as Rc<dyn WindowAdapter>;
                    text_input.select_word(&adaptor, &focus_item);
                }
            }
            adaptor
                .java_helper
                .show_action_menu()
                .unwrap_or_else(|e| print_jni_error(&adaptor.app, e))
        }
    };
}

fn position_for_event(motion_event: &MotionEvent, offset: PhysicalPosition) -> PhysicalPosition {
    motion_event.pointers().next().map_or_else(Default::default, |p| PhysicalPosition {
        x: p.x() as i32 - offset.x,
        y: p.y() as i32 - offset.y,
    })
}

fn button_for_event(
    motion_event: &MotionEvent,
    last_pressed_cell: &Cell<ButtonState>,
) -> PointerEventButton {
    //
    // The motion_event API has a method called action_button() which can be used to directly
    // determine the button associated with the event. However, the disadvantage of using the
    // action_button() API is that it relies on NDK 33 or higher, which implies that the output
    // application will only run on Android 13 or higher.
    //
    // This functionally equivalent method of computing the action button relies on the
    // button_state() call from the motion event, rather than action_button(). It is a bit more
    // complex than using action_button() directly, since the previous button state must be
    // tracked and used in the calculation for computing which button was toggled. However, this
    // will run on Android 12 (and possibly lower).
    //
    // See here for further discussion:
    //
    // https://stackoverflow.com/questions/75718566/amotionevent-getbuttonstate-returns-0-for-every-button-during-mouse-button-relea
    //
    let cur_pressed_state = motion_event.button_state();
    let last_pressed_state = last_pressed_cell.get();
    let toggled = match motion_event.action() {
        MotionAction::ButtonPress => {
            last_pressed_cell.set(cur_pressed_state);
            ButtonState((last_pressed_state.0 ^ cur_pressed_state.0) & cur_pressed_state.0)
        }
        MotionAction::ButtonRelease => {
            last_pressed_cell.set(cur_pressed_state);
            ButtonState((last_pressed_state.0 ^ cur_pressed_state.0) & last_pressed_state.0)
        }
        _ => ButtonState(0),
    };

    // if multiple buttons toggled, primary takes precedence, then secondary, etc.
    if toggled.primary() {
        return PointerEventButton::Left;
    }
    if toggled.secondary() {
        return PointerEventButton::Right;
    }
    if toggled.teriary() {
        return PointerEventButton::Middle;
    }
    if toggled.back() {
        return PointerEventButton::Back;
    }
    if toggled.forward() {
        return PointerEventButton::Forward;
    }
    return PointerEventButton::Other;
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
        Keycode::Back => Some(Key::Back.into()),
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

/// A dummy display handle that's Send + Sync. Required by wgpu, but harmless as
/// raw_window_handel::AndroidDisplayHandle is an empty struct.
struct DummyDisplayHandle;
impl raw_window_handle::HasDisplayHandle for DummyDisplayHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::android())
    }
}
