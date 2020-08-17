/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
use crate::component::ComponentVTable;
use crate::items::ItemRef;
use std::cell::RefCell;
use std::{
    pin::Pin,
    rc::{Rc, Weak},
};
use vtable::*;

use crate::{input::MouseEventType, properties::PropertyTracker};
#[cfg(not(target_arch = "wasm32"))]
use winit::platform::desktop::EventLoopExtDesktop;

pub trait GenericWindow {
    fn draw(&self, component: core::pin::Pin<crate::component::ComponentRef>);
    fn process_mouse_input(
        &self,
        pos: winit::dpi::PhysicalPosition<f64>,
        what: MouseEventType,
        component: core::pin::Pin<crate::component::ComponentRef>,
    );
    fn with_platform_window(&self, callback: &dyn Fn(&winit::window::Window));
    fn map_window(self: Rc<Self>, event_loop: &EventLoop, root_item: Pin<ItemRef>);
    fn unmap_window(self: Rc<Self>);
    fn request_redraw(&self);
    fn scale_factor(&self) -> f32;
    fn set_scale_factor(&self, factor: f32);
    fn set_width(&self, width: f32);
    fn set_height(&self, height: f32);
    fn free_graphics_resources(
        self: Rc<Self>,
        component: core::pin::Pin<crate::component::ComponentRef>,
    );
}

/// The ComponentWindow is the (rust) facing public type that can render the items
/// of components to the screen.
#[repr(C)]
#[derive(Clone)]
pub struct ComponentWindow(std::rc::Rc<dyn crate::eventloop::GenericWindow>);

impl ComponentWindow {
    /// Creates a new instance of a CompomentWindow based on the given window implementation. Only used
    /// internally.
    pub fn new(window_impl: std::rc::Rc<dyn crate::eventloop::GenericWindow>) -> Self {
        Self(window_impl)
    }
    /// Spins an event loop and renders the items of the provided component in this window.
    pub fn run(&self, component: Pin<VRef<ComponentVTable>>, root_item: Pin<ItemRef>) {
        let event_loop = crate::eventloop::EventLoop::new();

        self.0.clone().map_window(&event_loop, root_item);

        event_loop.run(component);

        self.0.clone().unmap_window();
    }

    pub fn scale_factor(&self) -> f32 {
        self.0.scale_factor()
    }

    pub fn set_scale_factor(&self, factor: f32) {
        self.0.set_scale_factor(factor)
    }

    pub fn free_graphics_resources(
        &self,
        component: core::pin::Pin<crate::component::ComponentRef>,
    ) {
        self.0.clone().free_graphics_resources(component);
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

pub struct EventLoop {
    winit_loop: winit::event_loop::EventLoop<()>,
}

impl EventLoop {
    pub fn new() -> Self {
        Self { winit_loop: winit::event_loop::EventLoop::new() }
    }
    #[allow(unused_mut)] // mut need changes for wasm
    pub fn run(mut self, component: core::pin::Pin<crate::component::ComponentRef>) {
        use winit::event::Event;
        use winit::event_loop::{ControlFlow, EventLoopWindowTarget};
        let layout_listener = Rc::pin(PropertyTracker::default());

        let mut cursor_pos = winit::dpi::PhysicalPosition::new(0., 0.);
        let mut run_fn = move |event: Event<()>,
                               _: &EventLoopWindowTarget<()>,
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
                            if layout_listener.as_ref().is_dirty() {
                                layout_listener
                                    .as_ref()
                                    .evaluate(|| component.as_ref().compute_layout())
                            }
                            window.draw(component);
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
                            window.set_scale_factor(scale_factor as f32);
                            window.set_width(size.width as f32);
                            window.set_height(size.height as f32);
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
                                winit::event::ElementState::Pressed => MouseEventType::MousePressed,
                                winit::event::ElementState::Released => {
                                    MouseEventType::MouseReleased
                                }
                            };
                            window.process_mouse_input(cursor_pos, what, component);
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
                                winit::event::TouchPhase::Started => MouseEventType::MousePressed,
                                winit::event::TouchPhase::Ended
                                | winit::event::TouchPhase::Cancelled => {
                                    MouseEventType::MouseReleased
                                }
                                winit::event::TouchPhase::Moved => MouseEventType::MouseMoved,
                            };
                            window.process_mouse_input(cursor_pos, what, component);
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
                            window.process_mouse_input(
                                cursor_pos,
                                MouseEventType::MouseMoved,
                                component,
                            );
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
                    //println!("Scheduling a redraw due to active animations");
                    ALL_WINDOWS.with(|windows| {
                        windows.borrow().values().for_each(|window| {
                            if let Some(window) = window.upgrade() {
                                window.request_redraw();
                            }
                        })
                    })
                })
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        self.winit_loop.run_return(run_fn);
        #[cfg(target_arch = "wasm32")]
        {
            // Since wasm does not have a run_return function that takes a non-static closure,
            // we use this hack to work that around
            scoped_tls_hkt::scoped_thread_local!(static mut RUN_FN_TLS: for <'a> &'a mut dyn FnMut(
                Event<'_, ()>,
                &EventLoopWindowTarget<()>,
                &mut ControlFlow,
            ));
            RUN_FN_TLS.set(&mut run_fn, move || {
                self.winit_loop.run(|e, t, cf| RUN_FN_TLS.with(|mut run_fn| run_fn(e, t, cf)))
            });
        }
    }

    pub fn get_winit_event_loop(&self) -> &winit::event_loop::EventLoop<()> {
        &self.winit_loop
    }
}

pub mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use crate::items::ItemVTable;

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

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_run(
        handle: *mut ComponentWindowOpaque,
        component: Pin<VRef<ComponentVTable>>,
        root_item: Pin<VRef<ItemVTable>>,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.run(component, root_item);
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
        handle: *mut ComponentWindowOpaque,
        value: f32,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.set_scale_factor(value)
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_free_graphics_resources(
        handle: *const ComponentWindowOpaque,
        component: Pin<VRef<ComponentVTable>>,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.free_graphics_resources(component)
    }
}
