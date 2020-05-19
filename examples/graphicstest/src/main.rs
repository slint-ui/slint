use cgmath::{Matrix4, SquareMatrix, Vector3};
use lyon::path::{math::Point, math::Rect, math::Size};
use sixtyfps_corelib::{
    graphics::{Color, Frame, GraphicsBackend, RenderingPrimitivesBuilder},
    MainWindow,
};
use sixtyfps_gl_backend::GLRenderer;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn wasm_main() {
    main();
}

fn main() {
    let mut main_window =
        MainWindow::new(|event_loop, window_builder| GLRenderer::new(&event_loop, window_builder));

    let renderer = &mut main_window.graphics_backend;

    let render_cache = &mut main_window.rendering_cache;

    let mut rendering_primitives_builder = renderer.new_rendering_primitives_builder();

    let root = {
        let root_rect = rendering_primitives_builder.create_rect_primitive(100., 100., Color::BLUE);
        render_cache.allocate_entry(root_rect)
    };

    let child_rect = {
        let child_rect =
            rendering_primitives_builder.create_rect_primitive(100., 100., Color::GREEN);
        render_cache.allocate_entry(child_rect)
    };

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

        let dest_rect = Rect::new(
            Point::new(200.0, 200.0),
            Size::new(source_size.0 as f32, source_size.1 as f32),
        );

        let image_primitive = rendering_primitives_builder.create_image_primitive(dest_rect, image);

        render_cache.allocate_entry(image_primitive)
    };

    renderer.finish_primitives(rendering_primitives_builder);

    main_window.run_event_loop(move |frame, rendering_cache| {
        frame.render_primitive(rendering_cache.entry_at(root), &Matrix4::identity());
        frame.render_primitive(
            rendering_cache.entry_at(child_rect),
            &Matrix4::from_translation(Vector3::new(100., 100., 0.)),
        );
        frame.render_primitive(rendering_cache.entry_at(image_node), &Matrix4::identity());
    });

    //render_cache.free_entry(root);
    //render_cache.free_entry(child_rect);
    //render_cache.free_entry(image_node);
}
