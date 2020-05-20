pub mod graphics;
pub mod input;
pub mod layout;

pub mod abi {
    pub mod datastructures;
    pub mod model;
    pub mod primitives;
    pub mod properties;
    pub mod signals;
    pub mod string;
}

#[doc(inline)]
pub use abi::string::SharedString;

#[doc(inline)]
pub use abi::properties::Property;

#[doc(inline)]
pub use abi::signals::Signal;

mod item_rendering;

pub struct MainWindow<GraphicsBackend>
where
    GraphicsBackend: graphics::GraphicsBackend,
{
    pub graphics_backend: GraphicsBackend,
    event_loop: winit::event_loop::EventLoop<()>,
    pub rendering_cache: graphics::RenderingCache<GraphicsBackend>,
}

impl<GraphicsBackend> MainWindow<GraphicsBackend>
where
    GraphicsBackend: graphics::GraphicsBackend,
{
    pub fn new<FactoryFunc>(graphics_backend_factory: FactoryFunc) -> Self
    where
        FactoryFunc: FnOnce(
            &winit::event_loop::EventLoop<()>,
            winit::window::WindowBuilder,
        ) -> GraphicsBackend,
    {
        let event_loop = winit::event_loop::EventLoop::new();
        let window_builder = winit::window::WindowBuilder::new();

        let graphics_backend = graphics_backend_factory(&event_loop, window_builder);

        Self { graphics_backend, event_loop, rendering_cache: graphics::RenderingCache::default() }
    }

    pub fn run_event_loop(
        self,
        mut render_function: impl FnMut(&mut GraphicsBackend::Frame, &mut graphics::RenderingCache<GraphicsBackend>)
            + 'static,
        mut input_function: impl FnMut(winit::dpi::PhysicalPosition<f64>, winit::event::ElementState)
            + 'static,
    ) where
        GraphicsBackend: 'static,
    {
        let mut graphics_backend = self.graphics_backend;
        let mut rendering_cache = self.rendering_cache;
        let mut cursor_pos = winit::dpi::PhysicalPosition::new(0., 0.);
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Wait;

            let window = graphics_backend.window();

            match event {
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::CloseRequested,
                    ..
                } => *control_flow = winit::event_loop::ControlFlow::Exit,
                winit::event::Event::RedrawRequested(_) => {
                    let size = window.inner_size();
                    let mut frame = graphics_backend.new_frame(
                        size.width,
                        size.height,
                        &graphics::Color::WHITE,
                    );
                    render_function(&mut frame, &mut rendering_cache);
                    graphics_backend.present_frame(frame);
                }
                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    cursor_pos = position;
                    // TODO: propagate mouse move?
                }

                winit::event::Event::WindowEvent {
                    event: winit::event::WindowEvent::MouseInput { state, .. },
                    ..
                } => {
                    input_function(cursor_pos, state);
                    // FIXME: remove this, it should be based on actual changes rather than this
                    window.request_redraw();
                }

                _ => (),
            }
        });
    }
}

pub fn run_component<GraphicsBackend, GraphicsFactoryFunc>(
    mut component: vtable::VRefMut<'static, crate::abi::datastructures::ComponentVTable>,
    graphics_backend_factory: GraphicsFactoryFunc,
) where
    GraphicsBackend: graphics::GraphicsBackend + 'static,
    GraphicsFactoryFunc:
        FnOnce(&winit::event_loop::EventLoop<()>, winit::window::WindowBuilder) -> GraphicsBackend,
{
    let mut main_window = MainWindow::new(graphics_backend_factory);

    let renderer = &mut main_window.graphics_backend;
    let rendering_cache = &mut main_window.rendering_cache;

    let mut rendering_primitives_builder = renderer.new_rendering_primitives_builder();

    // Generate cached rendering data once
    crate::abi::datastructures::visit_items_mut(
        component.borrow_mut(),
        |item, _| {
            item_rendering::update_item_rendering_data(
                item,
                rendering_cache,
                &mut rendering_primitives_builder,
            );
        },
        (),
    );

    renderer.finish_primitives(rendering_primitives_builder);
    let component = component.into_ref();
    main_window.run_event_loop(
        move |frame, rendering_cache| {
            item_rendering::render_component_items(component, frame, &rendering_cache);
        },
        move |pos, state| {
            input::process_mouse_event(
                component,
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
            )
        },
    );
}
