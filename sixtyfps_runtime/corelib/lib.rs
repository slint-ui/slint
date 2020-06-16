/*!

# SixtyFPS runtime library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/

#![deny(unsafe_code)]

/// The animation system
pub mod animations;
pub mod graphics;
pub mod input;
pub mod item_tree;
pub mod layout;

#[cfg(feature = "rtti")]
pub mod rtti;

/// Things that are exposed to the C ABI
pub mod abi {
    #![warn(missing_docs)]
    // We need to allow unsafe functions because of FFI
    #![allow(unsafe_code)]
    pub mod datastructures;
    pub mod model;
    pub mod primitives;
    pub mod properties;
    pub mod signals;
    pub mod slice;
    pub mod string;
}

#[doc(inline)]
pub use abi::string::SharedString;

#[doc(inline)]
pub use abi::datastructures::Resource;

#[doc(inline)]
pub use abi::properties::{EvaluationContext, Property};

#[doc(inline)]
pub use abi::signals::Signal;

mod item_rendering;

use abi::datastructures::Color;
#[cfg(not(target_arch = "wasm32"))]
use winit::platform::desktop::EventLoopExtDesktop;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

trait GenericWindow {
    fn draw(&self, component: vtable::VRef<crate::abi::datastructures::ComponentVTable>);
    fn process_mouse_input(
        &self,
        pos: winit::dpi::PhysicalPosition<f64>,
        state: winit::event::ElementState,
        component: vtable::VRef<crate::abi::datastructures::ComponentVTable>,
    );
    fn window_handle(&self) -> std::cell::Ref<'_, winit::window::Window>;
}

thread_local! {
    static ALL_WINDOWS: RefCell<std::collections::HashMap<winit::window::WindowId, Weak<dyn GenericWindow>>> = RefCell::new(std::collections::HashMap::new());
}

pub struct MainWindow<GraphicsBackend: graphics::GraphicsBackend> {
    pub graphics_backend: GraphicsBackend,
    pub rendering_cache: graphics::RenderingCache<GraphicsBackend>,
}

impl<GraphicsBackend: graphics::GraphicsBackend + 'static> MainWindow<GraphicsBackend> {
    pub fn new(
        event_loop: &winit::event_loop::EventLoop<()>,
        graphics_backend_factory: impl FnOnce(
            &winit::event_loop::EventLoop<()>,
            winit::window::WindowBuilder,
        ) -> GraphicsBackend,
    ) -> Rc<RefCell<Self>> {
        let window_builder = winit::window::WindowBuilder::new();

        let this = Rc::new(RefCell::new(Self {
            graphics_backend: graphics_backend_factory(&event_loop, window_builder),
            rendering_cache: graphics::RenderingCache::default(),
        }));

        ALL_WINDOWS.with(|windows| {
            windows
                .borrow_mut()
                .insert(this.borrow().id(), Rc::downgrade(&(this.clone() as Rc<dyn GenericWindow>)))
        });

        this
    }

    pub fn id(&self) -> winit::window::WindowId {
        self.graphics_backend.window().id()
    }
}

impl<GraphicsBackend: graphics::GraphicsBackend> Drop for MainWindow<GraphicsBackend> {
    fn drop(&mut self) {
        ALL_WINDOWS.with(|windows| {
            windows.borrow_mut().remove(&self.graphics_backend.window().id());
        });
    }
}

impl<GraphicsBackend: graphics::GraphicsBackend> GenericWindow
    for RefCell<MainWindow<GraphicsBackend>>
{
    fn draw(&self, component: vtable::VRef<abi::datastructures::ComponentVTable>) {
        // FIXME: we should do that only if some property change
        component.compute_layout();

        let mut this = self.borrow_mut();

        {
            let mut rendering_primitives_builder =
                this.graphics_backend.new_rendering_primitives_builder();

            // Generate cached rendering data once
            crate::item_tree::visit_items(
                component,
                |item, _| {
                    let ctx = EvaluationContext { component };
                    item_rendering::update_item_rendering_data(
                        &ctx,
                        item,
                        &mut this.rendering_cache,
                        &mut rendering_primitives_builder,
                    );
                },
                (),
            );

            this.graphics_backend.finish_primitives(rendering_primitives_builder);
        }

        let window = this.graphics_backend.window();

        let size = window.inner_size();
        let context = EvaluationContext { component: component };
        let mut frame = this.graphics_backend.new_frame(size.width, size.height, &Color::WHITE);
        item_rendering::render_component_items(
            component,
            &context,
            &mut frame,
            &mut this.rendering_cache,
        );
        this.graphics_backend.present_frame(frame);
    }
    fn process_mouse_input(
        &self,
        pos: winit::dpi::PhysicalPosition<f64>,
        state: winit::event::ElementState,
        component: vtable::VRef<abi::datastructures::ComponentVTable>,
    ) {
        let context = EvaluationContext { component };
        input::process_mouse_event(
            component,
            &context,
            crate::abi::datastructures::MouseEvent {
                pos: euclid::point2(pos.x as _, pos.y as _),
                what: match state {
                    winit::event::ElementState::Pressed => {
                        crate::abi::datastructures::MouseEventType::MousePressed
                    }
                    winit::event::ElementState::Released => {
                        crate::abi::datastructures::MouseEventType::MouseReleased
                    }
                },
            },
        );
    }
    fn window_handle(&self) -> std::cell::Ref<'_, winit::window::Window> {
        std::cell::Ref::map(self.borrow(), |mw| mw.graphics_backend.window())
    }
}

#[allow(unused_mut)] // mut need changes for wasm
fn run_event_loop(
    mut event_loop: winit::event_loop::EventLoop<()>,
    component: vtable::VRef<crate::abi::datastructures::ComponentVTable>,
) {
    use winit::event::Event;
    use winit::event_loop::{ControlFlow, EventLoopWindowTarget};

    let mut cursor_pos = winit::dpi::PhysicalPosition::new(0., 0.);
    let mut run_fn =
        move |event: Event<()>, _: &EventLoopWindowTarget<()>, control_flow: &mut ControlFlow| {
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
    event_loop.run_return(run_fn);
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
            event_loop.run(|e, t, cf| RUN_FN_TLS.with(|mut run_fn| run_fn(e, t, cf)))
        });
    }
}
pub fn run_component<GraphicsBackend: graphics::GraphicsBackend + 'static>(
    component: vtable::VRef<crate::abi::datastructures::ComponentVTable>,
    graphics_backend_factory: impl FnOnce(
        &winit::event_loop::EventLoop<()>,
        winit::window::WindowBuilder,
    ) -> GraphicsBackend,
) {
    let event_loop = winit::event_loop::EventLoop::new();
    let _main_window = MainWindow::new(&event_loop, graphics_backend_factory);

    run_event_loop(event_loop, component);
}
