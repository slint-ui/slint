use super::abi::datastructures::ItemRef;
use super::graphics::{
    Frame, GraphicsBackend, HasRenderingPrimitive, RenderingCache, RenderingPrimitivesBuilder,
};
use super::EvaluationContext;
use cgmath::{Matrix4, SquareMatrix, Vector3};

pub(crate) fn update_item_rendering_data<Backend: GraphicsBackend>(
    context: &EvaluationContext,
    item: core::pin::Pin<ItemRef>,
    rendering_cache: &mut RenderingCache<Backend>,
    rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
) {
    let item_rendering_primitive = item.as_ref().rendering_primitive(context);

    let rendering_data = item.cached_rendering_data_offset();

    let last_rendering_primitive =
        rendering_data.low_level_rendering_primitive(&rendering_cache).map(|ll| ll.primitive());

    if let Some(last_rendering_primitive) = last_rendering_primitive {
        if *last_rendering_primitive == item_rendering_primitive {
            //println!("Keeping ... {:?}", item_rendering_info);
            return;
        }
    }

    rendering_data.cache_index.set(
        rendering_cache
            .allocate_entry(rendering_primitives_builder.create(item_rendering_primitive)),
    );
    rendering_data.cache_ok.set(true);
}

pub(crate) fn render_component_items<Backend: GraphicsBackend>(
    component: crate::ComponentRefPin,
    frame: &mut Backend::Frame,
    rendering_cache: &RenderingCache<Backend>,
) {
    let transform = Matrix4::identity();

    crate::item_tree::visit_items(
        component,
        |context, item, transform| {
            let origin = item.as_ref().geometry(context).origin;
            let transform =
                transform * Matrix4::from_translation(Vector3::new(origin.x, origin.y, 0.));

            let cached_rendering_data = item.cached_rendering_data_offset();
            if cached_rendering_data.cache_ok.get() {
                let primitive = rendering_cache.entry_at(cached_rendering_data.cache_index.get());
                frame.render_primitive(&primitive, &transform);
            }

            transform
        },
        transform,
    );
}
