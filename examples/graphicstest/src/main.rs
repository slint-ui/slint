use glium::glutin;
use kurbo::{Affine, BezPath};
use sixtyfps_corelib::graphics::GraphicsBackend;
use sixtyfps_gl_backend::GLRenderer;

fn main() {
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new();
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let mut renderer = GLRenderer::new(&display);

    let mut rect_path = BezPath::new();
    rect_path.move_to((0.0, 0.0));
    rect_path.line_to((100.0, 0.0));
    rect_path.line_to((100.0, 100.0));
    rect_path.line_to((0.0, 100.0));
    rect_path.close_path();
    let primitive = renderer.create_path_primitive(&rect_path);

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

        let mut frame = renderer.new_frame();
        frame.render_primitive(&primitive, &Affine::default());
        frame.submit();
    });
}
