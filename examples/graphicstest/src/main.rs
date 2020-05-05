use glium::glutin;
use kurbo::BezPath;
use sixtyfps_corelib::graphics::{GraphicsBackend, RenderTree};
use sixtyfps_gl_backend::{GLRenderer, GLRenderingPrimitive};

fn create_rect(
    renderer: &mut impl GraphicsBackend<RenderingPrimitive = GLRenderingPrimitive>,
    x0: f64,
    y0: f64,
) -> GLRenderingPrimitive {
    let mut rect_path = BezPath::new();
    rect_path.move_to((x0, y0));
    rect_path.line_to((x0 + 100.0, y0));
    rect_path.line_to((x0 + 100.0, y0 + 100.0));
    rect_path.line_to((x0, y0 + 100.0));
    rect_path.close_path();
    renderer.create_path_primitive(&rect_path)
}

fn main() {
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new();
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let mut renderer = GLRenderer::new(&display);

    let mut render_tree = RenderTree::<GLRenderer>::default();

    let root = {
        let root_rect = create_rect(&mut renderer, 0.0, 0.0);
        render_tree.allocate_index_with_content(Some(root_rect), None)
    };

    let translated_child_rect = {
        let child_rect = create_rect(&mut renderer, 100.0, 100.0);
        render_tree.allocate_index_with_content(Some(child_rect), None)
    };

    render_tree.node_at_mut(root).append_child(translated_child_rect);

    event_loop.run(move |event, _, control_flow| {
        let next_frame_time =
            std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        match event {
            glutin::event::Event::WindowEvent { event, .. } => match event {
                glutin::event::WindowEvent::CloseRequested => {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                    return;
                }
                _ => return,
            },
            glutin::event::Event::NewEvents(cause) => match cause {
                glutin::event::StartCause::ResumeTimeReached { .. } => (),
                glutin::event::StartCause::Init => (),
                _ => return,
            },
            _ => return,
        }

        render_tree.render(&renderer, root);
    });
}
