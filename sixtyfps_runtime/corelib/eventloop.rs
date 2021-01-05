/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]
/*!
    This module contains the event loop implementation using winit, as well as the
    [GenericWindow] trait used by the generated code and the run-time to change
    aspects of windows on the screen.
*/
use crate::component::ComponentRc;
use crate::graphics::Point;
use crate::items::{ItemRc, ItemRef};
use crate::slice::Slice;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::{convert::TryInto, pin::Pin};

use crate::input::{KeyEvent, MouseEventType};
#[cfg(not(target_arch = "wasm32"))]
use winit::platform::run_return::EventLoopExtRunReturn;

/// This trait represents the interface that the generated code and the run-time
/// require in order to implement functionality such as device-independent pixels,
/// window resizing and other typicaly windowing system related tasks.
///
/// [`crate::graphics`] provides an implementation of this trait for use with [`crate::graphics::GraphicsBackend`].
pub trait GenericWindow {
    /// Associates this window with the specified component. Further event handling and rendering, etc. will be
    /// done with that component.
    fn set_component(self: Rc<Self>, component: &ComponentRc);

    /// Draw the items of the specified `component` in the given window.
    fn draw(self: Rc<Self>);
    /// Receive a mouse event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `pos`: The position of the mouse event in window physical coordinates.
    /// * `what`: The type of mouse event.
    /// * `component`: The SixtyFPS compiled component that provides the tree of items.
    fn process_mouse_input(
        self: Rc<Self>,
        pos: winit::dpi::PhysicalPosition<f64>,
        what: MouseEventType,
    );
    /// Receive a key event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `event`: The key event received by the windowing system.
    /// * `component`: The SixtyFPS compiled component that provides the tree of items.
    fn process_key_input(self: Rc<Self>, event: &KeyEvent);
    /// Calls the `callback` function with the underlying winit::Window that this
    /// GenericWindow backs.
    fn with_platform_window(&self, callback: &dyn Fn(&winit::window::Window));
    /// Requests for the window to be mapped to the screen.
    ///
    /// Arguments:
    /// * `event_loop`: The event loop used to drive further event handling for this window
    ///   as it will receive events.
    /// * `component`: The component that holds the root item of the scene. If the item is a [`crate::items::Window`], then
    ///   the `width` and `height` properties are read and the values are passed to the windowing system as request
    ///   for the initial size of the window. Then bindings are installed on these properties to keep them up-to-date
    ///   with the size as it may be changed by the user or the windowing system in general.
    fn map_window(self: Rc<Self>, event_loop: &EventLoop);
    /// Removes the window from the screen. The window is not destroyed though, it can be show (mapped) again later
    /// by calling [`GenericWindow::map_window`].
    fn unmap_window(self: Rc<Self>);
    /// Issue a request to the windowing system to re-render the contents of the window. This is typically an asynchronous
    /// request.
    fn request_redraw(&self);
    /// Returns the scale factor set on the window, as provided by the windowing system.
    fn scale_factor(&self) -> f32;
    /// Sets an overriding scale factor for the window. This is typically only used for testing.
    fn set_scale_factor(&self, factor: f32);
    /// Sets the size of the window to the specified `width`. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    fn set_width(&self, width: f32);
    /// Sets the size of the window to the specified `height`. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    fn set_height(&self, height: f32);
    /// Returns the geometry of the window
    fn get_geometry(&self) -> crate::graphics::Rect;

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources cached via [`crate::graphics::RenderingCache`].
    fn free_graphics_resources<'a>(self: Rc<Self>, items: &Slice<'a, Pin<ItemRef<'a>>>);
    /// Installs a binding on the specified property that's toggled whenever the text cursor is supposed to be visible or not.
    fn set_cursor_blink_binding(&self, prop: &crate::properties::Property<bool>);

    /// Returns the currently active keyboard notifiers.
    fn current_keyboard_modifiers(&self) -> crate::input::KeyboardModifiers;
    /// Sets the currently active keyboard notifiers. This is used only for testing or directly
    /// from the event loop implementation.
    fn set_current_keyboard_modifiers(&self, modifiers: crate::input::KeyboardModifiers);

    /// Sets the focus to the item pointed to by item_ptr. This will remove the focus from any
    /// currently focused item.
    fn set_focus_item(self: Rc<Self>, focus_item: &ItemRc);
    /// Sets the focus on the window to true or false, depending on the have_focus argument.
    /// This results in WindowFocusReceived and WindowFocusLost events.
    fn set_focus(self: Rc<Self>, have_focus: bool);

