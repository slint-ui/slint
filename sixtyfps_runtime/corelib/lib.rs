use cgmath::{Matrix4, Vector3};
use lyon::math::{Point, Vector};

pub mod graphics;
pub mod layout;

pub mod abi {
    pub mod datastructures;
    pub mod model;
    pub mod primitives;
}

use abi::datastructures::RenderingInfo;
use graphics::Frame;
use graphics::RenderingPrimitivesBuilder;

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
    mut component: abi::datastructures::ComponentUniquePtr,
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
    component.visit_items_mut(
        |item, _| {
            let item_rendering_info = {
                match item.rendering_info() {
                    Some(info) => info,
                    None => return,
                }
            };

            println!("Caching ... {:?}", item_rendering_info);

            let rendering_data = item.cached_rendering_data_mut();

            match item_rendering_info {
                RenderingInfo::Rectangle(_x, _y, width, height, color) => {
                    if width <= 0. || height <= 0. {
                        return;
                    }
                    let primitive = rendering_primitives_builder.create_rect_primitive(
                        0.,
                        0.,
                        width,
                        height,
                        graphics::Color::from_argb_encoded(color),
                    );

                    rendering_data.cache_index = rendering_cache.allocate_entry(primitive);

                    rendering_data.cache_ok = true;
                }
                _ => {
                    // Cannot render this yet
                    rendering_data.cache_ok = false;
                }
            }
        },
        (),
    );

    renderer.finish_primitives(rendering_primitives_builder);

    main_window.run_event_loop(move |frame, rendering_cache| {
        let offset = Point::default();

        component.visit_items(
            |item, offset| {
                let mut offset = offset.clone();
                let item_rendering_info = {
                    match item.rendering_info() {
                        Some(info) => info,
                        None => return offset,
                    }
                };

                if !item.cached_rendering_data().cache_ok {
                    return offset;
                }

                match item_rendering_info {
                    RenderingInfo::Rectangle(x, y, ..) => offset += Vector::new(x, y),
                    _ => {}
                }

                println!(
                    "Rendering... {:?} from cache {}",
                    item_rendering_info,
                    item.cached_rendering_data().cache_index
                );

                let primitive = rendering_cache.entry_at(item.cached_rendering_data().cache_index);
                frame.render_primitive(
                    &primitive,
                    &Matrix4::from_translation(Vector3::new(offset.x, offset.y, 0.0)),
                );

                offset
            },
            offset,
        );
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
