use cgmath::{Matrix4, SquareMatrix};
use core::ptr::NonNull;
use kurbo::BezPath;

pub mod graphics;
pub mod layout;

pub mod abi {
    pub mod datastructures;
    pub mod model;
    pub mod primitives;
}

use abi::datastructures::RenderingInfo;
use graphics::Frame;

pub struct MainWindow<GraphicsBackend> {
    pub graphics_backend: GraphicsBackend,
    event_loop: winit::event_loop::EventLoop<()>,
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

        Self { graphics_backend, event_loop }
    }

    pub fn run_event_loop<RenderFunction>(self, render_function: RenderFunction)
    where
        GraphicsBackend: 'static,
        RenderFunction: Fn(u32, u32, &mut GraphicsBackend) + 'static,
    {
        let mut graphics_backend = self.graphics_backend;
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
                    // TODO #4: ensure GO context is current -- see if this can be done within the runtime
                    render_function(size.width, size.height, &mut graphics_backend);
                }
                _ => (),
            }
        });
    }
}

pub fn run_component<GraphicsBackend, GraphicsFactoryFunc>(
    component_type: *const abi::datastructures::ComponentType,
    component: NonNull<abi::datastructures::ComponentImpl>,
    graphics_backend_factory: GraphicsFactoryFunc,
) where
    GraphicsBackend: graphics::GraphicsBackend + 'static,
    GraphicsFactoryFunc:
        FnOnce(&winit::event_loop::EventLoop<()>, winit::window::WindowBuilder) -> GraphicsBackend,
{
    let component = unsafe {
        abi::datastructures::ComponentUniquePtr::new(
            NonNull::new_unchecked(component_type as *mut _),
            component,
        )
    };

    let main_window = MainWindow::new(graphics_backend_factory);

    main_window.run_event_loop(move |width, height, renderer| {
        let mut frame = renderer.new_frame(width, height, &graphics::Color::WHITE);

        component.visit_items(|item| {
            let item_rendering_info = {
                match item.rendering_info() {
                    Some(info) => info,
                    None => return,
                }
            };

            println!("Rendering... {:?}", item_rendering_info);

            match item_rendering_info {
                RenderingInfo::Rectangle(x, y, width, height, color) => {
                    if width <= 0. || height <= 0. {
                        return;
                    }
                    // TODO: stop using kurbo
                    let x = x as f64;
                    let y = y as f64;
                    let width = width as f64;
                    let height = height as f64;
                    let mut rect_path = BezPath::new();
                    rect_path.move_to((x, y));
                    rect_path.line_to((x + width, y));
                    rect_path.line_to((x + width, y + height));
                    rect_path.line_to((x, y + height));
                    rect_path.close_path();
                    let primitive = renderer.create_path_fill_primitive(
                        &rect_path,
                        graphics::FillStyle::SolidColor(graphics::Color::from_argb_encoded(color)),
                    );

                    frame.render_primitive(&primitive, &Matrix4::identity());
                }
                _ => {}
            }
        });

        renderer.present_frame(frame);
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
