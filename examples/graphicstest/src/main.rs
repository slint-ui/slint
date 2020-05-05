use cgmath::{Matrix4, SquareMatrix, Vector3};
use glium::glutin;
use kurbo::{BezPath, Rect};
use sixtyfps_corelib::graphics::{Color, FillStyle, GraphicsBackend, RenderTree};
use sixtyfps_gl_backend::{GLRenderer, OpaqueRenderingPrimitive};

fn create_rect(
    renderer: &mut impl GraphicsBackend<RenderingPrimitive = OpaqueRenderingPrimitive>,
    x0: f64,
    y0: f64,
    color: Color,
) -> OpaqueRenderingPrimitive {
    let mut rect_path = BezPath::new();
    rect_path.move_to((x0, y0));
    rect_path.line_to((x0 + 100.0, y0));
    rect_path.line_to((x0 + 100.0, y0 + 100.0));
    rect_path.line_to((x0, y0 + 100.0));
    rect_path.close_path();
    renderer.create_path_fill_primitive(&rect_path, FillStyle::SolidColor(color))
}

fn main() {
    let event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new();
    let cb = glutin::ContextBuilder::new();
    let display = glium::Display::new(wb, cb, &event_loop).unwrap();

    let mut renderer = GLRenderer::new(&display);

    let mut render_tree = RenderTree::<GLRenderer>::default();

    let root = {
        let root_rect = create_rect(&mut renderer, 0.0, 0.0, Color::BLUE);
        render_tree.allocate_index_with_content(Some(root_rect), None)
    };

    let translated_child_rect = {
        let child_rect = create_rect(&mut renderer, 0.0, 0.0, Color::GREEN);
        render_tree.allocate_index_with_content(
            Some(child_rect),
            Some(Matrix4::from_translation(Vector3::new(100., 100., 0.))),
        )
    };

    render_tree.node_at_mut(root).append_child(translated_child_rect);

    let image_node = {
        let mut logo_path = std::env::current_exe().unwrap();
        logo_path.pop(); // pop off executable file name
        logo_path.push("..");
        logo_path.push("..");
        logo_path.push("examples");
        logo_path.push("graphicstest");
        logo_path.push("logo.png");
        let im = image::open(logo_path.as_path()).unwrap().into_rgba();
        let source_size = im.dimensions();

        let source_rect = Rect::new(0.0, 0.0, source_size.0 as f64, source_size.1 as f64);
        let dest_rect =
            Rect::new(200.0, 200.0, 200. + source_size.0 as f64, 200. + source_size.1 as f64);

        let image_primitive = renderer.create_image_primitive(source_rect, dest_rect, im);

        render_tree.allocate_index_with_content(Some(image_primitive), Some(Matrix4::identity()))
    };

    render_tree.node_at_mut(root).append_child(image_node);

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
