// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [WindowAdapter] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use crate::drag_resize_window::{handle_cursor_move_for_resize, handle_resize};
use crate::winitwindowadapter::WinitWindowAdapter;
use crate::SlintUserEvent;
use crate::WinitWindowEventResult;
use corelib::api::EventLoopError;
use corelib::graphics::euclid;
use corelib::input::{KeyEvent, KeyEventType, MouseEvent};
use corelib::items::{ColorScheme, PointerEventButton};
use corelib::lengths::LogicalPoint;
use corelib::platform::PlatformError;
use corelib::window::*;
use i_slint_core as corelib;

#[cfg(not(target_family = "wasm"))]
use raw_window_handle::HasDisplayHandle;
#[allow(unused_imports)]
use std::cell::{RefCell, RefMut};
use std::rc::{Rc, Weak};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::ControlFlow;
use winit::window::ResizeDirection;
pub(crate) struct NotRunningEventLoop {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) clipboard: Rc<std::cell::RefCell<crate::clipboard::ClipboardPair>>,
    pub(crate) instance: winit::event_loop::EventLoop<SlintUserEvent>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<SlintUserEvent>,
}

impl NotRunningEventLoop {
    pub(crate) fn new(
        builder: Option<winit::event_loop::EventLoopBuilder<SlintUserEvent>>,
    ) -> Result<Self, PlatformError> {
        let mut builder =
            builder.unwrap_or_else(|| winit::event_loop::EventLoop::with_user_event());

        #[cfg(all(unix, not(target_vendor = "apple")))]
        {
            #[cfg(feature = "wayland")]
            {
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                builder.with_any_thread(true);
            }
            #[cfg(feature = "x11")]
            {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_any_thread(true);

                // Under WSL, the compositor sometimes crashes. Since we cannot reconnect after the compositor
                // was restarted, the application panics. This does not happen when using XWayland. Therefore,
                // when running under WSL, try to connect to X11 instead.
                #[cfg(feature = "wayland")]
                if std::fs::metadata("/proc/sys/fs/binfmt_misc/WSLInterop").is_ok()
                    || std::fs::metadata("/run/WSL").is_ok()
                {
                    builder.with_x11();
                }
            }
        }
        #[cfg(target_family = "windows")]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        }

        let instance =
            builder.build().map_err(|e| format!("Error initializing winit event loop: {e}"))?;
        let event_loop_proxy = instance.create_proxy();

        #[cfg(not(target_arch = "wasm32"))]
        let clipboard = crate::clipboard::create_clipboard(
            &instance
                .display_handle()
                .map_err(|display_err| PlatformError::OtherError(display_err.into()))?,
        );

        Ok(Self {
            instance,
            event_loop_proxy,
            #[cfg(not(target_family = "wasm"))]
            clipboard: Rc::new(clipboard.into()),
        })
    }
}

struct RunningEventLoop<'a> {
    active_event_loop: &'a ActiveEventLoop,
}

