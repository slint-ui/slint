use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[cfg(not(target_arch = "wasm32"))]
use winit::platform::desktop::EventLoopExtDesktop;

pub trait GenericWindow {
    fn draw(&self, component: vtable::VRef<crate::abi::datastructures::ComponentVTable>);
    fn process_mouse_input(
        &self,
        pos: winit::dpi::PhysicalPosition<f64>,
        state: winit::event::ElementState,
        component: vtable::VRef<crate::abi::datastructures::ComponentVTable>,
    );
    fn window_handle(&self) -> std::cell::Ref<'_, winit::window::Window>;
    fn map_window(self: Rc<Self>, event_loop: &EventLoop);
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
    pub fn run(mut self, component: vtable::VRef<crate::abi::datastructures::ComponentVTable>) {
        use winit::event::Event;
        use winit::event_loop::{ControlFlow, EventLoopWindowTarget};

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
                    ALL_WINDOWS.with(|windows| {
                        if let Some(Some(window)) =
                            windows.borrow().get(&id).map(|weakref| weakref.upgrade())
                        {
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
                    ref window_id,
                    event: winit::event::WindowEvent::MouseInput { state, .. },
                    ..
                } => {
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
