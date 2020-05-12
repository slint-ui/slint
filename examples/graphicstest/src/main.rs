use cgmath::{Matrix4, SquareMatrix};
use lyon::path::{math::Point, math::Rect, math::Size, Path};
use sixtyfps_corelib::{
    graphics::{Color, FillStyle, Frame, GraphicsBackend, RenderingCache},
    MainWindow,
};
use sixtyfps_gl_backend::{GLRenderer, OpaqueRenderingPrimitive};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

fn create_rect(
    renderer: &mut GLRenderer,
    x0: f32,
    y0: f32,
    color: Color,
) -> OpaqueRenderingPrimitive {
    let mut rect_path = Path::builder();
    rect_path.move_to(Point::new(x0, y0));
    rect_path.line_to(Point::new(x0 + 100.0, y0));
    rect_path.line_to(Point::new(x0 + 100.0, y0 + 100.0));
    rect_path.line_to(Point::new(x0, y0 + 100.0));
    rect_path.close();
    renderer.create_path_fill_primitive(&rect_path.build(), FillStyle::SolidColor(color))
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn wasm_main() {
    main();
}

fn main() {
    let mut main_window =
        MainWindow::new(|event_loop, window_builder| GLRenderer::new(&event_loop, window_builder));

    let mut renderer = &mut main_window.graphics_backend;

    let mut render_cache = RenderingCache::<GLRenderer>::default();

    let root = {
        let root_rect = create_rect(&mut renderer, 0.0, 0.0, Color::BLUE);
        render_cache.allocate_entry(root_rect)
    };

    let child_rect = {
        let child_rect = create_rect(&mut renderer, 100.0, 100.0, Color::GREEN);
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

        let source_rect =
            Rect::new(Point::new(0.0, 0.0), Size::new(source_size.0 as f32, source_size.1 as f32));
        let dest_rect = Rect::new(
            Point::new(200.0, 200.0),
            Size::new(source_size.0 as f32, source_size.1 as f32),
        );

        let image_primitive = renderer.create_image_primitive(source_rect, dest_rect, image);

        render_cache.allocate_entry(image_primitive)
    };

    main_window.run_event_loop(move |width, height, renderer| {
        let mut frame = renderer.new_frame(width, height, &Color::WHITE);

        frame.render_primitive(render_cache.entry_at(root), &Matrix4::identity());
        frame.render_primitive(render_cache.entry_at(child_rect), &Matrix4::identity());
        frame.render_primitive(render_cache.entry_at(image_node), &Matrix4::identity());

        renderer.present_frame(frame);
    });

    //render_cache.free_entry(root);
    //render_cache.free_entry(child_rect);
    //render_cache.free_entry(image_node);
}
