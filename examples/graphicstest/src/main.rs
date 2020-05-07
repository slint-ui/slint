use cgmath::{Matrix4, SquareMatrix, Vector3};
#[cfg(target_arch = "wasm32")]
use glow::HasRenderLoop;
#[cfg(not(target_arch = "wasm32"))]
use glutin;
use kurbo::{BezPath, Rect};
use sixtyfps_corelib::graphics::{Color, FillStyle, GraphicsBackend, RenderTree};
use sixtyfps_gl_backend::{GLRenderer, OpaqueRenderingPrimitive};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn wasm_main() {
    main();
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    let (event_loop, windowed_context, gl_context) = {
        let event_loop = glutin::event_loop::EventLoop::new();
        let wb = glutin::window::WindowBuilder::new();
        let windowed_context =
            glutin::ContextBuilder::new().with_vsync(true).build_windowed(wb, &event_loop).unwrap();
        let windowed_context = unsafe { windowed_context.make_current().unwrap() };

        let gl_context = glow::Context::from_loader_function(|s| {
            windowed_context.get_proc_address(s) as *const _
        });

        (event_loop, windowed_context, gl_context)
    };

    #[cfg(target_arch = "wasm32")]
    let (event_loop, windowed_context, gl_context) = {
        use wasm_bindgen::JsCast;
        let canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();
        let webgl1_context = canvas
            .get_context("webgl")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::WebGlRenderingContext>()
            .unwrap();
        (
            glow::RenderLoop::from_request_animation_frame(),
            (canvas.width(), canvas.height()),
            glow::Context::from_webgl1_context(webgl1_context),
        )
    };

    let mut renderer = GLRenderer::new(gl_context);

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

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
    render_tree.node_at_mut(root).append_child(image_node);

    #[cfg(not(target_arch = "wasm32"))]
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

        let size = windowed_context.window().inner_size();
        render_tree.render(&mut renderer, size.width, size.height, root);
        windowed_context.swap_buffers().unwrap();
    });

    #[cfg(target_arch = "wasm32")]
    event_loop.run(move |_running: &mut bool| {
        render_tree.render(&mut renderer, windowed_context.0, windowed_context.1, root);
    });
}
