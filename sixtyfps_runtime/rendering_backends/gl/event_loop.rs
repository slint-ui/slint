// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [PlatformWindow] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use corelib::component::ComponentRc;
use corelib::items::PointerEventButton;
use corelib::layout::Orientation;
use sixtyfps_corelib as corelib;

use corelib::graphics::Point;
use corelib::input::{KeyEvent, KeyEventType, KeyboardModifiers, MouseEvent};
use corelib::SharedString;
use corelib::{window::*, Color};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use winit::event::WindowEvent;

#[cfg(not(target_arch = "wasm32"))]
use winit::platform::run_return::EventLoopExtRunReturn;

pub trait WinitWindow: PlatformWindow {
    fn runtime_window(&self) -> Rc<corelib::window::Window>;
    fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>>;
    fn current_keyboard_modifiers(&self) -> &Cell<KeyboardModifiers>;
    fn draw(self: Rc<Self>);
    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window));
    fn constraints(&self) -> (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo);
    fn set_constraints(
        &self,
        constraints: (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo),
    );
    fn set_background_color(&self, color: Color);
    fn set_icon(&self, icon: corelib::graphics::Image);

    fn apply_constraints(
        &self,
        constraints_horizontal: corelib::layout::LayoutInfo,
        constraints_vertical: corelib::layout::LayoutInfo,
    ) {
        self.with_window_handle(&mut |winit_window| {
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_some() {
                return;
            }

            if (constraints_horizontal, constraints_vertical) != self.constraints() {
                let min_width = constraints_horizontal.min.min(constraints_horizontal.max);
                let min_height = constraints_vertical.min.min(constraints_vertical.max);
                let max_width = constraints_horizontal.max.max(constraints_horizontal.min);
                let max_height = constraints_vertical.max.max(constraints_vertical.min);

                let sf = self.runtime_window().scale_factor();

                winit_window.set_min_inner_size(if min_width > 0. || min_height > 0. {
                    Some(winit::dpi::PhysicalSize::new(min_width * sf, min_height * sf))
                } else {
                    None
                });
                winit_window.set_max_inner_size(if max_width < f32::MAX || max_height < f32::MAX {
                    Some(winit::dpi::PhysicalSize::new(
                        (max_width * sf).min(65535.),
                        (max_height * sf).min(65535.),
                    ))
                } else {
                    None
                });
                self.set_constraints((constraints_horizontal, constraints_vertical));

                #[cfg(target_arch = "wasm32")]
                {
                    // set_max_inner_size / set_min_inner_size don't work on wasm, so apply the size manually
                    let existing_size = winit_window.inner_size();
                    if !(min_width..=max_width).contains(&(existing_size.width as f32))
                        || !(min_height..=max_height).contains(&(existing_size.height as f32))
                    {
                        let new_size = winit::dpi::LogicalSize::new(
                            existing_size
                                .width
                                .min(max_width.ceil() as u32)
                                .max(min_width.ceil() as u32),
                            existing_size
                                .height
                                .min(max_height.ceil() as u32)
                                .max(min_height.ceil() as u32),
                        );
                        winit_window.set_inner_size(new_size);
                    }
                }
            }
        });
    }

    fn apply_window_properties(
        &self,
        window_item: core::pin::Pin<&sixtyfps_corelib::items::WindowItem>,
    ) {
        let background = window_item.background();
        let title = window_item.title();
        let no_frame = window_item.no_frame();
        let icon = window_item.icon();
        let width = window_item.width();
        let height = window_item.height();

        self.set_background_color(background);
        self.set_icon(icon);

        let mut size: winit::dpi::LogicalSize<f64> = Default::default();
        self.with_window_handle(&mut |winit_window| {
            winit_window.set_title(&title);
            if no_frame && winit_window.fullscreen().is_none() {
                winit_window.set_decorations(false);
            } else {
                winit_window.set_decorations(true);
            }
            size =
                winit_window.inner_size().to_logical(self.runtime_window().scale_factor() as f64);
        });

        let mut must_resize = false;
        let mut resizable = None;
        let mut w = width;
        let mut h = height;
        if (size.width as f32 - w).abs() < 1. || (size.height as f32 - h).abs() < 1. {
            return;
        }
        if w <= 0. || h <= 0. {
            if let Some(component_rc) = self.runtime_window().try_component() {
                let component = ComponentRc::borrow_pin(&component_rc);
                let hor_info = component.as_ref().layout_info(Orientation::Horizontal);
                if w <= 0. {
                    w = hor_info.preferred_bounded();
                    must_resize = true;
                }
                let ver_info = component.as_ref().layout_info(Orientation::Vertical);
                if h <= 0. {
                    h = ver_info.preferred_bounded();
                    must_resize = true;
                }
                resizable = Some(hor_info.min != hor_info.max && ver_info.min != ver_info.max);
            }
        };
        if w > 0. {
            size.width = w as _;
        }
        if h > 0. {
            size.height = h as _;
        }
        self.with_window_handle(&mut |winit_window| {
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_none() {
                winit_window.set_inner_size(size);
                if let Some(resizable) = resizable {
                    winit_window.set_resizable(resizable)
                }
            }
        });

        if must_resize {
            self.runtime_window().set_window_item_geometry(size.width as _, size.height as _)
        }
    }
}