    /// Show a popup at the given position
    fn show_popup(&self, popup: &ComponentRc, position: Point);
    /// Close the active popup if any
    fn close_popup(&self);
}

/// The ComponentWindow is the (rust) facing public type that can render the items
/// of components to the screen.
#[repr(C)]
#[derive(Clone)]
pub struct ComponentWindow(pub std::rc::Rc<dyn crate::eventloop::GenericWindow>);

impl ComponentWindow {
    /// Creates a new instance of a CompomentWindow based on the given window implementation. Only used
    /// internally.
    pub fn new(window_impl: std::rc::Rc<dyn crate::eventloop::GenericWindow>) -> Self {
        Self(window_impl)
    }
    /// Spins an event loop and renders the items of the provided component in this window.
    pub fn run(&self) {
        let event_loop = crate::eventloop::EventLoop::new();

        self.0.clone().map_window(&event_loop);

        event_loop.run();

        self.0.clone().unmap_window();
    }

    /// Returns the scale factor set on the window.
    pub fn scale_factor(&self) -> f32 {
        self.0.scale_factor()
    }

    /// Sets an overriding scale factor for the window. This is typically only used for testing.
    pub fn set_scale_factor(&self, factor: f32) {
        self.0.set_scale_factor(factor)
    }

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources cached via [RenderingCache][`crate::graphics::RenderingCache`].
    pub fn free_graphics_resources<'a>(&self, items: &Slice<'a, Pin<ItemRef<'a>>>) {
        self.0.clone().free_graphics_resources(items);
    }

    /// Installs a binding on the specified property that's toggled whenever the text cursor is supposed to be visible or not.
    pub(crate) fn set_cursor_blink_binding(&self, prop: &crate::properties::Property<bool>) {
        self.0.clone().set_cursor_blink_binding(prop)
    }

    /// Sets the currently active keyboard notifiers. This is used only for testing or directly
    /// from the event loop implementation.
    pub(crate) fn set_current_keyboard_modifiers(
        &self,
        modifiers: crate::input::KeyboardModifiers,
    ) {
        self.0.clone().set_current_keyboard_modifiers(modifiers)
    }

    /// Returns the currently active keyboard notifiers.
    pub(crate) fn current_keyboard_modifiers(&self) -> crate::input::KeyboardModifiers {
        self.0.clone().current_keyboard_modifiers()
    }

    pub(crate) fn process_key_input(&self, event: &KeyEvent) {
        self.0.clone().process_key_input(event)
    }

    /// Clears the focus on any previously focused item and makes the provided
    /// item the focus item, in order to receive future key events.
    pub fn set_focus_item(&self, focus_item: &ItemRc) {
        self.0.clone().set_focus_item(focus_item)
    }

    /// Associates this window with the specified component, for future event handling, etc.
    pub fn set_component(&self, component: &ComponentRc) {
        self.0.clone().set_component(component)
    }

    /// Show a popup at the given position
    pub fn show_popup(&self, popup: &ComponentRc, position: Point) {
        self.0.clone().show_popup(popup, position)
    }
    /// Close the active popup if any
    pub fn close_popup(&self) {
        self.0.clone().close_popup()
    }
}

