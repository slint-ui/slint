// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [WindowAdapter] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use crate::EventResult;
use crate::SharedBackendData;
use crate::drag_resize_window::{handle_cursor_move_for_resize, handle_resize};
use crate::winitwindowadapter::{WindowVisibility, WinitWindowAdapter};
use corelib::SharedString;
use corelib::graphics::euclid;
use corelib::input::{InternalKeyEvent, KeyEvent, KeyEventType, MouseEvent, TouchPhase};
use corelib::items::{ColorScheme, PointerEventButton};
use corelib::lengths::LogicalPoint;
use corelib::platform::PlatformError;
use corelib::window::*;
use i_slint_core as corelib;

#[allow(unused_imports)]
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::Key;

fn winit_touch_phase(phase: winit::event::TouchPhase) -> corelib::input::TouchPhase {
    match phase {
        winit::event::TouchPhase::Started => corelib::input::TouchPhase::Started,
        winit::event::TouchPhase::Moved => corelib::input::TouchPhase::Moved,
        winit::event::TouchPhase::Ended => corelib::input::TouchPhase::Ended,
        winit::event::TouchPhase::Cancelled => corelib::input::TouchPhase::Cancelled,
    }
}
use winit::event::PointerSource;
use winit::event_loop::ControlFlow;
use winit::window::ResizeDirection;

/// This enum captures run-time specific events that can be dispatched to the event loop in
/// addition to the winit events.
pub enum CustomEvent {
    /// Slint internal: Invoke the
    UserEvent(Box<dyn FnOnce() + Send>),
    /// Emitted from quit_event_loop with the current event loop generation
    Exit(usize),
    #[cfg(enable_accesskit)]
    Accesskit(accesskit_winit::Event),
    #[cfg(muda)]
    Muda(muda::MenuEvent),
}

impl std::fmt::Debug for CustomEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserEvent(_) => write!(f, "UserEvent"),
            Self::Exit(_) => write!(f, "Exit"),
            #[cfg(enable_accesskit)]
            Self::Accesskit(a) => write!(f, "AccessKit({a:?})"),
            #[cfg(muda)]
            Self::Muda(e) => write!(f, "Muda({e:?})"),
        }
    }
}

pub struct EventLoopState {
    shared_backend_data: Rc<SharedBackendData>,
    // last seen cursor position
    cursor_pos: LogicalPoint,
    /// Whether a *mouse* button is currently pressed. Touch input is handled
    /// separately via `process_touch_input` and does not affect this flag.
    pressed: bool,

    loop_error: Option<PlatformError>,
    current_resize_direction: Option<ResizeDirection>,

    /// Buffered mouse move event pending dispatch. Consecutive `CursorMoved`
    /// events are coalesced. Otherwise winit sends events so frequently that it can cause performance
    /// issues (see #9038 and #10912).
    pending_mouse_move: Option<(winit::window::WindowId, LogicalPoint)>,

    /// Set to true when pumping events for the shortest amount of time possible.
    pumping_events_instantly: bool,

    /// Allocates small i32 finger ids for iOS's pointer-valued touch ids.
    #[cfg(target_os = "ios")]
    touch_finger_ids: crate::ios::TouchFingerIdAllocator,

    custom_application_handler: Option<Box<dyn crate::CustomApplicationHandler>>,
}

impl EventLoopState {
    pub fn new(
        shared_backend_data: Rc<SharedBackendData>,
        custom_application_handler: Option<Box<dyn crate::CustomApplicationHandler>>,
    ) -> Self {
        Self {
            shared_backend_data,
            cursor_pos: Default::default(),
            pressed: Default::default(),
            loop_error: Default::default(),
            current_resize_direction: Default::default(),
            pending_mouse_move: Default::default(),
            pumping_events_instantly: Default::default(),
            #[cfg(target_os = "ios")]
            touch_finger_ids: Default::default(),
            custom_application_handler,
        }
    }

