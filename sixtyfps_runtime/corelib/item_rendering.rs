use super::abi::datastructures::{ItemRefMut, RenderingInfo};
use super::graphics::{
    Color, Frame, GraphicsBackend, RenderingCache, RenderingPrimitive, RenderingPrimitivesBuilder,
};
use cgmath::{Matrix4, SquareMatrix, Vector3};

pub(crate) fn update_item_rendering_data<Backend: GraphicsBackend>(
    mut item: ItemRefMut<'_>,
    rendering_cache: &mut RenderingCache<Backend>,
    rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
) {
    let item_rendering_info = item.rendering_info();

    println!("Caching ... {:?}", item_rendering_info);

    let rendering_data = item.cached_rendering_data_offset_mut();

    match &item_rendering_info {
        RenderingInfo::Rectangle(x, y, width, height, color) => {
            if *width > 0. || *height > 0. {
                let primitive =
                    rendering_primitives_builder.create(RenderingPrimitive::Rectangle {
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                        color: Color::from_argb_encoded(*color),
                    });

                rendering_data.cache_index = rendering_cache.allocate_entry(primitive);

                rendering_data.cache_ok = true;
            }
        }
        RenderingInfo::Image(x, y, source) => {
            rendering_data.cache_ok = false;
            #[cfg(not(target_arch = "wasm32"))]
            {
                let image_primitive = rendering_primitives_builder
                    .create(RenderingPrimitive::Image { x: *x, y: *y, source: source.clone() });
                rendering_data.cache_index = rendering_cache.allocate_entry(image_primitive);
                rendering_data.cache_ok = true;
            }
        }
        RenderingInfo::Text(x, y, text, font_family, font_pixel_size, color) => {
            let primitive = rendering_primitives_builder.create(RenderingPrimitive::Text {
                x: *x,
                y: *y,
                text: text.clone(),
                font_family: font_family.clone(),
                font_pixel_size: *font_pixel_size,
                color: Color::from_argb_encoded(*color),
            });
            rendering_data.cache_index = rendering_cache.allocate_entry(primitive);
            rendering_data.cache_ok = true;
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
            let origin = item.geometry().origin;
            let transform =
                transform * Matrix4::from_translation(Vector3::new(origin.x, origin.y, 0.));

            let cached_rendering_data = item.cached_rendering_data_offset();
            if cached_rendering_data.cache_ok {
                println!(
                    "Rendering... {:?} from cache {}",
                    item.rendering_info(),
                    cached_rendering_data.cache_index
                );

                let primitive = rendering_cache.entry_at(cached_rendering_data.cache_index);
                frame.render_primitive(&primitive, &transform);
            }

            transform
        },
        transform,
    );
}
