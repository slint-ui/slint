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

mod eventloop;
mod item_rendering;

use abi::datastructures::Color;

use std::cell::RefCell;
use std::rc::Rc;

pub struct GraphicsWindow<GraphicsBackend: graphics::GraphicsBackend + 'static> {
    graphics_backend_factory: Box<
        dyn Fn(&winit::event_loop::EventLoop<()>, winit::window::WindowBuilder) -> GraphicsBackend,
    >,
    graphics_backend: Option<GraphicsBackend>,
    rendering_cache: graphics::RenderingCache<GraphicsBackend>,
}

impl<GraphicsBackend: graphics::GraphicsBackend + 'static> GraphicsWindow<GraphicsBackend> {
    pub fn new(
        graphics_backend_factory: impl Fn(&winit::event_loop::EventLoop<()>, winit::window::WindowBuilder) -> GraphicsBackend
            + 'static,
    ) -> Rc<RefCell<Self>> {
        let this = Rc::new(RefCell::new(Self {
            graphics_backend_factory: Box::new(graphics_backend_factory),
            graphics_backend: None,
            rendering_cache: graphics::RenderingCache::default(),
        }));

        this
    }

    pub fn id(&self) -> Option<winit::window::WindowId> {
        self.graphics_backend.as_ref().map(|backend| backend.window().id())
    }
}

impl<GraphicsBackend: graphics::GraphicsBackend> Drop for GraphicsWindow<GraphicsBackend> {
    fn drop(&mut self) {
        if let Some(backend) = self.graphics_backend.as_ref() {
            eventloop::ALL_WINDOWS.with(|windows| {
                windows.borrow_mut().remove(&backend.window().id());
            });
        }
    }
}

impl<GraphicsBackend: graphics::GraphicsBackend> eventloop::GenericWindow
    for RefCell<GraphicsWindow<GraphicsBackend>>
{
    fn draw(&self, component: vtable::VRef<abi::datastructures::ComponentVTable>) {
        // FIXME: we should do that only if some property change
        component.compute_layout();

        let mut this = self.borrow_mut();

        {
            let mut rendering_primitives_builder =
                this.graphics_backend.as_mut().unwrap().new_rendering_primitives_builder();

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

            this.graphics_backend.as_mut().unwrap().finish_primitives(rendering_primitives_builder);
        }

        let window = this.graphics_backend.as_ref().unwrap().window();

        let size = window.inner_size();
        let context = EvaluationContext { component: component };
        let mut frame = this.graphics_backend.as_mut().unwrap().new_frame(
            size.width,
            size.height,
            &Color::WHITE,
        );
        item_rendering::render_component_items(
            component,
            &context,
            &mut frame,
            &mut this.rendering_cache,
        );
        this.graphics_backend.as_mut().unwrap().present_frame(frame);
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
    fn window_handle(&self) -> std::cell::Ref<winit::window::Window> {
        std::cell::Ref::map(self.borrow(), |mw| mw.graphics_backend.as_ref().unwrap().window())
    }
    fn map_window(self: Rc<Self>, event_loop: &winit::event_loop::EventLoop<()>) {
        if self.borrow().graphics_backend.is_some() {
            return;
        }

        let id = {
            let window_builder = winit::window::WindowBuilder::new();

            let mut this = self.borrow_mut();
            let factory = this.graphics_backend_factory.as_mut();
            let backend = factory(&event_loop, window_builder);

            let window_id = backend.window().id();

            this.graphics_backend = Some(backend);

            window_id
        };

        eventloop::ALL_WINDOWS.with(|windows| {
            windows
                .borrow_mut()
                .insert(id, Rc::downgrade(&(self.clone() as Rc<dyn eventloop::GenericWindow>)))
        });
    }
}

pub fn run_component<GraphicsBackend: graphics::GraphicsBackend + 'static>(
    component: vtable::VRef<crate::abi::datastructures::ComponentVTable>,
    graphics_backend_factory: impl Fn(&winit::event_loop::EventLoop<()>, winit::window::WindowBuilder) -> GraphicsBackend
        + 'static,
) {
    use eventloop::GenericWindow;

    let window = GraphicsWindow::new(graphics_backend_factory);

    let event_loop = winit::event_loop::EventLoop::new();
    window.clone().map_window(&event_loop);

    eventloop::run(event_loop, component);
}
