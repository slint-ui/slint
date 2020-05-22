use super::abi::datastructures::{Color, ItemRefMut, RenderingInfo, RenderingPrimitive};
use super::graphics::{
    Frame, GraphicsBackend, HasRenderingPrimitive, RenderingCache, RenderingPrimitivesBuilder,
};
use cgmath::{Matrix4, SquareMatrix, Vector3};

pub(crate) fn update_item_rendering_data<Backend: GraphicsBackend>(
    mut item: ItemRefMut<'_>,
    rendering_cache: &mut RenderingCache<Backend>,
    rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
) {
    let item_rendering_info = item.rendering_info();

    let item_rendering_primitive = match &item_rendering_info {
        RenderingInfo::Rectangle(x, y, width, height, color) => {
            if *width > 0. || *height > 0. {
                RenderingPrimitive::Rectangle {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                    color: Color::from_argb_encoded(*color),
                }
            } else {
                RenderingPrimitive::NoContents
            }
        }
        RenderingInfo::Image(x, y, source) => {
            RenderingPrimitive::Image { x: *x, y: *y, source: source.clone() }
        }
        RenderingInfo::Text(x, y, text, font_family, font_pixel_size, color) => {
            RenderingPrimitive::Text {
                x: *x,
                y: *y,
                text: text.clone(),
                font_family: font_family.clone(),
                font_pixel_size: *font_pixel_size,
                color: Color::from_argb_encoded(*color),
            }
        }
        RenderingInfo::NoContents => RenderingPrimitive::NoContents,
    };

    let rendering_data = item.cached_rendering_data_offset_mut();

    let last_rendering_primitive =
        rendering_data.low_level_rendering_primitive(&rendering_cache).map(|ll| ll.primitive());

    if let Some(last_rendering_primitive) = last_rendering_primitive {
        if *last_rendering_primitive == item_rendering_primitive {
            //println!("Keeping ... {:?}", item_rendering_info);
            return;
        }
    }

    println!(
        "Updating rendering primitives for ... {:?} (old data: {:?})",
        item_rendering_info, last_rendering_primitive
    );

    rendering_data.cache_index = rendering_cache
        .allocate_entry(rendering_primitives_builder.create(item_rendering_primitive));
    rendering_data.cache_ok = true;
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