struct NotRunningEventLoop {
    instance: winit::event_loop::EventLoop<CustomEvent>,
    event_loop_proxy: winit::event_loop::EventLoopProxy<CustomEvent>,
}

impl NotRunningEventLoop {
    fn new() -> Self {
        let instance = winit::event_loop::EventLoop::with_user_event();
        let event_loop_proxy = instance.create_proxy();
        Self { instance, event_loop_proxy }
    }
}

struct RunningEventLoop<'a> {
    event_loop_target: &'a winit::event_loop::EventLoopWindowTarget<CustomEvent>,
    event_loop_proxy: &'a winit::event_loop::EventLoopProxy<CustomEvent>,
}

pub(crate) trait EventLoopInterface {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<CustomEvent>;
    fn event_loop_proxy(&self) -> &winit::event_loop::EventLoopProxy<CustomEvent>;
}

impl EventLoopInterface for NotRunningEventLoop {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<CustomEvent> {
        &*self.instance
    }

    fn event_loop_proxy(&self) -> &winit::event_loop::EventLoopProxy<CustomEvent> {
        &self.event_loop_proxy
    }
}

impl<'a> EventLoopInterface for RunningEventLoop<'a> {
    fn event_loop_target(&self) -> &winit::event_loop::EventLoopWindowTarget<CustomEvent> {
        self.event_loop_target
    }

    fn event_loop_proxy(&self) -> &winit::event_loop::EventLoopProxy<CustomEvent> {
        self.event_loop_proxy
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
    pub(crate) fn send_event(&mut self, event: CustomEvent) {
        match self {
            GlobalEventLoopProxyOrEventQueue::Proxy(proxy) => proxy.send_event(event).ok().unwrap(),
            GlobalEventLoopProxyOrEventQueue::Queue(queue) => {
                queue.push(event);
            }
        };
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
    /// Request for the event loop to wake up and redraw all windows. This is used on the
    /// web for example to request an animation frame.
    #[cfg(target_arch = "wasm32")]
    RedrawAllWindows,
    UpdateWindowProperties(winit::window::WindowId),
    UserEvent(Box<dyn FnOnce() + Send>),
    Exit,
}

impl std::fmt::Debug for CustomEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::RedrawAllWindows => write!(f, "RedrawAllWindows"),
            Self::UpdateWindowProperties(e) => write!(f, "UpdateWindowProperties({:?})", e),
            Self::UserEvent(_) => write!(f, "UserEvent"),
            Self::Exit => write!(f, "Exit"),
        }
    }
}

fn redraw_all_windows() {
    let all_windows_weak =
        ALL_WINDOWS.with(|windows| windows.borrow().values().cloned().collect::<Vec<_>>());
    for window_weak in all_windows_weak {
        if let Some(window) = window_weak.upgrade() {
            window.request_redraw();
        }
    }
}

