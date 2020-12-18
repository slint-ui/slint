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

use super::graphics::{
    Frame, GraphicsBackend, GraphicsWindow, RenderingCache, RenderingPrimitivesBuilder,
};
use super::items::ItemRef;
use crate::component::ComponentRc;
use crate::eventloop::ComponentWindow;
use crate::item_tree::ItemVisitorResult;
use crate::slice::Slice;
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
        window: &std::rc::Rc<GraphicsWindow<Backend>>,
    ) {
        let update_fn = || {
            rendering_primitives_builder
                .create(item.as_ref().rendering_primitive(&ComponentWindow::new(window.clone())))
        };

        if self.cache_ok.get() {
            let index = self.cache_index.get();
            let mut cache = cache.borrow_mut();
            let existing_entry = cache.get_mut(index).unwrap();
            if existing_entry.dependency_tracker.is_dirty() {
                existing_entry.primitive =
                    existing_entry.dependency_tracker.as_ref().evaluate(update_fn)
            }
        } else {
            self.cache_index.set(
                cache
                    .borrow_mut()
                    .insert(crate::graphics::TrackingRenderingPrimitive::new(update_fn)),
            );
            self.cache_ok.set(true);
        }
    }

    fn release<Backend: GraphicsBackend>(&self, cache: &RefCell<RenderingCache<Backend>>) {
        if self.cache_ok.get() {
            let index = self.cache_index.get();
            cache.borrow_mut().remove(index);
        }
    }
}

pub(crate) fn update_item_rendering_data<Backend: GraphicsBackend>(
    item: core::pin::Pin<ItemRef>,
    rendering_cache: &RefCell<RenderingCache<Backend>>,
    rendering_primitives_builder: &mut Backend::RenderingPrimitivesBuilder,
    window: &std::rc::Rc<GraphicsWindow<Backend>>,
) {
    let rendering_data = item.cached_rendering_data_offset();
    rendering_data.ensure_up_to_date(rendering_cache, item, rendering_primitives_builder, window);
}

pub(crate) fn render_component_items<Backend: GraphicsBackend>(
    component: &ComponentRc,
    frame: &mut Backend::Frame,
    rendering_cache: &RefCell<RenderingCache<Backend>>,
    window: &std::rc::Rc<GraphicsWindow<Backend>>,
    origin: crate::graphics::Point,
) {
    let window = ComponentWindow::new(window.clone());

    let frame = RefCell::new(frame);

    crate::item_tree::visit_items_with_post_visit(
        component,
        crate::item_tree::TraversalOrder::BackToFront,
        |_, item, _, translation| {
            let origin = item.as_ref().geometry().origin;
            let translation = *translation + euclid::Vector2D::new(origin.x, origin.y);

            let cached_rendering_data = item.cached_rendering_data_offset();
            let cleanup_primitives = if cached_rendering_data.cache_ok.get() {
                let cache = rendering_cache.borrow();
                let primitive =
                    &cache.get(cached_rendering_data.cache_index.get()).unwrap().primitive;
                frame.borrow_mut().render_primitive(
                    &primitive,
                    translation,
                    item.as_ref().rendering_variables(&window),
                )
            } else {
                Vec::new()
            };

            (ItemVisitorResult::Continue(translation), (translation, cleanup_primitives))
        },
        |_, _, (translation, cleanup_primitives)| {
            cleanup_primitives.into_iter().for_each(|primitive| {
                frame.borrow_mut().render_primitive(&primitive, translation, Default::default());
            })
        },
        origin,
    );
}

pub(crate) fn free_item_rendering_data<'a, Backend: GraphicsBackend>(
    items: &Slice<'a, core::pin::Pin<ItemRef<'a>>>,
    rendering_cache: &RefCell<RenderingCache<Backend>>,
) {
    for item in items.iter() {
        let cached_rendering_data = item.cached_rendering_data_offset();
        cached_rendering_data.release(rendering_cache);
    }
}