thread_local! {
    static ALL_WINDOWS: RefCell<std::collections::HashMap<winit::window::WindowId, Weak<dyn GenericWindow>>> = RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn register_window(id: winit::window::WindowId, window: Rc<dyn GenericWindow>) {
    ALL_WINDOWS.with(|windows| {
        windows.borrow_mut().insert(id, Rc::downgrade(&window));
    })
}

pub(crate) fn unregister_window(id: winit::window::WindowId) {
    ALL_WINDOWS.with(|windows| {
        windows.borrow_mut().remove(&id);
    })
}

/// This enum captures run-time specific events that can be dispatched to the event loop in
/// addition to the winit events.
#[derive(Debug)]
pub enum CustomEvent {
    /// Request for the event loop to wake up and poll. This is used on the web for example to
    /// request an animation frame.
    WakeUpAndPoll,
}

/// This is the main structure to hold the event loop responsible for delegating events from the
/// windowing system to the individual windows managed by the run-time, and then subsequently to
/// the items. These are typically rendering and input events.
pub struct EventLoop {
    winit_loop: winit::event_loop::EventLoop<CustomEvent>,
}

impl EventLoop {
    /// Returns a new instance of the event loop, backed by a winit eventloop.
    pub fn new() -> Self {
        Self { winit_loop: winit::event_loop::EventLoop::with_user_event() }
    }

    /// Runs the event loop and renders the items in the provided `component` in its
    /// own window.
    #[allow(unused_mut)] // mut need changes for wasm
    pub fn run(mut self) {
        use winit::event::Event;
        use winit::event_loop::{ControlFlow, EventLoopWindowTarget};

        let mut cursor_pos = winit::dpi::PhysicalPosition::new(0., 0.);
        let mut pressed = false;
        let mut run_fn = move |event: Event<CustomEvent>,
                               _: &EventLoopWindowTarget<CustomEvent>,
                               control_flow: &mut ControlFlow| {
            *control_flow = ControlFlow::Wait;

            match event {
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::CloseRequested,
                    ..
                } => *control_flow = winit::event_loop::ControlFlow::Exit,
                winit::event::Event::RedrawRequested(id) => {
                    crate::animations::update_animations();
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&id).map(|weakref| weakref.upgrade())
                        {
                            window.draw();
                        }
                    });
                }
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::Resized(size),
                    window_id,
                } => {
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            window.with_platform_window(&|platform_window| {
                                window.set_scale_factor(platform_window.scale_factor() as f32);
                            });
                            window.set_width(size.width as f32);
                            window.set_height(size.height as f32);
                        }
                    });
                }
                winit::event::Event::WindowEvent {
                    event:
                        winit::event::WindowEvent::ScaleFactorChanged {
                            scale_factor,
                            new_inner_size: size,
                        },
                    window_id,
                } => {
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            window.set_width(size.width as f32);
                            window.set_height(size.height as f32);
                            window.set_scale_factor(scale_factor as f32);
                        }
                    });
                }

                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::MouseInput { state, .. },
                    ..
                } => {
                    crate::animations::update_animations();
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            let what = match state {
                                winit::event::ElementState::Pressed => {
                                    pressed = true;
                                    MouseEventType::MousePressed
                                }
                                winit::event::ElementState::Released => {
                                    pressed = false;
                                    MouseEventType::MouseReleased
                                }
                            };
                            window.clone().process_mouse_input(cursor_pos, what);
                            // FIXME: remove this, it should be based on actual changes rather than this
                            window.request_redraw();
                        }
                    });
                }
                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::Touch(touch),
                    ..
                } => {
                    crate::animations::update_animations();
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            let cursor_pos = touch.location;
                            let what = match touch.phase {
                                winit::event::TouchPhase::Started => {
                                    pressed = true;
                                    MouseEventType::MousePressed
                                }
                                winit::event::TouchPhase::Ended
                                | winit::event::TouchPhase::Cancelled => {
                                    pressed = false;
                                    MouseEventType::MouseReleased
                                }
                                winit::event::TouchPhase::Moved => MouseEventType::MouseMoved,
                            };
                            window.clone().process_mouse_input(cursor_pos, what);
                            // FIXME: remove this, it should be based on actual changes rather than this
                            window.request_redraw();
                        }
                    });
                }
                winit::event::Event::WindowEvent {
                    window_id,
                    event: winit::event::WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    cursor_pos = position;
                    crate::animations::update_animations();
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            window
                                .clone()
                                .process_mouse_input(cursor_pos, MouseEventType::MouseMoved);
                            // FIXME: remove this, it should be based on actual changes rather than this
                            window.request_redraw();
                        }
                    });
                }
                // On the html canvas, we don't get the mouse move or release event when outside the canvas. So we have no choice but canceling the event
                #[cfg(target_arch = "wasm32")]
                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::CursorLeft { .. },
                    ..
                } => {
                    if pressed {
                        crate::animations::update_animations();
                        ALL_WINDOWS.with(|windows| {
                            if let Some(Some(window)) =
                                windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                            {
                                pressed = false;
                                window
                                    .clone()
                                    .process_mouse_input(cursor_pos, MouseEventType::MouseExit);
                                // FIXME: remove this, it should be based on actual changes rather than this
                                window.request_redraw();
                            }
                        });
                    }
                }

                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::KeyboardInput { ref input, .. },
                } => {
                    crate::animations::update_animations();
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            if let Some(ref key_event) =
                                (input, window.current_keyboard_modifiers()).try_into().ok()
                            {
                                window.clone().process_key_input(key_event);
                                // FIXME: remove this, it should be based on actual changes rather than this
                                window.request_redraw();
                            }
                        }
                    });
                }
                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::ReceivedCharacter(ch),
                } => {
                    if !ch.is_control() {
                        crate::animations::update_animations();
                        ALL_WINDOWS.with(|windows| {
                            if let Some(Some(window)) =
                                windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                            {
                                let modifiers = window.current_keyboard_modifiers();

                                if !modifiers.control() && !modifiers.alt() && !modifiers.logo() {
                                    let key_event = KeyEvent::CharacterInput {
                                        unicode_scalar: ch.into(),
                                        modifiers,
                                    };
                                    window.clone().process_key_input(&key_event);
                                    // FIXME: remove this, it should be based on actual changes rather than this
                                    window.request_redraw();
                                }
                            }
                        });
                    }
                }
                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::ModifiersChanged(state),
                } => {
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            window.set_current_keyboard_modifiers(state.into());
                        }
                    });
                }

                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::Focused(have_focus),
                } => {
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            window.clone().set_focus(have_focus);
                            // FIXME: remove this, it should be based on actual changes rather than this
                            window.request_redraw();
                        }
                    });
                }

                _ => (),
            }

            if *control_flow != winit::event_loop::ControlFlow::Exit {
                crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
                    if !driver.has_active_animations() {
                        return;
                    }
                    *control_flow = ControlFlow::Poll;
                    ALL_WINDOWS.with(|windows| {
                        windows.borrow().values().for_each(|window| {
                            if let Some(window) = window.upgrade() {
                                window.request_redraw();
                            }
                        })
                    })
                })
            }

            if crate::timers::TimerList::maybe_activate_timers() {
                ALL_WINDOWS.with(|windows| {
                    windows.borrow().values().for_each(|window| {
                        if let Some(window) = window.upgrade() {
                            window.request_redraw();
                        }
                    })
                })
            }

            if *control_flow == winit::event_loop::ControlFlow::Wait {
                if let Some(next_timer) = crate::timers::TimerList::next_timeout() {
                    *control_flow = winit::event_loop::ControlFlow::WaitUntil(next_timer);
                }
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        self.winit_loop.run_return(run_fn);
        #[cfg(target_arch = "wasm32")]
        {
            // Since wasm does not have a run_return function that takes a non-static closure,
            // we use this hack to work that around
            scoped_tls_hkt::scoped_thread_local!(static mut RUN_FN_TLS: for <'a> &'a mut dyn FnMut(
                Event<'_, CustomEvent>,
                &EventLoopWindowTarget<CustomEvent>,
                &mut ControlFlow,
            ));
            RUN_FN_TLS.set(&mut run_fn, move || {
                self.winit_loop.run(|e, t, cf| RUN_FN_TLS.with(|mut run_fn| run_fn(e, t, cf)))
            });
        }
    }

    /// Returns a reference to the backing winit event loop.
    pub fn get_winit_event_loop(&self) -> &winit::event_loop::EventLoop<CustomEvent> {
        &self.winit_loop
    }
}

