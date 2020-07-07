use cgmath::{Matrix4, SquareMatrix, Vector3};
use sixtyfps_corelib::abi::datastructures::{
    Color, PathElement, PathElements, PathLineTo, RenderingPrimitive, Resource,
};
use sixtyfps_corelib::graphics::{
    Frame, GraphicsBackend, RenderingCache, RenderingPrimitivesBuilder,
};

use sixtyfps_rendering_backend_gl::GLRenderer;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn wasm_main() {
    main();
}

fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window_builder = winit::window::WindowBuilder::new();

    let mut renderer = GLRenderer::new(&event_loop, window_builder);

    let mut render_cache = RenderingCache::<GLRenderer>::default();

    let mut rendering_primitives_builder = renderer.new_rendering_primitives_builder();

    let root = {
        let root_rect = rendering_primitives_builder.create(RenderingPrimitive::Rectangle {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 100.,
            color: Color::from_rgb(0, 0, 255),
        });
        render_cache.allocate_entry(root_rect)
    };

    let child_rect = {
        let child_rect = rendering_primitives_builder.create(RenderingPrimitive::Rectangle {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 100.,
            color: Color::from_rgb(0, 255, 0),
        });
        render_cache.allocate_entry(child_rect)
    };

    let image_node = {
        let mut logo_path = std::env::current_exe().unwrap();
        logo_path.pop(); // pop off executable file name
        logo_path.push("..");
        logo_path.push("..");
        logo_path.push("examples");
        logo_path.push("graphicstest");
        logo_path.push("logo.png");

        let image_primitive = rendering_primitives_builder.create(RenderingPrimitive::Image {
            x: 0.,
            y: 0.,
            source: Resource::AbsoluteFilePath(logo_path.to_str().unwrap().into()),
        });

        render_cache.allocate_entry(image_primitive)
    };

    const TRIANGLE_PATH: &'static [PathElement] = &[
        PathElement::LineTo(PathLineTo { x: 100., y: 50. }),
        PathElement::LineTo(PathLineTo { x: 0., y: 100. }),
    ];
    let path_node = {
        let path_primitive = rendering_primitives_builder.create(RenderingPrimitive::Path {
            x: 50.,
            y: 300.,
            elements: PathElements::StaticElements(TRIANGLE_PATH.into()),
            fill_color: Color::from_rgb(0, 128, 255),
        });
        render_cache.allocate_entry(path_primitive)
    };

    renderer.finish_primitives(rendering_primitives_builder);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;

        match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => *control_flow = winit::event_loop::ControlFlow::Exit,
            winit::event::Event::RedrawRequested(_) => {
                let window = renderer.window();

                let size = window.inner_size();
                let mut frame = renderer.new_frame(size.width, size.height, &Color::WHITE);

                frame.render_primitive(render_cache.entry_at(root), &Matrix4::identity());
                frame.render_primitive(
                    render_cache.entry_at(child_rect),
                    &Matrix4::from_translation(Vector3::new(100., 100., 0.)),
                );
                frame.render_primitive(
                    render_cache.entry_at(image_node),
                    &Matrix4::from_translation(Vector3::new(200., 200., 0.)),
                );
                frame.render_primitive(
                    render_cache.entry_at(path_node),
                    &Matrix4::from_translation(Vector3::new(20., 200., 0.)),
                );

                renderer.present_frame(frame);
            }
            _ => (),
        }
    });

    //render_cache.free_entry(root);
    //render_cache.free_entry(child_rect);
    //render_cache.free_entry(image_node);
}
