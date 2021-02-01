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

use super::graphics::RenderingCache;
use super::items::*;
use crate::component::ComponentRc;
use crate::graphics::{Point, Rect};
use crate::item_tree::ItemVisitorResult;
use core::pin::Pin;
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
    /// This function allows retrieving the backend specific per-item data cache, updating
    /// it if depending properties have changed. The supplied update_fn will be called when
    /// properties have changed or the cache is initialized the first time.
    pub fn ensure_up_to_date<T: Clone>(
        &self,
        cache: &mut RenderingCache<T>,
        update_fn: impl FnOnce() -> T,
    ) -> T {
        if self.cache_ok.get() {
            let index = self.cache_index.get();
            let existing_entry = cache.get_mut(index).unwrap();
            if existing_entry.dependency_tracker.is_dirty() {
                existing_entry.data = existing_entry.dependency_tracker.as_ref().evaluate(update_fn)
            }
            existing_entry.data.clone()
        } else {
            self.cache_index.set(cache.insert(crate::graphics::CachedGraphicsData::new(update_fn)));
            self.cache_ok.set(true);
            cache.get(self.cache_index.get()).unwrap().data.clone()
        }
    }

    /// This function can be used to remove an entry from the rendering cache for a given item, if it
    /// exists, i.e. if any data was ever cached. This is typically called by the graphics backend's
    /// implementation of the release_item_graphics_cache function.
    pub fn release<T>(&self, cache: &mut RenderingCache<T>) {
        if self.cache_ok.get() {
            let index = self.cache_index.get();
            cache.remove(index);
        }
    }
}

/// Renders the tree of items that component holds, using the specified renderer. Rendering is done
/// relative to the specified origin.
pub fn render_component_items(
    component: &ComponentRc,
    renderer: &mut dyn ItemRenderer,
    origin: crate::graphics::Point,
) {
    let renderer = RefCell::new(renderer);

    crate::item_tree::visit_items_with_post_visit(
        component,
        crate::item_tree::TraversalOrder::BackToFront,
        |_, item, _, translation| {
            renderer.borrow_mut().save_state();

            item.as_ref()
                .render(*translation, &mut (*renderer.borrow_mut() as &mut dyn ItemRenderer));

            let origin = item.as_ref().geometry().origin;
            let translation = *translation + euclid::Vector2D::new(origin.x, origin.y);

            (ItemVisitorResult::Continue(translation), ())
        },
        |_, _, _| {
            renderer.borrow_mut().restore_state();
        },
        origin,
    );
}

/// Trait used to render each items.
///
/// The item needs to be rendered relative to its (x,y) position. For example,
/// draw_rectangle should draw a rectangle in `(pos.x + rect.x, pos.y + rect.y)`
#[allow(missing_docs)]
pub trait ItemRenderer {
    fn draw_rectangle(&mut self, pos: Point, rect: Pin<&Rectangle>);
    fn draw_border_rectangle(&mut self, pos: Point, rect: Pin<&BorderRectangle>);
    fn draw_image(&mut self, pos: Point, image: Pin<&Image>);
    fn draw_clipped_image(&mut self, pos: Point, image: Pin<&ClippedImage>);
    fn draw_text(&mut self, pos: Point, text: Pin<&Text>);
    fn draw_text_input(&mut self, pos: Point, text_input: Pin<&TextInput>);
    fn draw_path(&mut self, pos: Point, path: Pin<&Path>);
    fn draw_box_shadow(&mut self, pos: Point, box_shadow: Pin<&BoxShadow>);
    fn combine_clip(&mut self, pos: Point, rect: Rect);
    fn save_state(&mut self);
    fn restore_state(&mut self);

    /// Returns the scale factor
    fn scale_factor(&self) -> f32;

    /// Draw a pixmap in position indicated by the `pos`.
    /// The pixmap will be taken from cache if the cache is valid, otherwise, update_fn will be called
    /// with a callback that need to be called once with `fn (width, height, data)` where data are the
    /// argb premultiplied pixel values
    fn draw_cached_pixmap(
        &mut self,
        item_cache: &CachedRenderingData,
        pos: Point,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    );

    /// Return the internal renderer
    fn as_any(&mut self) -> &mut dyn core::any::Any;
}
