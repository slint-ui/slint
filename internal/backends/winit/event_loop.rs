// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [WindowAdapter] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use crate::winitwindowadapter::WinitWindowAdapter;
use crate::SlintUserEvent;
#[cfg(not(target_arch = "wasm32"))]
use copypasta::ClipboardProvider;
use corelib::api::EventLoopError;
use corelib::graphics::euclid;
use corelib::input::{KeyEvent, KeyEventType, MouseEvent};
use corelib::items::{ColorScheme, PointerEventButton};
use corelib::lengths::LogicalPoint;
use corelib::platform::PlatformError;
use corelib::window::*;
use i_slint_core as corelib;
#[allow(unused_imports)]
use std::cell::{RefCell, RefMut};
use std::rc::{Rc, Weak};
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;

#[cfg(not(target_arch = "wasm32"))]
/// The Default, and the selection clippoard
type ClipboardPair = (Box<dyn ClipboardProvider>, Box<dyn ClipboardProvider>);

struct NotRunningEventLoop {
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: RefCell<ClipboardPair>,
    instance: winit::event_loop::EventLoop<SlintUserEvent>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<SlintUserEvent>,
}

impl NotRunningEventLoop {
    fn new() -> Result<Self, PlatformError> {
        let mut builder = winit::event_loop::EventLoopBuilder::with_user_event();

        #[cfg(all(unix, not(target_os = "macos")))]
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
        Ok(Self {
            #[cfg(not(target_arch = "wasm32"))]
            clipboard: RefCell::new(create_clipboard(&instance)),
            instance,
            event_loop_proxy,
        })
    }
}

struct RunningEventLoop<'a> {
    event_loop_target: &'a winit::event_loop::EventLoopWindowTarget<SlintUserEvent>,
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: &'a RefCell<ClipboardPair>,
}

pub(crate) trait EventLoopInterface {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<SlintUserEvent>;
    #[cfg(not(target_arch = "wasm32"))]
    fn clipboard(
        &self,
        _: i_slint_core::platform::Clipboard,
    ) -> Option<RefMut<'_, dyn ClipboardProvider>>;
}

impl EventLoopInterface for NotRunningEventLoop {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<SlintUserEvent> {
        &self.instance
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn clipboard(
        &self,
        clipboard: i_slint_core::platform::Clipboard,
    ) -> Option<RefMut<'_, dyn ClipboardProvider>> {
        match clipboard {
            corelib::platform::Clipboard::DefaultClipboard => {
                Some(RefMut::map(self.clipboard.borrow_mut(), |p| p.0.as_mut()))
            }
            corelib::platform::Clipboard::SelectionClipboard => {
                Some(RefMut::map(self.clipboard.borrow_mut(), |p| p.1.as_mut()))
            }
            _ => None,
        }
    }
}

impl<'a> EventLoopInterface for RunningEventLoop<'a> {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<SlintUserEvent> {
        self.event_loop_target
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn clipboard(
        &self,
        clipboard: i_slint_core::platform::Clipboard,
    ) -> Option<RefMut<'_, dyn ClipboardProvider>> {
        match clipboard {
            corelib::platform::Clipboard::DefaultClipboard => {
                Some(RefMut::map(self.clipboard.borrow_mut(), |p| p.0.as_mut()))
            }
            corelib::platform::Clipboard::SelectionClipboard => {
                Some(RefMut::map(self.clipboard.borrow_mut(), |p| p.1.as_mut()))
            }
            _ => None,
        }
    }
}

thread_local! {
    static ALL_WINDOWS: RefCell<std::collections::HashMap<winit::window::WindowId, Weak<WinitWindowAdapter>>> = RefCell::new(std::collections::HashMap::new());
    static MAYBE_LOOP_INSTANCE: RefCell<Option<NotRunningEventLoop>> = RefCell::default();
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
                *loop_instance.borrow_mut() = Some(NotRunningEventLoop::new()?);
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
}

