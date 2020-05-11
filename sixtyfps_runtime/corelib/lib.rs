pub mod graphics;

pub mod abi {
    pub mod datastructures;
    pub mod model;
    pub mod primitives;
}

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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
