// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [WindowAdapter] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use copypasta::ClipboardProvider;
use corelib::items::PointerEventButton;
use corelib::lengths::LogicalPoint;
use corelib::lengths::LogicalSize;
use i_slint_core as corelib;

use corelib::api::EventLoopError;
use corelib::graphics::euclid;
use corelib::input::{KeyEventType, KeyInputEvent, MouseEvent};
use corelib::window::*;
use std::cell::{Cell, RefCell, RefMut};
use std::rc::{Rc, Weak};

use winit::event::WindowEvent;
#[cfg(not(target_arch = "wasm32"))]
use winit::platform::run_return::EventLoopExtRunReturn;

pub(crate) static QUIT_ON_LAST_WINDOW_CLOSED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

pub trait WinitWindow: WindowAdapter {
    fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>>;
    /// Returns true if during the drawing request_redraw() was called.
    fn draw(&self) -> bool;
    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window));
    fn constraints(&self) -> (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo);
    fn set_constraints(
        &self,
        constraints: (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo),
    );

    /// Called by the event loop when a WindowEvent::Resized is received.
    fn resize_event(&self, _size: winit::dpi::PhysicalSize<u32>) {}

    /// Return true if the proxy element used for input method has the focus
    fn input_method_focused(&self) -> bool {
        false
    }
    /// Returns true if request_redraw() was called since the last event loop iteration
    /// and resets the state back to false.
    fn take_pending_redraw(&self) -> bool;
}

struct NotRunningEventLoop {
    clipboard: RefCell<Box<dyn copypasta::ClipboardProvider>>,
    instance: winit::event_loop::EventLoop<CustomEvent>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<CustomEvent>,
}

impl NotRunningEventLoop {
    fn new() -> Self {
        let instance = winit::event_loop::EventLoopBuilder::with_user_event().build();
        let event_loop_proxy = instance.create_proxy();
        let clipboard = RefCell::new(create_clipboard(&instance));
        Self { clipboard, instance, event_loop_proxy }
    }
}

struct RunningEventLoop<'a> {
    event_loop_target: &'a winit::event_loop::EventLoopWindowTarget<CustomEvent>,
    event_loop_proxy: &'a winit::event_loop::EventLoopProxy<CustomEvent>,
    clipboard: &'a RefCell<Box<dyn copypasta::ClipboardProvider>>,
}

pub(crate) trait EventLoopInterface {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<CustomEvent>;
    fn event_loop_proxy(&self) -> &winit::event_loop::EventLoopProxy<CustomEvent>;
    fn clipboard(&self) -> RefMut<'_, dyn ClipboardProvider>;
}

impl EventLoopInterface for NotRunningEventLoop {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<CustomEvent> {
        &*self.instance
    }

    fn event_loop_proxy(&self) -> &winit::event_loop::EventLoopProxy<CustomEvent> {
        &self.event_loop_proxy
    }

    fn clipboard(&self) -> RefMut<'_, dyn ClipboardProvider> {
        RefMut::map(self.clipboard.borrow_mut(), |clipboard_box| clipboard_box.as_mut())
    }
}

impl<'a> EventLoopInterface for RunningEventLoop<'a> {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<CustomEvent> {
        self.event_loop_target
    }

    fn event_loop_proxy(&self) -> &winit::event_loop::EventLoopProxy<CustomEvent> {
        self.event_loop_proxy
    }

    fn clipboard(&self) -> RefMut<'_, dyn ClipboardProvider> {
        RefMut::map(self.clipboard.borrow_mut(), |clipboard_box| clipboard_box.as_mut())
    }
}

thread_local! {
    static ALL_WINDOWS: RefCell<std::collections::HashMap<winit::window::WindowId, Weak<dyn WinitWindow>>> = RefCell::new(std::collections::HashMap::new());
    static MAYBE_LOOP_INSTANCE: RefCell<Option<NotRunningEventLoop>> = RefCell::new(Some(NotRunningEventLoop::new()));
}