mod key_codes {
    macro_rules! winit_key_to_string_fn {
        ($($char:literal # $name:ident # $($_qt:ident)|* # $($winit:ident)|* ;)*) => {
            pub fn winit_key_to_string(virtual_keycode: winit::event::VirtualKeyCode) -> Option<sixtyfps_corelib::SharedString> {
                let char = match(virtual_keycode) {
                    $($(winit::event::VirtualKeyCode::$winit => $char,)*)*
                    _ => return None,
                };
                let mut buffer = [0; 6];
                Some(sixtyfps_corelib::SharedString::from(char.encode_utf8(&mut buffer) as &str))
            }
        };
    }
    sixtyfps_common::for_each_special_keys!(winit_key_to_string_fn);
}

fn process_window_event(
    window: Rc<dyn WinitWindow>,
    event: WindowEvent,
    quit_behavior: sixtyfps_corelib::backend::EventLoopQuitBehavior,
    control_flow: &mut winit::event_loop::ControlFlow,
    cursor_pos: &mut Point,
    pressed: &mut bool,
) {
    let runtime_window = window.runtime_window();
    match event {
        WindowEvent::Resized(size) => {
            let size = size.to_logical(runtime_window.scale_factor() as f64);
            runtime_window.set_window_item_geometry(size.width, size.height);
        }
        WindowEvent::CloseRequested => {
            window.hide();
            match quit_behavior {
                corelib::backend::EventLoopQuitBehavior::QuitOnLastWindowClosed => {
                    let window_count = ALL_WINDOWS.with(|windows| windows.borrow().len());
                    if window_count == 0 {
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                    }
                }
                corelib::backend::EventLoopQuitBehavior::QuitOnlyExplicitly => {}
            }
        }
        WindowEvent::ReceivedCharacter(ch) => {
            corelib::animations::update_animations();
            // On Windows, X11 and Wayland sequences like Ctrl+C will send a ReceivedCharacter after the pressed keyboard input event,
            // with a control character. We choose not to forward those but try to use the current key code instead.
            let text: Option<SharedString> = if ch.is_control() {
                window.currently_pressed_key_code().take().and_then(winit_key_code_to_string)
            } else {
                Some(ch.to_string().into())
            };

            let text = match text {
                Some(text) => text,
                None => return,
            };

            let modifiers = window.current_keyboard_modifiers().get();

            let mut event = KeyEvent { event_type: KeyEventType::KeyPressed, text, modifiers };
            runtime_window.clone().process_key_input(&event);
            event.event_type = KeyEventType::KeyReleased;
            runtime_window.process_key_input(&event);
        }
        WindowEvent::Focused(have_focus) => {
            // We don't render popups as separate windows yet, so treat
            // focus to be the same as being active.
            runtime_window.set_active(have_focus);
            runtime_window.set_focus(have_focus);
        }
        WindowEvent::KeyboardInput { ref input, .. } => {
            corelib::animations::update_animations();
            window.currently_pressed_key_code().set(match input.state {
                winit::event::ElementState::Pressed => input.virtual_keycode.clone(),
                _ => None,
            });
            if let Some(text) = input.virtual_keycode.and_then(key_codes::winit_key_to_string) {
                let event = KeyEvent {
                    event_type: match input.state {
                        winit::event::ElementState::Pressed => KeyEventType::KeyPressed,
                        winit::event::ElementState::Released => KeyEventType::KeyReleased,
                    },
                    text,
                    modifiers: window.current_keyboard_modifiers().get(),
                };
                runtime_window.process_key_input(&event);
            };
        }
        WindowEvent::ModifiersChanged(state) => {
            // To provide an easier cross-platform behavior, we map the command key to control
            // on macOS, and control to meta.
            #[cfg(target_os = "macos")]
            let (control, meta) = (state.logo(), state.ctrl());
            #[cfg(not(target_os = "macos"))]
            let (control, meta) = (state.ctrl(), state.logo());
            let modifiers =
                KeyboardModifiers { shift: state.shift(), alt: state.alt(), control, meta };
            window.current_keyboard_modifiers().set(modifiers);
        }
        WindowEvent::CursorMoved { position, .. } => {
            corelib::animations::update_animations();
            let position = position.to_logical(runtime_window.scale_factor() as f64);
            *cursor_pos = euclid::point2(position.x, position.y);
            runtime_window.process_mouse_input(MouseEvent::MouseMoved { pos: *cursor_pos });
        }
        WindowEvent::CursorLeft { .. } => {
            // On the html canvas, we don't get the mouse move or release event when outside the canvas. So we have no choice but canceling the event
            #[cfg(target_arch = "wasm32")]
            if *pressed {
                corelib::animations::update_animations();
                *pressed = false;
                runtime_window.process_mouse_input(MouseEvent::MouseExit);
            }
        }
        WindowEvent::MouseWheel { delta, .. } => {
            corelib::animations::update_animations();
            let delta = match delta {
                winit::event::MouseScrollDelta::LineDelta(lx, ly) => {
                    euclid::point2(lx * 60., ly * 60.)
                }
                winit::event::MouseScrollDelta::PixelDelta(d) => {
                    let d = d.to_logical(runtime_window.scale_factor() as f64);
                    euclid::point2(d.x, d.y)
                }
            };
            runtime_window.process_mouse_input(MouseEvent::MouseWheel { pos: *cursor_pos, delta });
        }
        WindowEvent::MouseInput { state, button, .. } => {
            corelib::animations::update_animations();
            let button = match button {
                winit::event::MouseButton::Left => PointerEventButton::left,
                winit::event::MouseButton::Right => PointerEventButton::right,
                winit::event::MouseButton::Middle => PointerEventButton::middle,
                winit::event::MouseButton::Other(_) => PointerEventButton::none,
            };
            let ev = match state {
                winit::event::ElementState::Pressed => {
                    *pressed = true;
                    MouseEvent::MousePressed { pos: *cursor_pos, button }
                }
                winit::event::ElementState::Released => {
                    *pressed = false;
                    MouseEvent::MouseReleased { pos: *cursor_pos, button }
                }
            };
            runtime_window.process_mouse_input(ev);
        }
        WindowEvent::Touch(touch) => {
            corelib::animations::update_animations();
            let location = touch.location.to_logical(runtime_window.scale_factor() as f64);
            let pos = euclid::point2(location.x, location.y);
            let ev = match touch.phase {
                winit::event::TouchPhase::Started => {
                    *pressed = true;
                    MouseEvent::MousePressed { pos, button: PointerEventButton::left }
                }
                winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => {
                    *pressed = false;
                    MouseEvent::MouseReleased { pos, button: PointerEventButton::left }
                }
                winit::event::TouchPhase::Moved => MouseEvent::MouseMoved { pos },
            };
            runtime_window.process_mouse_input(ev);
        }
        WindowEvent::ScaleFactorChanged { scale_factor, new_inner_size: size } => {
            if !std::env::var("SIXTYFPS_SCALE_FACTOR").is_ok() {
                let size = size.to_logical(scale_factor);
                runtime_window.set_window_item_geometry(size.width, size.height);
                runtime_window.set_scale_factor(scale_factor as f32);
            }
        }
        _ => {}
    }
}

/// Runs the event loop and renders the items in the provided `component` in its
/// own window.
#[allow(unused_mut)] // mut need changes for wasm
pub fn run(quit_behavior: sixtyfps_corelib::backend::EventLoopQuitBehavior) {
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

    // last seen cursor position, (physical coordinate)
    let mut cursor_pos = Point::default();
    let mut pressed = false;
    let mut run_fn = move |event: Event<CustomEvent>,
                           event_loop_target: &EventLoopWindowTarget<CustomEvent>,
                           control_flow: &mut ControlFlow| {
        let running_instance =
            RunningEventLoop { event_loop_target, event_loop_proxy: &event_loop_proxy };
        CURRENT_WINDOW_TARGET.set(&running_instance, || {
            *control_flow = ControlFlow::Wait;

            match event {
                winit::event::Event::WindowEvent { event, window_id } => {
                    if let Some(window) = window_by_id(window_id) {
                        process_window_event(
                            window,
                            event,
                            quit_behavior,
                            control_flow,
                            &mut cursor_pos,
                            &mut pressed,
                        );
                    };
                }

                winit::event::Event::RedrawRequested(id) => {
                    corelib::animations::update_animations();
                    if let Some(window) = window_by_id(id) {
                        window.draw();
                    }
                }

                winit::event::Event::UserEvent(CustomEvent::UpdateWindowProperties(window_id)) => {
                    if let Some(window) = window_by_id(window_id) {
                        window.runtime_window().update_window_properties();
                    }
                }

                winit::event::Event::UserEvent(CustomEvent::Exit) => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }

                winit::event::Event::UserEvent(CustomEvent::UserEvent(user)) => {
                    user();
                }

                #[cfg(target_arch = "wasm32")]
                winit::event::Event::UserEvent(CustomEvent::RedrawAllWindows) => {
                    redraw_all_windows()
                }
                _ => (),
            }

            if *control_flow != winit::event_loop::ControlFlow::Exit {
                if corelib::animations::CURRENT_ANIMATION_DRIVER
                    .with(|driver| driver.has_active_animations())
                {
                    *control_flow = ControlFlow::Poll;
                    redraw_all_windows()
                }
            }

            corelib::timers::TimerList::maybe_activate_timers();

            if *control_flow == winit::event_loop::ControlFlow::Wait {
                if let Some(next_timer) = corelib::timers::TimerList::next_timeout() {
                    *control_flow = winit::event_loop::ControlFlow::WaitUntil(next_timer.into());
                }
            }
        })
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        winit_loop.run_return(run_fn);

        *GLOBAL_PROXY.get_or_init(Default::default).lock().unwrap() = Default::default();
    }

