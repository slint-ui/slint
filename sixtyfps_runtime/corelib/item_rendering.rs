/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]
//! module for rendering the tree of items

use super::graphics::{Frame, GraphicsBackend, RenderingCache, RenderingPrimitivesBuilder};
use super::items::ItemRef;
use crate::item_tree::ItemVisitorResult;
use cgmath::{Matrix4, SquareMatrix, Vector3};
use std::cell::{Cell, RefCell};

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
    pub(crate) fn ensure_up_to_date<Backend: GraphicsBackend>(
        &self,
        cache: &RefCell<RenderingCache<Backend>>,
        item: core::pin::Pin<ItemRef>,
        rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
    ) {
        let idx = if self.cache_ok.get() { Some(self.cache_index.get()) } else { None };

        self.cache_index.set(cache.borrow_mut().ensure_cached(idx, || {
            rendering_primitives_builder.create(item.as_ref().rendering_primitive())
        }));
        self.cache_ok.set(true);
    }

    fn release<Backend: GraphicsBackend>(&self, cache: &RefCell<RenderingCache<Backend>>) {
        if self.cache_ok.get() {
            let index = self.cache_index.get();
            cache.borrow_mut().free_entry(index);
        }
    }
}

pub(crate) fn update_item_rendering_data<Backend: GraphicsBackend>(
    item: core::pin::Pin<ItemRef>,
    rendering_cache: &RefCell<RenderingCache<Backend>>,
    rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
) {
    let rendering_data = item.cached_rendering_data_offset();
    rendering_data.ensure_up_to_date(rendering_cache, item, rendering_primitives_builder);
}

pub(crate) fn render_component_items<Backend: GraphicsBackend>(
    component: crate::component::ComponentRefPin,
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

pub(crate) fn free_item_rendering_data<Backend: GraphicsBackend>(
    component: crate::component::ComponentRefPin,
    rendering_cache: &RefCell<RenderingCache<Backend>>,
) {
    crate::item_tree::visit_items(
        component,
        crate::item_tree::TraversalOrder::FrontToBack,
        |_, item, _| {
            let cached_rendering_data = item.cached_rendering_data_offset();
            cached_rendering_data.release(rendering_cache);
            ItemVisitorResult::Continue(())
        },
        (),
    );
}