impl std::fmt::Debug for CustomEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WakeEventLoopWorkaround => write!(f, "WakeEventLoopWorkaround"),
            Self::UserEvent(_) => write!(f, "UserEvent"),
            Self::Exit => write!(f, "Exit"),
        }
    }
}

#[derive(Default)]
pub struct EventLoopState {
    // last seen cursor position
    cursor_pos: LogicalPoint,
    pressed: bool,

    loop_error: Option<PlatformError>,
}

impl EventLoopState {
    fn process_window_event(&mut self, window: Rc<WinitWindowAdapter>, event: WindowEvent) {
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
                window.window().dispatch_event(corelib::platform::WindowEvent::CloseRequested);
            }
            WindowEvent::Focused(have_focus) => {
                let have_focus = have_focus || window.input_method_focused();
                // We don't render popups as separate windows yet, so treat
                // focus to be the same as being active.
                if have_focus != runtime_window.active() {
                    window.window().dispatch_event(
                        corelib::platform::WindowEvent::WindowActiveChanged(have_focus),
                    );
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let key_code = event.logical_key;
                // For now: Match Qt's behavior of mapping command to control and control to meta (LWin/RWin).
                #[cfg(target_os = "macos")]
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
                    match key_code {
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

                window.window().dispatch_event(match event.state {
                    winit::event::ElementState::Pressed if event.repeat => {
                        corelib::platform::WindowEvent::KeyPressRepeated { text }
                    }
                    winit::event::ElementState::Pressed => {
                        corelib::platform::WindowEvent::KeyPressed { text }
                    }
                    winit::event::ElementState::Released => {
                        corelib::platform::WindowEvent::KeyReleased { text }
                    }
                });
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
                    winit::event::MouseButton::Other(_) => PointerEventButton::Other,
                    winit::event::MouseButton::Back => PointerEventButton::Other,
                    winit::event::MouseButton::Forward => PointerEventButton::Other,
                };
                let ev = match state {
                    winit::event::ElementState::Pressed => {
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
                let location = touch.location.to_logical(runtime_window.scale_factor() as f64);
                let position = euclid::point2(location.x, location.y);
                let ev = match touch.phase {
                    winit::event::TouchPhase::Started => {
                        self.pressed = true;
                        MouseEvent::Pressed {
                            position,
                            button: PointerEventButton::Left,
                            click_count: 0,
                        }
                    }
                    winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                        self.pressed = false;
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
            WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer: _ } => {
                if std::env::var("SLINT_SCALE_FACTOR").is_err() {
                    window.window().dispatch_event(
                        corelib::platform::WindowEvent::ScaleFactorChanged {
                            scale_factor: scale_factor as f32,
                        },
                    );
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
    }

    fn process_event(
        &mut self,
        event: Event<SlintUserEvent>,
        event_loop_target: &EventLoopWindowTarget<SlintUserEvent>,
    ) {
        use winit::event_loop::ControlFlow;

        match event {
            Event::WindowEvent { event, window_id } => {
                if let Some(window) = window_by_id(window_id) {
                    #[cfg(enable_accesskit)]
                    window.accesskit_adapter.process_event(&window.winit_window(), &event);
                    self.process_window_event(window, event);
                };
            }

            Event::UserEvent(SlintUserEvent::CustomEvent { event: CustomEvent::Exit }) => {
                event_loop_target.exit();
            }

            Event::UserEvent(SlintUserEvent::CustomEvent {
                event: CustomEvent::UserEvent(user),
            }) => {
                user();
            }

            #[cfg(target_arch = "wasm32")]
            Event::UserEvent(SlintUserEvent::CustomEvent {
                event: CustomEvent::WakeEventLoopWorkaround,
            }) => {
                event_loop_target.set_control_flow(ControlFlow::Poll);
            }

            Event::NewEvents(_) => {
                event_loop_target.set_control_flow(ControlFlow::Wait);

                corelib::platform::update_timers_and_animations();
            }

            Event::Resumed => ALL_WINDOWS.with(|ws| {
                for (_, window_weak) in ws.borrow().iter() {
                    if let Some(w) = window_weak.upgrade() {
                        if let Err(e) = w.renderer.resumed(&w.winit_window()) {
                            self.loop_error = Some(e);
                        }
                    }
                }
            }),

            Event::AboutToWait => {
                if !event_loop_target.exiting() {
                    ALL_WINDOWS.with(|windows| {
                        for w in windows.borrow().iter().filter_map(|(_, w)| w.upgrade()) {
                            if w.window().has_active_animations() {
                                w.request_redraw();
                            }
                        }
                    })
                }

                if event_loop_target.control_flow() == ControlFlow::Wait {
                    if let Some(next_timer) = corelib::platform::duration_until_next_timer_update()
                    {
                        event_loop_target.set_control_flow(ControlFlow::wait_duration(next_timer));
                    }
                }
            }

            _ => (),
        };

        if self.loop_error.is_some() {
            event_loop_target.exit();
        }
    }

    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[allow(unused_mut)] // mut need changes for wasm
    pub fn run(mut self) -> Result<Self, corelib::platform::PlatformError> {
        let not_running_loop_instance = MAYBE_LOOP_INSTANCE
            .with(|loop_instance| match loop_instance.borrow_mut().take() {
                Some(instance) => Ok(instance),
                None => NotRunningEventLoop::new(),
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

        #[cfg(not(target_arch = "wasm32"))]
        {
            use winit::platform::run_on_demand::EventLoopExtRunOnDemand as _;
            let clipboard = not_running_loop_instance.clipboard;
            winit_loop
            .run_on_demand(
                |event: Event<SlintUserEvent>,
                 event_loop_target: &EventLoopWindowTarget<SlintUserEvent>| {
                    let running_instance = RunningEventLoop {
                        event_loop_target,
                        clipboard: &clipboard,
                    };
                    CURRENT_WINDOW_TARGET.set(&running_instance, || {
                        self.process_event(event, event_loop_target)
                    })
                },
            )
            .map_err(|e| format!("Error running winit event loop: {e}"))?;

            *GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap() = Default::default();

            // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
            // Winit does not support creating multiple instances of the event loop.
            let nre = NotRunningEventLoop { clipboard, instance: winit_loop, event_loop_proxy };
            MAYBE_LOOP_INSTANCE.with(|loop_instance| *loop_instance.borrow_mut() = Some(nre));

            if let Some(error) = self.loop_error {
                return Err(error);
            }
            Ok(self)
        }

        #[cfg(target_arch = "wasm32")]
        {
            winit_loop
            .run(
                move |event: Event<SlintUserEvent>,
                      event_loop_target: &EventLoopWindowTarget<SlintUserEvent>| {
                    let running_instance = RunningEventLoop {
                        event_loop_target,
                    };
                    CURRENT_WINDOW_TARGET.set(&running_instance, || {
                        self.process_event(event, event_loop_target)
                    })
                },
            )
            .map_err(|e| format!("Error running winit event loop: {e}"))?;
            // This can't really happen, as run() doesn't return
            Ok(Self::default())
        }
    }

    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pump_events(
        mut self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(Self, winit::platform::pump_events::PumpStatus), corelib::platform::PlatformError>
    {
        use winit::platform::pump_events::EventLoopExtPumpEvents;

        let not_running_loop_instance = MAYBE_LOOP_INSTANCE
            .with(|loop_instance| match loop_instance.borrow_mut().take() {
                Some(instance) => Ok(instance),
                None => NotRunningEventLoop::new(),
            })
            .map_err(|e| format!("Error initializing winit event loop: {e}"))?;

        let event_loop_proxy = not_running_loop_instance.event_loop_proxy;
        GLOBAL_PROXY
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .set_proxy(event_loop_proxy.clone());

        let mut winit_loop = not_running_loop_instance.instance;
        let clipboard = not_running_loop_instance.clipboard;

        let result = winit_loop.pump_events(
            timeout,
            |event: Event<SlintUserEvent>,
             event_loop_target: &EventLoopWindowTarget<SlintUserEvent>| {
                let running_instance =
                    RunningEventLoop { event_loop_target, clipboard: &clipboard };
                CURRENT_WINDOW_TARGET
                    .set(&running_instance, || self.process_event(event, event_loop_target))
            },
        );

        *GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap() = Default::default();

        // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
        // Winit does not support creating multiple instances of the event loop.
        let nre = NotRunningEventLoop { clipboard, instance: winit_loop, event_loop_proxy };
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
            None => NotRunningEventLoop::new(),
        })
        .map_err(|e| format!("Error initializing winit event loop: {e}"))?;

    let event_loop_proxy = not_running_loop_instance.event_loop_proxy;
    GLOBAL_PROXY.with(|global_proxy| {
        global_proxy
            .borrow_mut()
            .get_or_insert_with(Default::default)
            .set_proxy(event_loop_proxy.clone())
    });

    let mut loop_state = EventLoopState::default();

    not_running_loop_instance.instance.spawn(
        move |event: Event<SlintUserEvent>,
              event_loop_target: &EventLoopWindowTarget<SlintUserEvent>| {
            let running_instance = RunningEventLoop { event_loop_target };
            CURRENT_WINDOW_TARGET
                .set(&running_instance, || loop_state.process_event(event, event_loop_target))
        },
    );

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn create_clipboard<T>(_event_loop: &winit::event_loop::EventLoopWindowTarget<T>) -> ClipboardPair {
    // Provide a truly silent no-op clipboard context, as copypasta's NoopClipboard spams stdout with
    // println.
    struct SilentClipboardContext;
    impl copypasta::ClipboardProvider for SilentClipboardContext {
        fn get_contents(
            &mut self,
        ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
            Ok(Default::default())
        }

        fn set_contents(
            &mut self,
            _: String,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
            Ok(())
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(all(
            unix,
            not(any(
                target_os = "macos",
                target_os = "android",
                target_os = "ios",
                target_os = "emscripten"
            ))
        ))]
        {

            #[cfg(feature = "wayland")]
            if let raw_window_handle::RawDisplayHandle::Wayland(wayland) = raw_window_handle::HasRawDisplayHandle::raw_display_handle(&_event_loop) {
                let clipboard = unsafe { copypasta::wayland_clipboard::create_clipboards_from_external(wayland.display) };
                return (Box::new(clipboard.1), Box::new(clipboard.0));
            };
            #[cfg(feature = "x11")]
            {
                use copypasta::x11_clipboard::{X11ClipboardContext, Primary, Clipboard};
                let prim = X11ClipboardContext::<Primary>::new()
                    .map_or(
                        Box::new(SilentClipboardContext) as Box<dyn ClipboardProvider>,
                        |x| Box::new(x) as Box<dyn ClipboardProvider>,
                    );
                let sec = X11ClipboardContext::<Clipboard>::new()
                    .map_or(
                        Box::new(SilentClipboardContext) as Box<dyn ClipboardProvider>,
                        |x| Box::new(x) as Box<dyn ClipboardProvider>,
                    );
                (sec, prim)
            }
            #[cfg(not(feature = "x11"))]
            (Box::new(SilentClipboardContext), Box::new(SilentClipboardContext))
        } else {
            (
                copypasta::ClipboardContext::new().map_or(
                    Box::new(SilentClipboardContext) as Box<dyn ClipboardProvider>,
                    |x| Box::new(x) as Box<dyn ClipboardProvider>,
                ),
                Box::new(SilentClipboardContext),
            )
        }
    }
}
