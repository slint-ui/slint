pub mod graphics;
pub mod layout;

pub mod abi {
    pub mod datastructures;
    pub mod model;
    pub mod primitives;
    pub mod string;
}

#[doc(inline)]
pub use abi::string::SharedString;

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

    pub fn run_event_loop<RenderFunction>(self, mut render_function: RenderFunction)
    where
        GraphicsBackend: 'static,
        RenderFunction: FnMut(&mut GraphicsBackend::Frame, &mut graphics::RenderingCache<GraphicsBackend>)
            + 'static,
    {
        let mut graphics_backend = self.graphics_backend;
        let mut rendering_cache = self.rendering_cache;
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
    main_window.run_event_loop(move |frame, rendering_cache| {
        item_rendering::render_component_items(component, frame, &rendering_cache);
    });
}