    /// Maps a winit finger id to the i32 finger id used by the core library.
    /// On all platforms but iOS the raw id is a small integer; iOS stores a
    /// pointer address in it, which TouchFingerIdAllocator maps to a small id.
    fn map_touch_finger_id(
        &mut self,
        finger_id: winit::event::FingerId,
        phase: corelib::input::TouchPhase,
    ) -> Option<i32> {
        #[cfg(not(target_os = "ios"))]
        {
            let _ = phase;
            Some(i32::try_from(finger_id.into_raw()).expect("winit touch id out of i32 range"))
        }
        #[cfg(target_os = "ios")]
        match phase {
            corelib::input::TouchPhase::Started | corelib::input::TouchPhase::Moved => {
                self.touch_finger_ids.id_for(finger_id.into_raw() as u64)
            }
            corelib::input::TouchPhase::Ended | corelib::input::TouchPhase::Cancelled => {
                self.touch_finger_ids.take(finger_id.into_raw() as u64)
            }
        }
    }

    /// Free graphics resources for any hidden windows. Called when quitting the event loop, to work
    /// around #8795.
    fn suspend_all_hidden_windows(&self) {
        let windows_to_suspend = self
            .shared_backend_data
            .active_windows
            .borrow()
            .values()
            .filter_map(|w| w.upgrade())
            .filter(|w| matches!(w.visibility(), WindowVisibility::Hidden))
            .collect::<Vec<_>>();
        for window in windows_to_suspend.into_iter() {
            let _ = window.suspend();
        }
    }

    /// Dispatch the buffered mouse move event, if any.
    fn flush_pending_mouse_move(&mut self) {
        if let Some((window_id, position)) = self.pending_mouse_move.take()
            && let Some(window) = self.shared_backend_data.window_by_id(window_id)
        {
            let runtime_window = WindowInner::from_pub(window.window());
            runtime_window.process_mouse_input(MouseEvent::Moved { position, touch_finger_id: 0 });
        }
    }

    /// Hand a native drag built by `WinitWindowAdapter::start_drag` to winit, now that the
    /// `ActiveEventLoop` is in hand.
    ///
    /// Falls back to the in-window drag if the native start fails, so the gesture isn't lost.
    fn start_drag_if_pending(&mut self, event_loop: &dyn ActiveEventLoop) {
        let Some(drag) = self.shared_backend_data.pending_drag.borrow_mut().take() else {
            return;
        };

        if event_loop.start_drag(drag.window_id, drag.data, &drag.actions, drag.icon).is_err()
            && let Some(window) = self.shared_backend_data.window_by_id(drag.window_id)
        {
            WindowInner::from_pub(window.window()).start_in_window_drag();
        }
    }

    /// Dispatch an incoming native drag as a `DragMove` (or `Drop`) to the item tree, using
    /// the data accumulated for `id`, and return the action a `DropArea` negotiated.
    fn dispatch_incoming_drag(
        &self,
        runtime_window: &WindowInner,
        id: winit::data_transfer::DataTransferId,
        proposed: corelib::items::DragAction,
        is_drop: bool,
    ) -> corelib::items::DragAction {
        let data = self
            .shared_backend_data
            .incoming_transfers
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or_default();
        let mut drop_event = corelib::items::DropEvent::default();
        drop_event.data = data;
        drop_event.position =
            corelib::api::LogicalPosition::new(self.cursor_pos.x, self.cursor_pos.y);
        drop_event.proposed_action = proposed;
        // The OS already negotiated the source's allowed actions, so let any DropArea choice
        // through.
        let allowed = corelib::items::AllowedDragActions { copy: true, move_: true, link: true };
        let event = if is_drop {
            MouseEvent::Drop { event: drop_event, allowed }
        } else {
            MouseEvent::DragMove { event: drop_event, allowed }
        };
        runtime_window
            .process_mouse_input(event)
            .and_then(|r| r.drag_action)
            .unwrap_or(corelib::items::DragAction::None)
    }

    /// Dispatch a `DragMove` for an incoming drag and report the resulting valid actions to the
    /// OS. Called once the payload has arrived, so `can-drop` sees the real data.
    fn dispatch_and_report_incoming(
        &self,
        event_loop: &dyn ActiveEventLoop,
        runtime_window: &WindowInner,
        id: winit::data_transfer::DataTransferId,
        proposed: corelib::items::DragAction,
    ) {
        let action = self.dispatch_incoming_drag(runtime_window, id, proposed, false);
        let _ = event_loop.set_valid_dnd_actions(id, slint_action_to_dnd(action).as_slice());
    }

