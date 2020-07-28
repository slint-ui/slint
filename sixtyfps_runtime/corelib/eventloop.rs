use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[cfg(not(target_arch = "wasm32"))]
use winit::platform::desktop::EventLoopExtDesktop;

pub trait GenericWindow {
    fn draw(&self, component: core::pin::Pin<crate::abi::datastructures::ComponentRef>);
    fn process_mouse_input(
        &self,
        pos: winit::dpi::PhysicalPosition<f64>,
        state: winit::event::ElementState,
        component: core::pin::Pin<crate::abi::datastructures::ComponentRef>,
    );
    fn window_handle(&self) -> std::cell::Ref<'_, winit::window::Window>;
    fn map_window(self: Rc<Self>, event_loop: &EventLoop);
    fn request_redraw(&self);
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
    pub fn run(
        mut self,
        component: core::pin::Pin<crate::abi::datastructures::ComponentRef>,
        window_properties: &crate::abi::datastructures::WindowProperties,
    ) {
        use winit::event::Event;
        use winit::event_loop::{ControlFlow, EventLoopWindowTarget};
        let layout_listener = Rc::pin(crate::abi::properties::PropertyListenerScope::default());

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
                    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
                        driver.update_animations(instant::Instant::now());
                    });

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
                    event: winit::event::WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    cursor_pos = position;
                    // TODO: propagate mouse move?
                }
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::Resized(size),
                    window_id,
                } => {
                    if let Some(width_property) = window_properties.width {
                        width_property.set(size.width as f32)
                    }
                    if let Some(height_property) = window_properties.height {
                        height_property.set(size.height as f32)
                    }
                    if let Some(dpi_property) = window_properties.dpi {
                        ALL_WINDOWS.with(|windows| {
                            if let Some(Some(window)) =
                                windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                            {
                                let window = window.window_handle();
                                dpi_property.set(window.scale_factor() as f32)
                            }
                        });
                    }
                }
                winit::event::Event::WindowEvent {
                    event:
                        winit::event::WindowEvent::ScaleFactorChanged {
                            scale_factor,
                            new_inner_size: size,
                        },
                    ..
                } => {
                    if let Some(width_property) = window_properties.width {
                        width_property.set(size.width as f32)
                    }
                    if let Some(height_property) = window_properties.height {
                        height_property.set(size.height as f32)
                    }
                    if let Some(dpi_property) = window_properties.dpi {
                        dpi_property.set(scale_factor as f32)
                    }
                }

                winit::event::Event::WindowEvent {
                    ref window_id,
                    event: winit::event::WindowEvent::MouseInput { state, .. },
                    ..
                } => {
                    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
                        driver.update_animations(instant::Instant::now());
                    });
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&window_id).map(|weakref| weakref.upgrade())
                        {
                            window.process_mouse_input(cursor_pos, state, component);
                            let window = window.window_handle();
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