scoped_tls_hkt::scoped_thread_local!(static CURRENT_WINDOW_TARGET : for<'a> &'a RunningEventLoop<'a>);

pub(crate) enum GlobalEventLoopProxyOrEventQueue {
    Proxy(winit::event_loop::EventLoopProxy<CustomEvent>),
    Queue(Vec<CustomEvent>),
}

impl GlobalEventLoopProxyOrEventQueue {
    pub(crate) fn send_event(&mut self, event: CustomEvent) -> Result<(), EventLoopError> {
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

    fn set_proxy(&mut self, proxy: winit::event_loop::EventLoopProxy<CustomEvent>) {
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

pub(crate) fn with_window_target<T>(callback: impl FnOnce(&dyn EventLoopInterface) -> T) -> T {
    if CURRENT_WINDOW_TARGET.is_set() {
        CURRENT_WINDOW_TARGET.with(|current_target| callback(current_target))
    } else {
        MAYBE_LOOP_INSTANCE.with(|loop_instance| {
            if loop_instance.borrow().is_none() {
                *loop_instance.borrow_mut() = Some(NotRunningEventLoop::new());
            }
            callback(loop_instance.borrow().as_ref().unwrap())
        })
    }
}

pub fn register_window(id: winit::window::WindowId, window: Rc<dyn WinitWindow>) {
    ALL_WINDOWS.with(|windows| {
        windows.borrow_mut().insert(id, Rc::downgrade(&window));
    })
}

pub fn unregister_window(id: winit::window::WindowId) {
    ALL_WINDOWS.with(|windows| {
        windows.borrow_mut().remove(&id);
    })
}

fn window_by_id(id: winit::window::WindowId) -> Option<Rc<dyn WinitWindow>> {
    ALL_WINDOWS.with(|windows| windows.borrow().get(&id).and_then(|weakref| weakref.upgrade()))
}

/// This enum captures run-time specific events that can be dispatched to the event loop in
/// addition to the winit events.
pub enum CustomEvent {
    /// On wasm request_redraw doesn't wake the event loop, so we need to manually send an event
    /// so that the event loop can run
    #[cfg(target_arch = "wasm32")]
    WakeEventLoopWorkaround,
    UpdateWindowProperties(winit::window::WindowId),
    UserEvent(Box<dyn FnOnce() + Send>),
    /// Called from `GLWindow::hide` so that we can check if we should quit the event loop
    WindowHidden,
    Exit,
}

impl std::fmt::Debug for CustomEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::WakeEventLoopWorkaround => write!(f, "WakeEventLoopWorkaround"),
            Self::UpdateWindowProperties(e) => write!(f, "UpdateWindowProperties({:?})", e),
            Self::UserEvent(_) => write!(f, "UserEvent"),
            Self::WindowHidden => write!(f, "WindowHidden"),
            Self::Exit => write!(f, "Exit"),
        }
    }
}

mod key_codes {
    macro_rules! winit_key_to_char_fn {
        ($($char:literal # $name:ident # $($_qt:ident)|* # $($winit:ident)|* ;)*) => {
            pub fn winit_key_to_char(virtual_keycode: winit::event::VirtualKeyCode) -> Option<char> {
                let char = match(virtual_keycode) {
                    $($(winit::event::VirtualKeyCode::$winit => $char,)*)*
                    _ => return None,
                };
                Some(char)
            }
        };
    }
    i_slint_common::for_each_special_keys!(winit_key_to_char_fn);
}

fn process_window_event(
    window: Rc<dyn WinitWindow>,
    event: WindowEvent,
    cursor_pos: &mut LogicalPoint,
    pressed: &mut bool,
) {
    let runtime_window = WindowInner::from_pub(window.window());
    match event {
        WindowEvent::Resized(size) => {
            window.resize_event(size);
        }
        WindowEvent::CloseRequested => {
            if runtime_window.request_close() {
                window.hide();
            }
        }
        WindowEvent::ReceivedCharacter(ch) => {
            // On Windows, X11 and Wayland sequences like Ctrl+C will send a ReceivedCharacter after the pressed keyboard input event,
            // with a control character. We choose not to forward those but try to use the current key code instead.
            //
            // We do not want to change the text to the value of the key press when that was a
            // control key itself: We already sent that event when handling the KeyboardInput.
            let text: char = if ch.is_control() {
                if let Some(ch) = window
                    .currently_pressed_key_code()
                    .take()
                    .and_then(winit_key_code_to_string)
                    .filter(|ch| !ch.is_control())
                {
                    ch
                } else {
                    return;
                }
            } else {
                ch
            };

            window.window().dispatch_event(corelib::platform::WindowEvent::KeyPressed { text });
            window.window().dispatch_event(corelib::platform::WindowEvent::KeyReleased { text });
        }
        WindowEvent::Focused(have_focus) => {
            let have_focus = have_focus || window.input_method_focused();
            // We don't render popups as separate windows yet, so treat
            // focus to be the same as being active.
            if have_focus != runtime_window.active() {
                runtime_window.set_active(have_focus);
                runtime_window.set_focus(have_focus);
            }
        }
        WindowEvent::KeyboardInput { ref input, .. } => {
            // For now: Match Qt's behavior of mapping command to control and control to meta (LWin/RWin).
            let key_code = input.virtual_keycode.map(|key_code| match key_code {
                #[cfg(target_os = "macos")]
                winit::event::VirtualKeyCode::LControl => winit::event::VirtualKeyCode::LWin,
                #[cfg(target_os = "macos")]
                winit::event::VirtualKeyCode::RControl => winit::event::VirtualKeyCode::RWin,
                #[cfg(target_os = "macos")]
                winit::event::VirtualKeyCode::LWin => winit::event::VirtualKeyCode::LControl,
                #[cfg(target_os = "macos")]
                winit::event::VirtualKeyCode::RWin => winit::event::VirtualKeyCode::RControl,
                code @ _ => code,
            });
            window.currently_pressed_key_code().set(match input.state {
                winit::event::ElementState::Pressed => key_code,
                _ => None,
            });
            if let Some(ch) = key_code.and_then(key_codes::winit_key_to_char) {
                window.window().dispatch_event(match input.state {
                    winit::event::ElementState::Pressed => {
                        corelib::platform::WindowEvent::KeyPressed { text: ch }
                    }
                    winit::event::ElementState::Released => {
                        corelib::platform::WindowEvent::KeyReleased { text: ch }
                    }
                });
            };
        }
        WindowEvent::Ime(winit::event::Ime::Preedit(string, preedit_selection)) => {
            let preedit_selection = preedit_selection.unwrap_or((0, 0));
            let event = KeyInputEvent {
                event_type: KeyEventType::UpdateComposition,
                text: string.into(),
                preedit_selection_start: preedit_selection.0,
                preedit_selection_end: preedit_selection.1,
                ..Default::default()
            };
            runtime_window.process_key_input(event);
        }
        WindowEvent::Ime(winit::event::Ime::Commit(string)) => {
            let event = KeyInputEvent {
                event_type: KeyEventType::CommitComposition,
                text: string.into(),
                ..Default::default()
            };
            runtime_window.process_key_input(event);
        }
        WindowEvent::CursorMoved { position, .. } => {
            let position = position.to_logical(runtime_window.scale_factor() as f64);
            *cursor_pos = euclid::point2(position.x, position.y);
            runtime_window.process_mouse_input(MouseEvent::Moved { position: *cursor_pos });
        }
        WindowEvent::CursorLeft { .. } => {
            // On the html canvas, we don't get the mouse move or release event when outside the canvas. So we have no choice but canceling the event
            if cfg!(target_arch = "wasm32") || !*pressed {
                *pressed = false;
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
                position: *cursor_pos,
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
            };
            let ev = match state {
                winit::event::ElementState::Pressed => {
                    *pressed = true;
                    MouseEvent::Pressed { position: *cursor_pos, button }
                }
                winit::event::ElementState::Released => {
                    *pressed = false;
                    MouseEvent::Released { position: *cursor_pos, button }
                }
            };
            runtime_window.process_mouse_input(ev);
        }
        WindowEvent::Touch(touch) => {
            let location = touch.location.to_logical(runtime_window.scale_factor() as f64);
            let position = euclid::point2(location.x, location.y);
            let ev = match touch.phase {
                winit::event::TouchPhase::Started => {
                    *pressed = true;
                    MouseEvent::Pressed { position, button: PointerEventButton::Left }
                }
                winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                    *pressed = false;
                    MouseEvent::Released { position, button: PointerEventButton::Left }
                }
                winit::event::TouchPhase::Moved => MouseEvent::Moved { position },
            };
            runtime_window.process_mouse_input(ev);
        }
        WindowEvent::ScaleFactorChanged { scale_factor, new_inner_size: size } => {
            if std::env::var("SLINT_SCALE_FACTOR").is_err() {
                let size = size.to_logical(scale_factor);
                runtime_window.set_window_item_geometry(LogicalSize::new(size.width, size.height));
                runtime_window.set_scale_factor(scale_factor as f32);
            }
        }
        _ => {}
    }
}

/// Runs the event loop and renders the items in the provided `component` in its
/// own window.
#[allow(unused_mut)] // mut need changes for wasm
pub fn run() {
    use winit::event::Event;
    use winit::event_loop::{ControlFlow, EventLoopWindowTarget};

    let not_running_loop_instance = MAYBE_LOOP_INSTANCE.with(|loop_instance| {
        loop_instance.borrow_mut().take().unwrap_or_else(NotRunningEventLoop::new)
    });

    let event_loop_proxy = not_running_loop_instance.event_loop_proxy;
    #[cfg(not(target_arch = "wasm32"))]
    GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap().set_proxy(event_loop_proxy.clone());
    #[cfg(target_arch = "wasm32")]
    GLOBAL_PROXY.with(|global_proxy| {
        global_proxy
            .borrow_mut()
            .get_or_insert_with(Default::default)
            .set_proxy(event_loop_proxy.clone())
    });

    let mut winit_loop = not_running_loop_instance.instance;
    let clipboard = not_running_loop_instance.clipboard;

    // Applying the property values of the `Window` item in .slint files to the underlying winit Window
    // may for example set the width but preserve the height of the window. Winit only knows the entire
    // size, so we need to query the existing size via `inner_size()`. The timing of when to call `inner_size()`
    // is important to avoid jitter as described in https://github.com/slint-ui/slint/issues/1269 . We want to
    // process pending resize events first. That is why this vector collects the window ids of windows that
    // we have scheduled for a sync between the `Window` item properties the the winit window, so that we
    // apply them at `MainEventsCleared` time.
    let mut windows_with_pending_property_updates = Vec::new();
    // With winit on Windows and with wasm, calling winit::Window::request_redraw() will not always deliver an
    // Event::RedrawRequested (for example when the mouse cursor is outside of the window). So when we get woken
    // up by the event loop to process new events from the operating system (NewEvents), we take note of all windows
    // that called request_redraw() since the last iteration and we will call draw() ourselves, unless they received
    // an Event::RedrawRequested in this new iteration. This vector collects the window ids of windows with pending
    // redraw requests in the beginning of the loop iteration, removes ids that are covered by a windowing system
    // supplied Event::RedrawRequested, and drains them for drawing at RedrawEventsCleared.
    let mut windows_with_pending_redraw_requests = Vec::new();

    // last seen cursor position
    let mut cursor_pos = LogicalPoint::default();
    let mut pressed = false;

    let mut run_fn = move |event: Event<CustomEvent>, control_flow: &mut ControlFlow| match event {
        Event::WindowEvent { event, window_id } => {
            if let Some(window) = window_by_id(window_id) {
                process_window_event(window, event, &mut cursor_pos, &mut pressed);
            };
        }

        Event::RedrawRequested(id) => {
            if let Some(window) = window_by_id(id) {
                if let Ok(pos) = windows_with_pending_redraw_requests.binary_search(&id) {
                    windows_with_pending_redraw_requests.remove(pos);
                }
                let redraw_requested_during_draw = window.draw();
                if redraw_requested_during_draw {
                    // If during rendering a new redraw_request() was issued (for example in a rendering notifier callback), then
                    // pretend that an animation is running, so that we return Poll from the event loop to ensure a repaint as
                    // soon as possible.
                    *control_flow = ControlFlow::Poll;
                }
            }
        }

        Event::UserEvent(CustomEvent::UpdateWindowProperties(window_id)) => {
            if let Err(insert_pos) = windows_with_pending_property_updates.binary_search(&window_id)
            {
                windows_with_pending_property_updates.insert(insert_pos, window_id);
            }
        }
        Event::UserEvent(CustomEvent::WindowHidden) => {
            if QUIT_ON_LAST_WINDOW_CLOSED.load(std::sync::atomic::Ordering::Relaxed) {
                let window_count = ALL_WINDOWS.with(|windows| windows.borrow().len());
                if window_count == 0 {
                    *control_flow = ControlFlow::Exit;
                }
            }
        }

        Event::UserEvent(CustomEvent::Exit) => {
            *control_flow = ControlFlow::Exit;
        }

        Event::UserEvent(CustomEvent::UserEvent(user)) => {
            user();
        }

        #[cfg(target_arch = "wasm32")]
        Event::UserEvent(CustomEvent::WakeEventLoopWorkaround) => {
            *control_flow = ControlFlow::Poll;
        }

        Event::NewEvents(_) => {
            *control_flow = ControlFlow::Wait;

            windows_with_pending_redraw_requests.clear();
            ALL_WINDOWS.with(|windows| {
                for (window_id, window_weak) in windows.borrow().iter() {
                    if window_weak.upgrade().map_or(false, |window| window.take_pending_redraw()) {
                        if let Err(insert_pos) =
                            windows_with_pending_redraw_requests.binary_search(window_id)
                        {
                            windows_with_pending_redraw_requests.insert(insert_pos, *window_id);
                        }
                    }
                }
            });

            corelib::platform::update_timers_and_animations();
        }

        Event::MainEventsCleared => {
            for window in windows_with_pending_property_updates
                .drain(..)
                .filter_map(|window_id| window_by_id(window_id))
            {
                WindowInner::from_pub(window.window()).update_window_properties();
            }
        }

        Event::RedrawEventsCleared => {
            if *control_flow != ControlFlow::Exit
                && ALL_WINDOWS.with(|windows| {
                    windows.borrow().iter().any(|(_, w)| {
                        w.upgrade().map_or(false, |w| w.window().has_active_animations())
                    })
                })
            {
                *control_flow = ControlFlow::Poll;
            }

            for window in windows_with_pending_redraw_requests
                .drain(..)
                .filter_map(|window_id| window_by_id(window_id))
            {
                let redraw_requested_during_draw = window.draw();
                if redraw_requested_during_draw {
                    // If during rendering a new redraw_request() was issued (for example in a rendering notifier callback), then
                    // pretend that an animation is running, so that we return Poll from the event loop to ensure a repaint as
                    // soon as possible.
                    *control_flow = ControlFlow::Poll;
                }
            }

            if *control_flow == ControlFlow::Wait {
                if let Some(next_timer) = corelib::platform::duration_until_next_timer_update() {
                    *control_flow = ControlFlow::WaitUntil(instant::Instant::now() + next_timer);
                }
            }
        }

        _ => (),
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        winit_loop.run_return(
            |event: Event<CustomEvent>,
             event_loop_target: &EventLoopWindowTarget<CustomEvent>,
             control_flow: &mut ControlFlow| {
                let running_instance = RunningEventLoop {
                    event_loop_target,
                    event_loop_proxy: &event_loop_proxy,
                    clipboard: &clipboard,
                };
                CURRENT_WINDOW_TARGET.set(&running_instance, || run_fn(event, control_flow))
            },
        );

        *GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap() = Default::default();

        // Keep the EventLoop instance alive and re-use it in future invocations of run_event_loop().
        // Winit does not support creating multiple instances of the event loop.
        let nre = NotRunningEventLoop { clipboard, instance: winit_loop, event_loop_proxy };
        MAYBE_LOOP_INSTANCE.with(|loop_instance| *loop_instance.borrow_mut() = Some(nre))
    }

    #[cfg(target_arch = "wasm32")]
    {
        winit_loop.run(
            move |event: Event<CustomEvent>,
                  event_loop_target: &EventLoopWindowTarget<CustomEvent>,
                  control_flow: &mut ControlFlow| {
                let running_instance = RunningEventLoop {
                    event_loop_target,
                    event_loop_proxy: &event_loop_proxy,
                    clipboard: &clipboard,
                };
                CURRENT_WINDOW_TARGET.set(&running_instance, || run_fn(event, control_flow))
            },
        )
    }
}

// This function is called when we receive a control character via WindowEvent::ReceivedCharacter and
// instead want to use the last virtual key code. That happens when for example pressing Ctrl+some_key
// on Windows/X11/Wayland. This function may be missing mappings, it's trying to cover what we may be
// getting when we're getting control character sequences.
fn winit_key_code_to_string(virtual_keycode: winit::event::VirtualKeyCode) -> Option<char> {
    use winit::event::VirtualKeyCode;
    Some(match virtual_keycode {
        VirtualKeyCode::Key1 => '1',
        VirtualKeyCode::Key2 => '2',
        VirtualKeyCode::Key3 => '3',
        VirtualKeyCode::Key4 => '4',
        VirtualKeyCode::Key5 => '5',
        VirtualKeyCode::Key6 => '6',
        VirtualKeyCode::Key7 => '7',
        VirtualKeyCode::Key8 => '8',
        VirtualKeyCode::Key9 => '9',
        VirtualKeyCode::Key0 => '0',
        VirtualKeyCode::A => 'a',
        VirtualKeyCode::B => 'b',
        VirtualKeyCode::C => 'c',
        VirtualKeyCode::D => 'd',
        VirtualKeyCode::E => 'e',
        VirtualKeyCode::F => 'f',
        VirtualKeyCode::G => 'g',
        VirtualKeyCode::H => 'h',
        VirtualKeyCode::I => 'i',
        VirtualKeyCode::J => 'j',
        VirtualKeyCode::K => 'k',
        VirtualKeyCode::L => 'l',
        VirtualKeyCode::M => 'm',
        VirtualKeyCode::N => 'n',
        VirtualKeyCode::O => 'o',
        VirtualKeyCode::P => 'p',
        VirtualKeyCode::Q => 'q',
        VirtualKeyCode::R => 'r',
        VirtualKeyCode::S => 's',
        VirtualKeyCode::T => 't',
        VirtualKeyCode::U => 'u',
        VirtualKeyCode::V => 'v',
        VirtualKeyCode::W => 'w',
        VirtualKeyCode::X => 'x',
        VirtualKeyCode::Y => 'y',
        VirtualKeyCode::Z => 'z',
        VirtualKeyCode::Space => ' ',
        VirtualKeyCode::Caret => '^',
        VirtualKeyCode::Apostrophe => '\'',
        VirtualKeyCode::Asterisk => '*',
        VirtualKeyCode::Backslash => '\\',
        VirtualKeyCode::Colon => ':',
        VirtualKeyCode::Comma => ',',
        VirtualKeyCode::Equals => '=',
        VirtualKeyCode::Grave => '`',
        VirtualKeyCode::Minus => '-',
        VirtualKeyCode::Period => '.',
        VirtualKeyCode::Plus => '+',
        VirtualKeyCode::Semicolon => ';',
        VirtualKeyCode::Slash => '/',
        VirtualKeyCode::Tab => '\t',
        _ => return None,
    })
}

fn create_clipboard<T>(
    _event_loop: &winit::event_loop::EventLoopWindowTarget<T>,
) -> Box<dyn copypasta::ClipboardProvider> {
    #[allow(unused_mut)]
    let mut clipboard: Option<Box<dyn copypasta::ClipboardProvider>> = None;

    cfg_if::cfg_if! {
        if #[cfg(all(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ), feature = "wayland", not(feature = "x11")))] {
            type DefaultClipboard = copypasta::nop_clipboard::NopClipboardContext;
        } else {
            type DefaultClipboard = copypasta::ClipboardContext;
        }
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    if cfg!(feature = "wayland") {
        use winit::platform::unix::EventLoopWindowTargetExtUnix;
        #[cfg(feature = "wayland")]
        if let Some(wayland_display) = _event_loop.wayland_display() {
            clipboard = unsafe {
                Some(Box::new(
                    copypasta::wayland_clipboard::create_clipboards_from_external(wayland_display)
                        .1,
                ))
            };
        }
    }

    clipboard.unwrap_or_else(|| Box::new(DefaultClipboard::new().unwrap()))
}
