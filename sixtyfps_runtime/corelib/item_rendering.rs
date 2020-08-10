use super::abi::datastructures::ItemRef;
use super::graphics::{Frame, GraphicsBackend, RenderingCache, RenderingPrimitivesBuilder};
use crate::item_tree::ItemVisitorResult;
use cgmath::{Matrix4, SquareMatrix, Vector3};
use std::cell::Cell;

/// This structure must be present in items that are Rendered and contains information.
/// Used by the backend.
#[derive(Default, Debug)]
#[repr(C)]
pub struct CachedRenderingData {
    /// Used and modified by the backend, should be initialized to 0 by the user code
    pub(crate) cache_index: Cell<usize>,
    /// Set to false initially and when changes happen that require updating the cache
    pub(crate) cache_ok: Cell<bool>,
}

impl CachedRenderingData {
    pub(crate) fn ensure_up_to_date<'a, Backend: GraphicsBackend>(
        &self,
        cache: &'a mut RenderingCache<Backend>,
        item: core::pin::Pin<ItemRef>,
        rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
    ) {
        let idx = if self.cache_ok.get() { Some(self.cache_index.get()) } else { None };

        self.cache_index.set(cache.ensure_cached(idx, || {
            rendering_primitives_builder.create(item.as_ref().rendering_primitive())
        }));
        self.cache_ok.set(true);
    }
}

pub(crate) fn update_item_rendering_data<Backend: GraphicsBackend>(
    item: core::pin::Pin<ItemRef>,
    rendering_cache: &mut RenderingCache<Backend>,
    rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
) {
    let rendering_data = item.cached_rendering_data_offset();
    rendering_data.ensure_up_to_date(rendering_cache, item, rendering_primitives_builder);
}

pub(crate) fn render_component_items<Backend: GraphicsBackend>(
    component: crate::ComponentRefPin,
    frame: &mut Backend::Frame,
    rendering_cache: &RenderingCache<Backend>,
) {
    let transform = Matrix4::identity();

    crate::item_tree::visit_items(
        component,
        crate::item_tree::TraversalOrder::BackToFront,
        |_, item, transform| {
            let origin = item.as_ref().geometry().origin;
            let transform =
                transform * Matrix4::from_translation(Vector3::new(origin.x, origin.y, 0.));

            let cached_rendering_data = item.cached_rendering_data_offset();
            if cached_rendering_data.cache_ok.get() {
                let primitive = rendering_cache.entry_at(cached_rendering_data.cache_index.get());
                frame.render_primitive(&primitive, &transform, item.as_ref().rendering_variables());
            }

            ItemVisitorResult::Continue(transform)
        },
        transform,
    );
}