    /// Whether the payload of the incoming drag `id` has arrived (via `DataTransferReceived`).
    fn has_incoming_data(&self, id: winit::data_transfer::DataTransferId) -> bool {
        self.shared_backend_data.incoming_transfers.borrow().contains_key(&id)
    }
}

/// Map a winit drag action to Slint's `DragAction`. A `None` (e.g. unknown) action becomes
/// `DragAction::None`.
fn dnd_action_to_slint(action: Option<winit::event_loop::DndAction>) -> corelib::items::DragAction {
    use corelib::items::DragAction;
    use winit::event_loop::DndAction;
    match action {
        Some(DndAction::Move) => DragAction::Move,
        Some(DndAction::Copy) => DragAction::Copy,
        Some(DndAction::Link) => DragAction::Link,
        Some(DndAction::Ask) | Some(DndAction::Private) | None => DragAction::None,
    }
}

/// The action proposed by the OS for an incoming drag, defaulting to `Copy` when the platform
/// did not supply one (some platforms, such as X11, only report the action when the drop
/// completes).
fn proposed_action_or_copy(
    action: Option<winit::event_loop::DndAction>,
) -> corelib::items::DragAction {
    let action = dnd_action_to_slint(action);
    if action == corelib::items::DragAction::None {
        corelib::items::DragAction::Copy
    } else {
        action
    }
}

/// Map a `DropArea`'s chosen action to the single valid winit drag action to report to the OS,
/// or `None` to reject the drag. Returned as an `Option` so the per-event report on the drag
/// hot path needs no allocation.
fn slint_action_to_dnd(action: corelib::items::DragAction) -> Option<winit::event_loop::DndAction> {
    use corelib::items::DragAction;
    match action {
        DragAction::Move => Some(winit::event_loop::DndAction::Move),
        DragAction::Copy => Some(winit::event_loop::DndAction::Copy),
        DragAction::Link => Some(winit::event_loop::DndAction::Link),
        DragAction::None => None,
        // `DragAction` is `#[non_exhaustive]`, so a catch-all is still required.
        #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
        _ => None,
    }
}