pub(crate) enum ActiveOrInactiveEventLoop<'a> {
    #[allow(unused)]
    Active(&'a ActiveEventLoop),
    #[allow(unused)]
    Inactive(&'a winit::event_loop::EventLoop<SlintUserEvent>),
}

pub(crate) trait EventLoopInterface {
    fn create_window(
        &self,
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<winit::window::Window, winit::error::OsError>;
    #[allow(unused)]
    fn event_loop(&self) -> ActiveOrInactiveEventLoop<'_>;
    fn is_wayland(&self) -> bool {
        false
    }
}

impl EventLoopInterface for NotRunningEventLoop {
    fn create_window(
        &self,
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<winit::window::Window, winit::error::OsError> {
        #[allow(deprecated)]
        self.instance.create_window(window_attributes)
    }
    fn event_loop(&self) -> ActiveOrInactiveEventLoop<'_> {
        ActiveOrInactiveEventLoop::Inactive(&self.instance)
    }
    #[cfg(all(unix, not(target_vendor = "apple"), feature = "wayland"))]
    fn is_wayland(&self) -> bool {
        use winit::platform::wayland::EventLoopExtWayland;
        return self.instance.is_wayland();
    }
}

impl<'a> EventLoopInterface for RunningEventLoop<'a> {
    fn create_window(
        &self,
        window_attributes: winit::window::WindowAttributes,
    ) -> Result<winit::window::Window, winit::error::OsError> {
        self.active_event_loop.create_window(window_attributes)
    }
    fn event_loop(&self) -> ActiveOrInactiveEventLoop<'_> {
        ActiveOrInactiveEventLoop::Active(self.active_event_loop)
    }
    #[cfg(all(unix, not(target_vendor = "apple"), feature = "wayland"))]
    fn is_wayland(&self) -> bool {
        use winit::platform::wayland::ActiveEventLoopExtWayland;
        return self.active_event_loop.is_wayland();
    }
}

thread_local! {
    static ALL_WINDOWS: RefCell<std::collections::HashMap<winit::window::WindowId, Weak<WinitWindowAdapter>>> = RefCell::new(std::collections::HashMap::new());
    pub(crate) static MAYBE_LOOP_INSTANCE: RefCell<Option<NotRunningEventLoop>> = RefCell::default();
}

scoped_tls_hkt::scoped_thread_local!(static CURRENT_WINDOW_TARGET : for<'a> &'a RunningEventLoop<'a>);

pub(crate) enum GlobalEventLoopProxyOrEventQueue {
    Proxy(winit::event_loop::EventLoopProxy<SlintUserEvent>),
    Queue(Vec<SlintUserEvent>),
}

impl GlobalEventLoopProxyOrEventQueue {
    pub(crate) fn send_event(&mut self, event: SlintUserEvent) -> Result<(), EventLoopError> {
        match self {
            GlobalEventLoopProxyOrEventQueue::Proxy(proxy) => {
                proxy.send_event(event).map_err(|_| EventLoopError::EventLoopTerminated)
            }
            GlobalEventLoopProxyOrEventQueue::Queue(queue) => {
                queue.push(event);
                Ok(())
            }
        }
    }

    fn set_proxy(&mut self, proxy: winit::event_loop::EventLoopProxy<SlintUserEvent>) {
        match self {
            GlobalEventLoopProxyOrEventQueue::Proxy(_) => {}
            GlobalEventLoopProxyOrEventQueue::Queue(queue) => {
                std::mem::take(queue)
                    .into_iter()
                    .for_each(|event| proxy.send_event(event).ok().unwrap());
                *self = GlobalEventLoopProxyOrEventQueue::Proxy(proxy);
            }
        }
    }
}

impl Default for GlobalEventLoopProxyOrEventQueue {
    fn default() -> Self {
        Self::Queue(Vec::new())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) static GLOBAL_PROXY: once_cell::sync::OnceCell<
    std::sync::Mutex<GlobalEventLoopProxyOrEventQueue>,
> = once_cell::sync::OnceCell::new();

#[cfg(target_arch = "wasm32")]
thread_local! {
    pub(crate) static GLOBAL_PROXY: RefCell<Option<GlobalEventLoopProxyOrEventQueue>> = RefCell::new(None)
}

pub(crate) fn with_window_target<T>(
    callback: impl FnOnce(
        &dyn EventLoopInterface,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>>,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
    if CURRENT_WINDOW_TARGET.is_set() {
        CURRENT_WINDOW_TARGET.with(|current_target| callback(current_target))
    } else {
        MAYBE_LOOP_INSTANCE.with(|loop_instance| {
            if loop_instance.borrow().is_none() {
                *loop_instance.borrow_mut() = Some(NotRunningEventLoop::new(None)?);
            }
            callback(loop_instance.borrow().as_ref().unwrap())
        })
    }
}

pub fn register_window(id: winit::window::WindowId, window: Rc<WinitWindowAdapter>) {
    ALL_WINDOWS.with(|windows| {
        windows.borrow_mut().insert(id, Rc::downgrade(&window));
    })
}

pub fn unregister_window(id: winit::window::WindowId) {
    let _ = ALL_WINDOWS.try_with(|windows| {
        windows.borrow_mut().remove(&id);
    });
}

pub fn window_by_id(id: winit::window::WindowId) -> Option<Rc<WinitWindowAdapter>> {
    ALL_WINDOWS.with(|windows| windows.borrow().get(&id).and_then(|weakref| weakref.upgrade()))
}

/// This enum captures run-time specific events that can be dispatched to the event loop in
/// addition to the winit events.
pub enum CustomEvent {
    /// On wasm request_redraw doesn't wake the event loop, so we need to manually send an event
    /// so that the event loop can run
    #[cfg(target_arch = "wasm32")]
    WakeEventLoopWorkaround,
    /// Slint internal: Invoke the
    UserEvent(Box<dyn FnOnce() + Send>),
    Exit,
    #[cfg(enable_accesskit)]
    Accesskit(accesskit_winit::Event),
    #[cfg(muda)]
    Muda(muda::MenuEvent),
}

impl std::fmt::Debug for CustomEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WakeEventLoopWorkaround => write!(f, "WakeEventLoopWorkaround"),
            Self::UserEvent(_) => write!(f, "UserEvent"),
            Self::Exit => write!(f, "Exit"),
            #[cfg(enable_accesskit)]
            Self::Accesskit(a) => write!(f, "AccessKit({a:?})"),
            #[cfg(muda)]
            Self::Muda(e) => write!(f, "Muda({e:?})"),
        }
    }
}

#[derive(Default)]
pub struct EventLoopState {
    // last seen cursor position
    cursor_pos: LogicalPoint,
    pressed: bool,
    current_touch_id: Option<u64>,

    loop_error: Option<PlatformError>,
    current_resize_direction: Option<ResizeDirection>,
}

impl winit::application::ApplicationHandler<SlintUserEvent> for EventLoopState {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        ALL_WINDOWS.with(|ws| {
            for (_, window_weak) in ws.borrow().iter() {
                if let Some(w) = window_weak.upgrade() {
                    if let Err(e) = w.ensure_window() {
                        self.loop_error = Some(e);
                    }
                }
            }
        })
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = window_by_id(window_id) else {
            return;
        };

        if let Some(_winit_window) = window.winit_window() {
            if let Some(mut window_event_filter) = window.window_event_filter.take() {
                let event_result = window_event_filter(window.window(), &event);
                window.window_event_filter.set(Some(window_event_filter));

                match event_result {
                    WinitWindowEventResult::PreventDefault => return,
                    WinitWindowEventResult::Propagate => (),
                }
            }

            #[cfg(enable_accesskit)]
            window
                .accesskit_adapter()
                .expect("internal error: accesskit adapter must exist when window exists")
                .borrow_mut()
                .process_event(&_winit_window, &event);
        } else {
            return;
        }

        let runtime_window = WindowInner::from_pub(window.window());
        match event {
            WindowEvent::RedrawRequested => {
                self.loop_error = window.draw().err();
            }
            WindowEvent::Resized(size) => {
                self.loop_error = window.resize_event(size).err();

                // Entering fullscreen, maximizing or minimizing the window will
                // trigger a resize event. We need to update the internal window
                // state to match the actual window state. We simulate a "window
                // state event" since there is not an official event for it yet.
                // Because we don't always get a Resized event (eg, minimized), also handle Occluded
                // See: https://github.com/rust-windowing/winit/issues/2334
                window.window_state_event();
            }
            WindowEvent::CloseRequested => {
                self.loop_error = window
                    .window()
                    .try_dispatch_event(corelib::platform::WindowEvent::CloseRequested)
                    .err();
            }
            WindowEvent::Focused(have_focus) => {
                self.loop_error = window.activation_changed(have_focus).err();
            }

            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                let key_code = event.logical_key;
                // For now: Match Qt's behavior of mapping command to control and control to meta (LWin/RWin).
                #[cfg(target_vendor = "apple")]
                let key_code = match key_code {
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control) => {
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Super)
                    }
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Super) => {
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
                    }
                    code => code,
                };

                macro_rules! winit_key_to_char {
                ($($char:literal # $name:ident # $($_qt:ident)|* # $($winit:ident $(($pos:ident))?)|* # $($_xkb:ident)|*;)*) => {
                    match &key_code {
                        $($(winit::keyboard::Key::Named(winit::keyboard::NamedKey::$winit) $(if event.location == winit::keyboard::KeyLocation::$pos)? => $char.into(),)*)*
                        winit::keyboard::Key::Character(str) => str.as_str().into(),
                        _ => {
                            if let Some(text) = &event.text {
                                text.as_str().into()
                            } else {
                                return;
                            }
                        }
                    }
                }
            }
                let text = i_slint_common::for_each_special_keys!(winit_key_to_char);

                self.loop_error = window
                    .window()
                    .try_dispatch_event(match event.state {
                        winit::event::ElementState::Pressed if event.repeat => {
                            corelib::platform::WindowEvent::KeyPressRepeated { text }
                        }
                        winit::event::ElementState::Pressed => {
                            if is_synthetic {
                                // Synthetic event are sent when the focus is acquired, for all the keys currently pressed.
                                // Don't forward these keys other than modifiers to the app
                                use winit::keyboard::{Key::Named, NamedKey as N};
                                if !matches!(
                                    key_code,
                                    Named(N::Control | N::Shift | N::Super | N::Alt | N::AltGraph),
                                ) {
                                    return;
                                }
                            }
                            corelib::platform::WindowEvent::KeyPressed { text }
                        }
                        winit::event::ElementState::Released => {
                            corelib::platform::WindowEvent::KeyReleased { text }
                        }
                    })
                    .err();
            }
            WindowEvent::Ime(winit::event::Ime::Preedit(string, preedit_selection)) => {
                let event = KeyEvent {
                    event_type: KeyEventType::UpdateComposition,
                    preedit_text: string.into(),
                    preedit_selection: preedit_selection.map(|e| e.0 as i32..e.1 as i32),
                    ..Default::default()
                };
                runtime_window.process_key_input(event);
            }
            WindowEvent::Ime(winit::event::Ime::Commit(string)) => {
                let event = KeyEvent {
                    event_type: KeyEventType::CommitComposition,
                    text: string.into(),
                    ..Default::default()
                };
                runtime_window.process_key_input(event);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.current_resize_direction = handle_cursor_move_for_resize(
                    &window.winit_window().unwrap(),
                    position,
                    self.current_resize_direction,
                    runtime_window
                        .window_item()
                        .map_or(0_f64, |w| w.as_pin_ref().resize_border_width().get().into()),
                );
                let position = position.to_logical(runtime_window.scale_factor() as f64);
                self.cursor_pos = euclid::point2(position.x, position.y);
                runtime_window.process_mouse_input(MouseEvent::Moved { position: self.cursor_pos });
            }
            WindowEvent::CursorLeft { .. } => {
                // On the html canvas, we don't get the mouse move or release event when outside the canvas. So we have no choice but canceling the event
                if cfg!(target_arch = "wasm32") || !self.pressed {
                    self.pressed = false;
                    runtime_window.process_mouse_input(MouseEvent::Exit);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (delta_x, delta_y) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(lx, ly) => (lx * 60., ly * 60.),
                    winit::event::MouseScrollDelta::PixelDelta(d) => {
                        let d = d.to_logical(runtime_window.scale_factor() as f64);
                        (d.x, d.y)
                    }
                };
                runtime_window.process_mouse_input(MouseEvent::Wheel {
                    position: self.cursor_pos,
                    delta_x,
                    delta_y,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => PointerEventButton::Left,
                    winit::event::MouseButton::Right => PointerEventButton::Right,
                    winit::event::MouseButton::Middle => PointerEventButton::Middle,
                    winit::event::MouseButton::Back => PointerEventButton::Back,
                    winit::event::MouseButton::Forward => PointerEventButton::Forward,
                    winit::event::MouseButton::Other(_) => PointerEventButton::Other,
                };
                let ev = match state {
                    winit::event::ElementState::Pressed => {
                        if button == PointerEventButton::Left
                            && self.current_resize_direction.is_some()
                        {
                            handle_resize(
                                &window.winit_window().unwrap(),
                                self.current_resize_direction,
                            );
                            return;
                        }

                        self.pressed = true;
                        MouseEvent::Pressed { position: self.cursor_pos, button, click_count: 0 }
                    }
                    winit::event::ElementState::Released => {
                        self.pressed = false;
                        MouseEvent::Released { position: self.cursor_pos, button, click_count: 0 }
                    }
                };
                runtime_window.process_mouse_input(ev);
            }
            WindowEvent::Touch(touch) => {
                if Some(touch.id) == self.current_touch_id || self.current_touch_id.is_none() {
                    let location = touch.location.to_logical(runtime_window.scale_factor() as f64);
                    let position = euclid::point2(location.x, location.y);
                    let ev = match touch.phase {
                        winit::event::TouchPhase::Started => {
                            self.pressed = true;
                            if self.current_touch_id.is_none() {
                                self.current_touch_id = Some(touch.id);
                            }
                            MouseEvent::Pressed {
                                position,
                                button: PointerEventButton::Left,
                                click_count: 0,
                            }
                        }
                        winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                            self.pressed = false;
                            self.current_touch_id = None;
                            MouseEvent::Released {
                                position,
                                button: PointerEventButton::Left,
                                click_count: 0,
                            }
                        }
                        winit::event::TouchPhase::Moved => MouseEvent::Moved { position },
                    };
                    runtime_window.process_mouse_input(ev);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer: _ } => {
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
            WindowEvent::ThemeChanged(theme) => window.set_color_scheme(match theme {
                winit::window::Theme::Dark => ColorScheme::Dark,
                winit::window::Theme::Light => ColorScheme::Light,
            }),
            WindowEvent::Occluded(x) => {
                window.renderer.occluded(x);

                // In addition to the hack done for WindowEvent::Resize, also do it for Occluded so we handle Minimized change
                window.window_state_event();
            }
            _ => {}
        }

        if self.loop_error.is_some() {
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: SlintUserEvent) {
        match event.0 {
            CustomEvent::UserEvent(user_callback) => user_callback(),
            CustomEvent::Exit => event_loop.exit(),
            #[cfg(enable_accesskit)]
            CustomEvent::Accesskit(accesskit_winit::Event { window_id, window_event }) => {
                if let Some(window) = window_by_id(window_id) {
                    let deferred_action = window
                        .accesskit_adapter()
                        .expect("internal error: accesskit adapter must exist when window exists")
                        .borrow_mut()
                        .process_accesskit_event(window_event);
                    // access kit adapter not borrowed anymore, now invoke the deferred action
                    if let Some(deferred_action) = deferred_action {
                        deferred_action.invoke(&window.window());
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            CustomEvent::WakeEventLoopWorkaround => {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            #[cfg(muda)]
            CustomEvent::Muda(event) => {
                if let Some((window, eid)) = event.id().0.split_once('|').and_then(|(w, e)| {
                    Some((
                        window_by_id(winit::window::WindowId::from(w.parse::<u64>().ok()?))?,
                        e.parse::<usize>().ok()?,
                    ))
                }) {
                    if let Some(ma) = window.muda_adapter.borrow().as_ref() {
                        ma.invoke(eid);
                    }
                };
            }
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        event_loop.set_control_flow(ControlFlow::Wait);

        corelib::platform::update_timers_and_animations();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !event_loop.exiting() {
            ALL_WINDOWS.with(|windows| {
                for w in windows.borrow().iter().filter_map(|(_, w)| w.upgrade()) {
                    if w.window().has_active_animations() {
                        w.request_redraw();
                    }
                }
            })
        }

        if event_loop.control_flow() == ControlFlow::Wait {
            if let Some(next_timer) = corelib::platform::duration_until_next_timer_update() {
                event_loop.set_control_flow(ControlFlow::wait_duration(next_timer));
            }
        }
    }
}

/// Wrapper around a Handler that implements the winit::application::ApplicationHandler
/// but make sure to call every function with CURRENT_WINDOW_TARGET set
struct ActiveEventLoopSetterDuringEventProcessing<Handler>(Handler);

impl<Event: 'static, Handler: winit::application::ApplicationHandler<Event>>
    winit::application::ApplicationHandler<Event>
    for ActiveEventLoopSetterDuringEventProcessing<Handler>
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET.set(&running_instance, || self.0.resumed(event_loop))
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET
            .set(&running_instance, || self.0.window_event(event_loop, window_id, event))
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET.set(&running_instance, || self.0.new_events(event_loop, cause))
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET.set(&running_instance, || self.0.user_event(event_loop, event))
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET
            .set(&running_instance, || self.0.device_event(event_loop, device_id, event))
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET.set(&running_instance, || self.0.about_to_wait(event_loop))
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET.set(&running_instance, || self.0.suspended(event_loop))
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET.set(&running_instance, || self.0.exiting(event_loop))
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        let running_instance = RunningEventLoop { active_event_loop: event_loop };
        CURRENT_WINDOW_TARGET.set(&running_instance, || self.0.memory_warning(event_loop))
    }
}

impl EventLoopState {
    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[allow(unused_mut)] // mut need changes for wasm

    pub fn run(mut self) -> Result<Self, corelib::platform::PlatformError> {
        let not_running_loop_instance = MAYBE_LOOP_INSTANCE
            .with(|loop_instance| match loop_instance.borrow_mut().take() {
                Some(instance) => Ok(instance),
                None => NotRunningEventLoop::new(None),
            })
            .map_err(|e| format!("Error initializing winit event loop: {e}"))?;

        let event_loop_proxy = not_running_loop_instance.event_loop_proxy;
        #[cfg(not(target_arch = "wasm32"))]
        GLOBAL_PROXY
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .set_proxy(event_loop_proxy.clone());
        #[cfg(target_arch = "wasm32")]
        GLOBAL_PROXY.with(|global_proxy| {
            global_proxy
                .borrow_mut()
                .get_or_insert_with(Default::default)
                .set_proxy(event_loop_proxy.clone())
        });

        let mut winit_loop = not_running_loop_instance.instance;

        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
        {
            use winit::platform::run_on_demand::EventLoopExtRunOnDemand as _;
            winit_loop
                .run_app_on_demand(&mut ActiveEventLoopSetterDuringEventProcessing(&mut self))
                .map_err(|e| format!("Error running winit event loop: {e}"))?;

            *GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap() = Default::default();

            // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
            // Winit does not support creating multiple instances of the event loop.
            let nre = NotRunningEventLoop {
                instance: winit_loop,
                event_loop_proxy,
                clipboard: not_running_loop_instance.clipboard,
            };
            MAYBE_LOOP_INSTANCE.with(|loop_instance| *loop_instance.borrow_mut() = Some(nre));

            if let Some(error) = self.loop_error {
                return Err(error);
            }
            Ok(self)
        }

        #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
        {
            winit_loop
                .run_app(&mut ActiveEventLoopSetterDuringEventProcessing(&mut self))
                .map_err(|e| format!("Error running winit event loop: {e}"))?;
            // This can't really happen, as run() doesn't return
            Ok(Self::default())
        }
    }

    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
    pub fn pump_events(
        mut self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(Self, winit::platform::pump_events::PumpStatus), corelib::platform::PlatformError>
    {
        use winit::platform::pump_events::EventLoopExtPumpEvents;

        let not_running_loop_instance = MAYBE_LOOP_INSTANCE
            .with(|loop_instance| match loop_instance.borrow_mut().take() {
                Some(instance) => Ok(instance),
                None => NotRunningEventLoop::new(None),
            })
            .map_err(|e| format!("Error initializing winit event loop: {e}"))?;

        let event_loop_proxy = not_running_loop_instance.event_loop_proxy;
        GLOBAL_PROXY
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .set_proxy(event_loop_proxy.clone());

        let mut winit_loop = not_running_loop_instance.instance;

        let result = winit_loop
            .pump_app_events(timeout, &mut ActiveEventLoopSetterDuringEventProcessing(&mut self));

        *GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap() = Default::default();

        // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
        // Winit does not support creating multiple instances of the event loop.
        let nre = NotRunningEventLoop {
            instance: winit_loop,
            event_loop_proxy,
            clipboard: not_running_loop_instance.clipboard,
        };
        MAYBE_LOOP_INSTANCE.with(|loop_instance| *loop_instance.borrow_mut() = Some(nre));

        if let Some(error) = self.loop_error {
            return Err(error);
        }
        Ok((self, result))
    }
}

#[cfg(target_arch = "wasm32")]
pub fn spawn() -> Result<(), corelib::platform::PlatformError> {
    use winit::platform::web::EventLoopExtWebSys;
    let not_running_loop_instance = MAYBE_LOOP_INSTANCE
        .with(|loop_instance| match loop_instance.borrow_mut().take() {
            Some(instance) => Ok(instance),
            None => NotRunningEventLoop::new(None),
        })
        .map_err(|e| format!("Error initializing winit event loop: {e}"))?;

    let event_loop_proxy = not_running_loop_instance.event_loop_proxy;
    GLOBAL_PROXY.with(|global_proxy| {
        global_proxy
            .borrow_mut()
            .get_or_insert_with(Default::default)
            .set_proxy(event_loop_proxy.clone())
    });

    let loop_state = EventLoopState::default();

    not_running_loop_instance
        .instance
        .spawn_app(ActiveEventLoopSetterDuringEventProcessing(loop_state));

    Ok(())
}