/// This module contains the functions needed to interface with the event loop and window traits
/// from outside the Rust language.
pub mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Same layout as ComponentWindow (fat pointer)
    #[repr(C)]
    pub struct ComponentWindowOpaque(*const c_void, *const c_void);

    /// Releases the reference to the component window held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_drop(handle: *mut ComponentWindowOpaque) {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        core::ptr::read(handle as *mut ComponentWindow);
    }

    /// Releases the reference to the component window held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_clone(
        source: *const ComponentWindowOpaque,
        target: *mut ComponentWindowOpaque,
    ) {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        let window = &*(source as *const ComponentWindow);
        core::ptr::write(target as *mut ComponentWindow, window.clone());
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_run(handle: *const ComponentWindowOpaque) {
        let window = &*(handle as *const ComponentWindow);
        window.run();
    }

    /// Returns the window scale factor.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_get_scale_factor(
        handle: *const ComponentWindowOpaque,
    ) -> f32 {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        let window = &*(handle as *const ComponentWindow);
        window.scale_factor()
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_set_scale_factor(
        handle: *const ComponentWindowOpaque,
        value: f32,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.set_scale_factor(value)
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_free_graphics_resources<'a>(
        handle: *const ComponentWindowOpaque,
        items: &Slice<'a, Pin<ItemRef<'a>>>,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.free_graphics_resources(items)
    }

    /// Sets the focus item.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_set_focus_item(
        handle: *const ComponentWindowOpaque,
        focus_item: &ItemRc,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.set_focus_item(focus_item)
    }

    /// Associates the window with the given component.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_set_component(
        handle: *const ComponentWindowOpaque,
        component: &ComponentRc,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.set_component(component)
    }

    /// Show a popup.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_show_popup(
        handle: *const ComponentWindowOpaque,
        popup: &ComponentRc,
        position: crate::graphics::Point,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.show_popup(popup, position);
    }
    /// Close the current popup
    pub unsafe extern "C" fn sixtyfps_component_window_close_popup(
        handle: *const ComponentWindowOpaque,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.close_popup();
    }
}