impl winit::application::ApplicationHandler for EventLoopState {
    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.resumed(event_loop);
        }
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if matches!(
            self.custom_application_handler.as_mut().map_or(EventResult::Propagate, |handler| {
                handler.can_create_surfaces(event_loop)
            }),
            EventResult::PreventDefault
        ) {
            return;
        }
        if let Err(err) = self.shared_backend_data.create_inactive_windows(event_loop) {
            self.loop_error = Some(err);
            event_loop.exit();
        }
    }

    #[allow(clippy::collapsible_match)]
    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.shared_backend_data.window_by_id(window_id) else {
            if let Some(handler) = self.custom_application_handler.as_mut() {
                handler.window_event(event_loop, window_id, None, None, &event);
            }
            return;
        };

        if let Some(winit_window) = window.winit_window() {
            if matches!(
                self.custom_application_handler.as_mut().map_or(
                    EventResult::Propagate,
                    |handler| handler.window_event(
                        event_loop,
                        window_id,
                        Some(&*winit_window),
                        Some(window.window()),
                        &event
                    )
                ),
                EventResult::PreventDefault
            ) {
                return;
            }

            if let Some(mut window_event_filter) = window.window_event_filter.take() {
                let event_result = window_event_filter(window.window(), &event);
                window.window_event_filter.set(Some(window_event_filter));

                match event_result {
                    EventResult::PreventDefault => return,
                    EventResult::Propagate => (),
                }
            }

            #[cfg(enable_accesskit)]
            window
                .accesskit_adapter()
                .expect("internal error: accesskit adapter must exist when window exists")
                .borrow_mut()
                .process_event(&winit_window, &event);
        } else {
            return;
        }

        let runtime_window = WindowInner::from_pub(window.window());
        self.maybe_set_custom_cursor(&window, event_loop);
        if !matches!(event, WindowEvent::PointerMoved { .. }) {
            self.flush_pending_mouse_move();
        }

        match event {
            WindowEvent::RedrawRequested => {
                self.loop_error = window.draw().err();
            }
            WindowEvent::SurfaceResized(size) => {
                self.loop_error = window.resize_event(size).err();

                // Entering fullscreen, maximizing or minimizing the window will
                // trigger a resize event. We need to update the internal window
                // state to match the actual window state. We simulate a "window
                // state event" since there is not an official event for it yet.
                // See: https://github.com/rust-windowing/winit/issues/2334
                window.window_state_event();

                // Some platforms (e.g., Windows) may not emit an Occluded event when minimized,
                // so manually mark the window as occluded if its size is zero.
                #[cfg(target_os = "windows")]
                {
                    if size.width == 0 || size.height == 0 {
                        window.renderer.occluded(true);
                    }
                }
            }
            WindowEvent::CloseRequested => {
                self.loop_error = window
                    .window()
                    .try_dispatch_event(corelib::platform::WindowEvent::CloseRequested)
                    .err();
            }
            WindowEvent::Focused(have_focus) => {
                // Work around https://github.com/rust-windowing/winit/issues/4371
                let have_focus = if cfg!(target_os = "macos") {
                    window.winit_window().map_or(have_focus, |w| w.has_focus())
                } else {
                    have_focus
                };
                self.loop_error = window.activation_changed(have_focus).err();
            }

            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                let key_code = event.logical_key.clone();
                // For now: Match Qt's behavior of mapping command to control and control to meta (LWin/RWin).
                let swap_cmd_ctrl = i_slint_core::is_apple_platform();

                let key_code = if swap_cmd_ctrl {
                    #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
                    match key_code {
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control) => {
                            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Meta)
                        }
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Meta) => {
                            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
                        }
                        code => code,
                    }
                } else {
                    key_code
                };

                fn to_slint_key(event: &winit::event::KeyEvent, key_code: &Key) -> SharedString {
                    macro_rules! winit_key_to_char {
                        ($($char:literal # $name:ident # $($shifted:ident)? $(=> $($_muda:ident)? # $($_qt:ident)|* # $($winit:ident $(($pos:ident))?)|* # $($_xkb:ident)|* )? ;)*) => {
                            #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
                            match key_code {
                                $( $( $(
                                            winit::keyboard::Key::Named(winit::keyboard::NamedKey::$winit)
                                            $(if event.location == winit::keyboard::KeyLocation::$pos)?
                                            => $char.into(),
                                )* )? )*
                                    winit::keyboard::Key::Character(str) => str.as_str().into(),
                                _ => {
                                    if let Some(text) = &event.text {
                                        text.as_str().into()
                                    } else {
                                        "".into()
                                    }
                                }
                            }
                        }
                    }
                    i_slint_common::for_each_keys!(winit_key_to_char)
                }
                #[allow(unused_mut)]
                let mut text = to_slint_key(&event, &key_code);

                #[cfg(target_os = "windows")]
                let text_without_modifiers = {
                    // On Windows, if Ctrl+Alt is pressed with a key that does not use
                    // AltGr for remapping, we need to fall back to the
                    // key_without_modifiers.
                    //
                    // See: https://github.com/rust-windowing/winit/issues/2945
                    //
                    // The text_without_modifiers also let's us disambiguate between a Ctrl+Alt
                    // combination used to imply AltGr or not.
                    // The latter case should be treated as a shortcut, the former should not.
                    let text_without_modifiers = to_slint_key(&event, &event.key_without_modifiers);
                    if text.is_empty() && !text_without_modifiers.is_empty() {
                        text = text_without_modifiers.clone();
                    }
                    text_without_modifiers
                };

                if text.is_empty() {
                    // Failed to translate the key event
                    return;
                }

                if is_synthetic {
                    // Synthetic event are sent when the focus is acquired, for all the keys currently pressed.
                    // Don't forward these keys other than modifiers to the app
                    use winit::keyboard::{Key::Named, NamedKey as N};
                    if !matches!(
                        key_code,
                        Named(N::Control | N::Shift | N::Meta | N::Alt | N::AltGraph),
                    ) {
                        return;
                    }
                }

                let event_type = match event.state {
                    winit::event::ElementState::Pressed => corelib::input::KeyEventType::KeyPressed,
                    winit::event::ElementState::Released => {
                        corelib::input::KeyEventType::KeyReleased
                    }
                };
                let mut key_event = KeyEvent::default();
                key_event.text = text;

                let event = corelib::input::InternalKeyEvent {
                    key_event,
                    event_type,
                    #[cfg(target_os = "windows")]
                    text_without_modifiers,
                    ..Default::default()
                };

                runtime_window.process_key_input(event);
            }
            WindowEvent::Ime(winit::event::Ime::Preedit(string, preedit_selection)) => {
                let event = InternalKeyEvent {
                    event_type: KeyEventType::UpdateComposition,
                    preedit_text: string.into(),
                    preedit_selection: preedit_selection.map(|e| e.0 as i32..e.1 as i32),
                    ..Default::default()
                };
                runtime_window.process_key_input(event);
            }
            WindowEvent::Ime(winit::event::Ime::Commit(string)) => {
                let mut key_event = KeyEvent::default();
                key_event.text = string.into();
                let event = InternalKeyEvent {
                    event_type: KeyEventType::CommitComposition,
                    key_event,
                    ..Default::default()
                };
                runtime_window.process_key_input(event);
            }
            WindowEvent::PointerMoved { position, source, primary, .. } => {
                let pos = position.to_logical(runtime_window.scale_factor() as f64);
                let pos = euclid::point2(pos.x, pos.y);

                if primary {
                    self.cursor_pos = pos;
                    self.current_resize_direction = handle_cursor_move_for_resize(
                        &*window.winit_window().unwrap(),
                        position,
                        self.current_resize_direction,
                        runtime_window
                            .window_item()
                            .map_or(0_f64, |w| w.as_pin_ref().resize_border_width().get().into()),
                    );
                }

                if let PointerSource::Touch { finger_id, .. } = source {
                    let phase = corelib::input::TouchPhase::Moved;
                    if let Some(finger_id) = self.map_touch_finger_id(finger_id, phase) {
                        runtime_window.process_touch_input(finger_id, pos, phase);
                    }
                } else if self.pressed {
                    // A held-button move may cross a DragArea's threshold and start a native
                    // drag. That must happen while the platform's pointer grab is still active,
                    // so dispatch it now instead of buffering (a deferred start gets cancelled).
                    self.pending_mouse_move = None;
                    runtime_window.process_mouse_input(MouseEvent::Moved {
                        position: pos,
                        touch_finger_id: 0,
                    });
                } else {
                    // winit sends this event at a very high frequency. So, bunch up consecutive
                    // cursor moved events and dispatch them as soon as any other kind of event
                    // arrives.
                    self.pending_mouse_move = Some((window_id, pos));
                }
            }
            WindowEvent::PointerLeft { kind, primary, .. } => {
                if let winit::event::PointerKind::Touch(finger_id) = kind {
                    let phase = corelib::input::TouchPhase::Cancelled;
                    if let Some(finger_id) = self.map_touch_finger_id(finger_id, phase) {
                        runtime_window.process_touch_input(finger_id, self.cursor_pos, phase);
                    }
                } else if primary {
                    // On the html canvas, we don't get the mouse move or release event outside the canvas, so we cancel the event
                    if cfg!(target_arch = "wasm32") || !self.pressed {
                        self.pressed = false;
                        runtime_window.process_mouse_input(MouseEvent::Exit);
                    }
                }
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let (delta_x, delta_y) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(lx, ly) => (lx * 60., ly * 60.),
                    winit::event::MouseScrollDelta::PixelDelta(d) => {
                        let d = d.to_logical(runtime_window.scale_factor() as f64);
                        (d.x, d.y)
                    }
                };
                let phase = winit_touch_phase(phase);
                runtime_window.process_mouse_input(MouseEvent::Wheel {
                    position: self.cursor_pos,
                    delta_x,
                    delta_y,
                    phase,
                });
            }
            WindowEvent::PointerButton { state, button, position, .. } => {
                use winit::event::{ButtonSource as S, MouseButton as B};

                let pos = position.to_logical(runtime_window.scale_factor() as f64);
                let pos = euclid::point2(pos.x, pos.y);

                let button = match button {
                    S::Mouse(B::Left) => PointerEventButton::Left,
                    S::Mouse(B::Right) => PointerEventButton::Right,
                    S::Mouse(B::Middle) => PointerEventButton::Middle,
                    S::Mouse(B::Back) => PointerEventButton::Back,
                    S::Mouse(B::Forward) => PointerEventButton::Forward,
                    S::Mouse(_) => PointerEventButton::Other,
                    S::Touch { finger_id, .. } => {
                        let phase = match state {
                            winit::event::ElementState::Pressed => TouchPhase::Started,
                            winit::event::ElementState::Released => TouchPhase::Ended,
                        };
                        if let Some(finger_id) = self.map_touch_finger_id(finger_id, phase) {
                            runtime_window.process_touch_input(finger_id, pos, phase);
                        }
                        return;
                    }
                    S::TabletTool { .. } => PointerEventButton::Other,
                    S::Unknown(_) => PointerEventButton::Other,
                };

                self.cursor_pos = pos;

                let ev = match state {
                    winit::event::ElementState::Pressed => {
                        if button == PointerEventButton::Left
                            && self.current_resize_direction.is_some()
                        {
                            handle_resize(
                                &*window.winit_window().unwrap(),
                                self.current_resize_direction,
                            );
                            return;
                        }

                        self.pressed = true;
                        MouseEvent::Pressed {
                            position: self.cursor_pos,
                            button,
                            click_count: 0,
                            touch_finger_id: 0,
                        }
                    }
                    winit::event::ElementState::Released => {
                        self.pressed = false;
                        MouseEvent::Released {
                            position: self.cursor_pos,
                            button,
                            click_count: 0,
                            touch_finger_id: 0,
                        }
                    }
                };
                runtime_window.process_mouse_input(ev);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, surface_size_writer: _ } => {
                if std::env::var("SLINT_SCALE_FACTOR").is_err() {
                    self.loop_error = window
                        .window()
                        .try_dispatch_event(corelib::platform::WindowEvent::ScaleFactorChanged {
                            scale_factor: scale_factor as f32,
                        })
                        .err();
                    // TODO: send a resize event or try to keep the logical size the same.
                    //window.resize_event(inner_size_writer.???)?;
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                window.set_color_scheme(match theme {
                    winit::window::Theme::Dark => ColorScheme::Dark,
                    winit::window::Theme::Light => ColorScheme::Light,
                });
                window.update_accent_color();
            }
            WindowEvent::Occluded(x) => {
                window.renderer.occluded(x);

                // In addition to the hack done for WindowEvent::Resize, also do it for Occluded so we handle Minimized change
                window.window_state_event();
            }
            // Note: winit's PinchGesture does not carry a position; we use the last
            // known cursor position as the best available approximation. On macOS
            // trackpads, CursorMoved events typically precede gesture events.
            WindowEvent::PinchGesture { delta, phase, .. } => {
                runtime_window.process_mouse_input(corelib::input::MouseEvent::PinchGesture {
                    position: self.cursor_pos,
                    delta: delta as f32,
                    phase: winit_touch_phase(phase),
                });
            }
            WindowEvent::RotationGesture { delta, phase, .. } => {
                // macOS/winit: positive = counterclockwise. Negate to match
                // Slint convention (positive = clockwise).
                runtime_window.process_mouse_input(corelib::input::MouseEvent::RotationGesture {
                    position: self.cursor_pos,
                    delta: -delta,
                    phase: winit_touch_phase(phase),
                });
            }
            // A native drag we started has finished. The core knows which drag is in flight, so
            // we only report the negotiated action (`None` for a cancel).
            WindowEvent::OutgoingDragDropped { action, .. } => {
                runtime_window.report_drag_finished(dnd_action_to_slint(action));
            }
            WindowEvent::OutgoingDragCanceled { .. } => {
                runtime_window.report_drag_finished(corelib::items::DragAction::None);
            }
            // An incoming native drag (maybe from another app) entered or moved over the window.
            // Fetch the data lazily, dispatch it to the DropAreas, and report which actions are
            // valid so the OS shows the right cursor.
            WindowEvent::DragEntered { id, position } => {
                // Fetch the payload. We can only tell whether a DropArea accepts it, and report
                // the valid actions to the OS, once it arrives (in `DataTransferReceived` below).
                let _ =
                    event_loop.fetch_data_transfer(id, &winit::data_transfer::TypeHint::Plaintext);
                if let Some(position) = position {
                    let pos = position.to_logical(runtime_window.scale_factor() as f64);
                    self.cursor_pos = euclid::point2(pos.x, pos.y);
                }
            }
            WindowEvent::DragPosition { id, position, proposed_action } => {
                let pos = position.to_logical(runtime_window.scale_factor() as f64);
                self.cursor_pos = euclid::point2(pos.x, pos.y);
                // Only evaluate once the payload has arrived, so `can-drop` sees the data.
                if self.has_incoming_data(id) {
                    let proposed = proposed_action_or_copy(proposed_action);
                    self.dispatch_and_report_incoming(event_loop, runtime_window, id, proposed);
                }
            }
            WindowEvent::DragDropped { id, proposed_action } => {
                let proposed = proposed_action_or_copy(proposed_action);
                self.dispatch_incoming_drag(runtime_window, id, proposed, true);
                self.shared_backend_data.incoming_transfers.borrow_mut().remove(&id);
            }
            WindowEvent::DragLeft { id } => {
                runtime_window.process_mouse_input(MouseEvent::Exit);
                self.shared_backend_data.incoming_transfers.borrow_mut().remove(&id);
            }
            WindowEvent::DataTransferReceived { id, value, .. } => {
                if let Ok(text) = value.try_as_string() {
                    self.shared_backend_data
                        .incoming_transfers
                        .borrow_mut()
                        .entry(id)
                        .or_default()
                        .set_plain_text(text.into());
                    // The payload is now available: evaluate the DropAreas at the current
                    // position and report the valid actions to the OS.
                    self.dispatch_and_report_incoming(
                        event_loop,
                        runtime_window,
                        id,
                        corelib::items::DragAction::Copy,
                    );
                }
            }
            _ => {}
        }

        // A `DragArea` may have requested a native drag above; start it now while the
        // `ActiveEventLoop` is in hand.
        self.start_drag_if_pending(event_loop);

        if self.loop_error.is_some() {
            event_loop.exit();
        }
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut()
            && matches!(handler.proxy_wake_up(event_loop), EventResult::PreventDefault)
        {
            return;
        }
        let events = std::mem::take(&mut *self.shared_backend_data.event_queue.lock().unwrap());
        for event in events {
            match event {
                CustomEvent::UserEvent(user_callback) => user_callback(),
                CustomEvent::Exit(generation) => {
                    if self
                        .shared_backend_data
                        .event_loop_generation
                        .load(std::sync::atomic::Ordering::Relaxed)
                        == generation
                    {
                        self.suspend_all_hidden_windows();
                        event_loop.exit()
                    }
                    // else ignore the event, since it's from a previous run of the event loop
                }
                #[cfg(enable_accesskit)]
                CustomEvent::Accesskit(accesskit_winit::Event { window_id, window_event }) => {
                    if let Some(window) = self.shared_backend_data.window_by_id(window_id) {
                        let deferred_action = window
                            .accesskit_adapter()
                            .expect(
                                "internal error: accesskit adapter must exist when window exists",
                            )
                            .borrow_mut()
                            .process_accesskit_event(window_event);
                        // access kit adapter not borrowed anymore, now invoke the deferred action
                        if let Some(deferred_action) = deferred_action {
                            deferred_action.invoke(window.window());
                        }
                    }
                }
                #[cfg(muda)]
                CustomEvent::Muda(event) => {
                    if let Some((window, eid, muda_type)) =
                        event.id().0.split_once('|').and_then(|(w, e)| {
                            let (e, muda_type) = e.split_once('|')?;
                            Some((
                                self.shared_backend_data.window_by_id(
                                    winit::window::WindowId::from_raw(w.parse::<usize>().ok()?),
                                )?,
                                e.parse::<usize>().ok()?,
                                muda_type.parse::<crate::muda::MudaType>().ok()?,
                            ))
                        })
                    {
                        window.muda_event(eid, muda_type);
                    };
                }
            }
        }
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: winit::event::StartCause) {
        if matches!(
            self.custom_application_handler.as_mut().map_or(EventResult::Propagate, |handler| {
                handler.new_events(event_loop, cause)
            }),
            EventResult::PreventDefault
        ) {
            return;
        }

        event_loop.set_control_flow(ControlFlow::Wait);

        corelib::platform::update_timers_and_animations();
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.flush_pending_mouse_move();

        if matches!(
            self.custom_application_handler
                .as_mut()
                .map_or(EventResult::Propagate, |handler| { handler.about_to_wait(event_loop) }),
            EventResult::PreventDefault
        ) {
            return;
        }

        if let Err(err) = self.shared_backend_data.create_inactive_windows(event_loop) {
            self.loop_error = Some(err);
        }

        if !event_loop.exiting() {
            for w in self
                .shared_backend_data
                .active_windows
                .borrow()
                .values()
                .filter_map(|w| w.upgrade())
            {
                if w.window().has_active_animations() {
                    w.request_redraw();
                }
            }
        }

        if event_loop.control_flow() == ControlFlow::Wait
            && let Some(next_timer) = corelib::platform::duration_until_next_timer_update()
        {
            event_loop.set_control_flow(ControlFlow::wait_duration(next_timer));
        }

        if self.pumping_events_instantly {
            event_loop.set_control_flow(ControlFlow::Poll);
        }
    }

    fn device_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        device_id: Option<winit::event::DeviceId>,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.device_event(event_loop, device_id, event);
        }
    }

    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.suspended(event_loop);
        }
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.destroy_surfaces(event_loop);
        }
    }

    fn memory_warning(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handler) = self.custom_application_handler.as_mut() {
            handler.memory_warning(event_loop);
        }
    }
}

