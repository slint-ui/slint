use cgmath::{Matrix4, SquareMatrix, Vector3};
use kurbo::{BezPath, Rect};
use sixtyfps_corelib::graphics::{Color, FillStyle, GraphicsBackend, RenderTree};
use sixtyfps_gl_backend::{GLRenderer, OpaqueRenderingPrimitive};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use winit::{event, event_loop, window::WindowBuilder};

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
    let event_loop = event_loop::EventLoop::new();
    let window_builder = WindowBuilder::new();

    let mut renderer = GLRenderer::new(&event_loop, window_builder);

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
        #[cfg(not(target_arch = "wasm32"))]
        let image = {
            let mut logo_path = std::env::current_exe().unwrap();
            logo_path.pop(); // pop off executable file name
            logo_path.push("..");
            logo_path.push("..");
            logo_path.push("examples");
            logo_path.push("graphicstest");
            logo_path.push("logo.png");
            image::open(logo_path.as_path()).unwrap().into_rgba()
        };

        #[cfg(target_arch = "wasm32")]
        let image = {
            use std::io::Cursor;
            image::load(Cursor::new(&include_bytes!("../logo.png")[..]), image::ImageFormat::Png)
                .unwrap()
                .to_rgba()
        };

        let source_size = image.dimensions();

        let source_rect = Rect::new(0.0, 0.0, source_size.0 as f64, source_size.1 as f64);
        let dest_rect =
            Rect::new(200.0, 200.0, 200. + source_size.0 as f64, 200. + source_size.1 as f64);

        let image_primitive = renderer.create_image_primitive(source_rect, dest_rect, image);

        render_tree.allocate_index_with_content(Some(image_primitive), Some(Matrix4::identity()))
    };

    render_tree.node_at_mut(root).append_child(image_node);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = event_loop::ControlFlow::Wait;

        let window = renderer.window();

        match event {
            event::Event::WindowEvent { event: event::WindowEvent::CloseRequested, .. } => {
                *control_flow = event_loop::ControlFlow::Exit
            }
            event::Event::RedrawRequested(_) => {
                let size = window.inner_size();
                // TODO #4: ensure GO context is current -- see if this can be done within the runtime
                render_tree.render(&mut renderer, size.width, size.height, root);
            }
            _ => (),
        }
    });
}
