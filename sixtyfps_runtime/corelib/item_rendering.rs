use super::abi::datastructures::{Item, RenderingInfo};
use super::graphics::{Color, Frame, GraphicsBackend, RenderingCache, RenderingPrimitivesBuilder};
use cgmath::{Matrix4, SquareMatrix, Vector3};
use lyon::math::{Point, Rect, Size};

pub(crate) fn update_item_rendering_data<Backend: GraphicsBackend>(
    item: &mut Item,
    rendering_cache: &mut RenderingCache<Backend>,
    rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
) {
    let item_rendering_info = {
        match item.rendering_info() {
            Some(info) => info,
            None => return,
        }
    };

    println!("Caching ... {:?}", item_rendering_info);

    let rendering_data = item.cached_rendering_data_mut();

    match item_rendering_info {
        RenderingInfo::Rectangle(_x, _y, width, height, color) => {
            if width <= 0. || height <= 0. {
                return;
            }
            let primitive = rendering_primitives_builder.create_rect_primitive(
                0.,
                0.,
                width,
                height,
                Color::from_argb_encoded(color),
            );

            rendering_data.cache_index = rendering_cache.allocate_entry(primitive);

            rendering_data.cache_ok = true;
        }
        RenderingInfo::Image(_source) => {
            rendering_data.cache_ok = false;
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut image_path = std::env::current_exe().unwrap();
                image_path.pop(); // pop of executable name
                image_path.push(_source);
                let image = image::open(image_path.as_path()).unwrap().into_rgba();
                let source_size = image.dimensions();

                let source_rect = Rect::new(
                    Point::new(0.0, 0.0),
                    Size::new(source_size.0 as f32, source_size.1 as f32),
                );
                let dest_rect = Rect::new(
                    Point::new(200.0, 200.0),
                    Size::new(source_size.0 as f32, source_size.1 as f32),
                );

                let image_primitive = rendering_primitives_builder.create_image_primitive(
                    source_rect,
                    dest_rect,
                    image,
                );
                rendering_data.cache_index = rendering_cache.allocate_entry(image_primitive);
                rendering_data.cache_ok = true;
            }
        }
        RenderingInfo::NoContents => {
            rendering_data.cache_ok = false;
        }
    }
}

pub(crate) fn render_component_items<Backend: GraphicsBackend>(
    component: vtable::VRef<'_, crate::abi::datastructures::ComponentVTable>,
    frame: &mut Backend::Frame,
    rendering_cache: &RenderingCache<Backend>,
) {
    let transform = Matrix4::identity();

    crate::abi::datastructures::visit_items(
        component,
        |item, transform| {
            let mut transform = transform.clone();
            let item_rendering_info = {
                match item.rendering_info() {
                    Some(info) => info,
                    None => return transform,
                }
            };

            match item_rendering_info {
                RenderingInfo::Rectangle(x, y, ..) => {
                    transform = transform * Matrix4::from_translation(Vector3::new(x, y, 0.0));
                }
                _ => {}
            }

            if item.cached_rendering_data().cache_ok {
                println!(
                    "Rendering... {:?} from cache {}",
                    item_rendering_info,
                    item.cached_rendering_data().cache_index
                );

                let primitive = rendering_cache.entry_at(item.cached_rendering_data().cache_index);
                frame.render_primitive(&primitive, &transform);
            }

            transform
        },
        transform,
    );
}