impl EventLoopState {
    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[allow(unused_mut)] // mut need changes for wasm
    pub fn run(mut self) -> Result<Self, corelib::platform::PlatformError> {
        let not_running_loop_instance = self
            .shared_backend_data
            .not_running_event_loop
            .take()
            .ok_or_else(|| PlatformError::from("Nested event loops are not supported"))?;
        let mut winit_loop = not_running_loop_instance;

        cfg_if::cfg_if! {
            if #[cfg(any(target_arch = "wasm32", ios_and_friends))] {
                let shared_backend_data = self.shared_backend_data.clone();
                winit_loop
                    .run_app(self)
                    .map_err(|e| format!("Error running winit event loop: {e}"))?;
                // On wasm, run_app registers the app and returns immediately.
                // On iOS, run_app blocks until the app exits.
                Ok(Self::new(shared_backend_data, None))
            } else {
                winit::event_loop::run_on_demand::EventLoopExtRunOnDemand::run_app_on_demand(&mut winit_loop, &mut self)
                    .map_err(|e| format!("Error running winit event loop: {e}"))?;

                // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
                // Winit does not support creating multiple instances of the event loop.
                self.shared_backend_data.not_running_event_loop.replace(Some(winit_loop));

                if let Some(error) = self.loop_error {
                    return Err(error);
                }
                Ok(self)
            }
        }
    }

    /// Sets the cursor to a custom source, if it needs to be set.
    pub fn maybe_set_custom_cursor(
        &self,
        window: &WinitWindowAdapter,
        event_loop: &dyn ActiveEventLoop,
    ) {
        // If there is a new custom cursor, update it.
        let custom_cursor_source = window.custom_cursor_source.take();
        if let (Some(source), Some(winit_window)) = (custom_cursor_source, window.winit_window()) {
            if let Ok(cursor) = event_loop.create_custom_cursor(source) {
                winit_window.set_cursor(cursor.into());
            }
        }
    }

    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[cfg(all(not(target_arch = "wasm32"), not(ios_and_friends)))]
    pub fn pump_events(
        mut self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(Self, winit::event_loop::pump_events::PumpStatus), corelib::platform::PlatformError>
    {
        use winit::event_loop::pump_events::EventLoopExtPumpEvents as _;

        let not_running_loop_instance = self
            .shared_backend_data
            .not_running_event_loop
            .take()
            .ok_or_else(|| PlatformError::from("Nested event loops are not supported"))?;
        let mut winit_loop = not_running_loop_instance;

        self.pumping_events_instantly = timeout.is_some_and(|duration| duration.is_zero());

        let result = winit_loop.pump_app_events(timeout, &mut self);

        self.pumping_events_instantly = false;

        // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
        // Winit does not support creating multiple instances of the event loop.
        self.shared_backend_data.not_running_event_loop.replace(Some(winit_loop));

        if let Some(error) = self.loop_error {
            return Err(error);
        }
        Ok((self, result))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn spawn(self) -> Result<(), corelib::platform::PlatformError> {
        let not_running_loop_instance = self
            .shared_backend_data
            .not_running_event_loop
            .take()
            .ok_or_else(|| PlatformError::from("Nested event loops are not supported"))?;

        not_running_loop_instance
            .run_app(self)
            .map_err(|e| format!("Error running winit event loop: {e}"))?;

        Ok(())
    }
}