    #[cfg(target_arch = "wasm32")]
    {
        winit_loop.run(run_fn)
    }
}

// This function is called when we receive a control character via WindowEvent::ReceivedCharacter and
// instead want to use the last virtual key code. That happens when for example pressing Ctrl+some_key
// on Windows/X11/Wayland. This function may be missing mappings, it's trying to cover what we may be
// getting when we're getting control character sequences.
fn winit_key_code_to_string(virtual_keycode: winit::event::VirtualKeyCode) -> Option<SharedString> {
    use winit::event::VirtualKeyCode;
    Some(
        match virtual_keycode {
            VirtualKeyCode::Key1 => "1",
            VirtualKeyCode::Key2 => "2",
            VirtualKeyCode::Key3 => "3",
            VirtualKeyCode::Key4 => "4",
            VirtualKeyCode::Key5 => "5",
            VirtualKeyCode::Key6 => "6",
            VirtualKeyCode::Key7 => "7",
            VirtualKeyCode::Key8 => "8",
            VirtualKeyCode::Key9 => "9",
            VirtualKeyCode::Key0 => "0",
            VirtualKeyCode::A => "a",
            VirtualKeyCode::B => "b",
            VirtualKeyCode::C => "c",
            VirtualKeyCode::D => "d",
            VirtualKeyCode::E => "e",
            VirtualKeyCode::F => "f",
            VirtualKeyCode::G => "g",
            VirtualKeyCode::H => "h",
            VirtualKeyCode::I => "i",
            VirtualKeyCode::J => "j",
            VirtualKeyCode::K => "k",
            VirtualKeyCode::L => "l",
            VirtualKeyCode::M => "m",
            VirtualKeyCode::N => "n",
            VirtualKeyCode::O => "o",
            VirtualKeyCode::P => "p",
            VirtualKeyCode::Q => "q",
            VirtualKeyCode::R => "r",
            VirtualKeyCode::S => "s",
            VirtualKeyCode::T => "t",
            VirtualKeyCode::U => "u",
            VirtualKeyCode::V => "v",
            VirtualKeyCode::W => "w",
            VirtualKeyCode::X => "x",
            VirtualKeyCode::Y => "y",
            VirtualKeyCode::Z => "z",
            VirtualKeyCode::Space => " ",
            VirtualKeyCode::Caret => "^",
            VirtualKeyCode::Apostrophe => "'",
            VirtualKeyCode::Asterisk => "*",
            VirtualKeyCode::Backslash => "\\",
            VirtualKeyCode::Colon => ":",
            VirtualKeyCode::Comma => ",",
            VirtualKeyCode::Equals => "=",
            VirtualKeyCode::Grave => "`",
            VirtualKeyCode::Minus => "-",
            VirtualKeyCode::Period => ".",
            VirtualKeyCode::Plus => "+",
            VirtualKeyCode::Semicolon => ";",
            VirtualKeyCode::Slash => "/",
            VirtualKeyCode::Tab => "\t",
            _ => return None,
        }
        .into(),
    )
}
