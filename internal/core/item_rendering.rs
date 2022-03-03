// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![warn(missing_docs)]
//! module for rendering the tree of items

use super::graphics::RenderingCache;
use super::items::*;
use crate::component::ComponentRc;
use crate::graphics::Rect;
use crate::item_tree::ItemVisitorResult;
use core::cell::{Cell, RefCell};
use core::pin::Pin;

/// This structure must be present in items that are Rendered and contains information.
/// Used by the backend.
#[derive(Default, Debug)]
#[repr(C)]
pub struct CachedRenderingData {
    /// Used and modified by the backend, should be initialized to 0 by the user code
    pub(crate) cache_index: Cell<usize>,
    /// Used and modified by the backend, should be initialized to 0 by the user code.
    /// The backend compares this generation against the one of the cache to verify
    /// the validity of the cache_index field.
    pub(crate) cache_generation: Cell<usize>,
}

impl CachedRenderingData {
    /// This function allows retrieving the backend specific per-item data cache, updating
    /// it if depending properties have changed. The supplied update_fn will be called when
    /// properties have changed or the cache is initialized the first time.
    pub fn get_or_update<T: Clone>(
        &self,
        cache: &RefCell<RenderingCache<T>>,
        update_fn: impl FnOnce() -> T,
    ) -> T {
        if self.cache_generation.get() == cache.borrow().generation()
            && cache.borrow().contains(self.cache_index.get())
        {
            let index = self.cache_index.get();
            let tracker = cache.borrow_mut().get_mut(index).unwrap().dependency_tracker.take();

            let maybe_new_data =
                tracker.as_ref().and_then(|tracker| tracker.as_ref().evaluate_if_dirty(update_fn));

            let mut cache = cache.borrow_mut();
            let cache_entry = cache.get_mut(index).unwrap();
            cache_entry.dependency_tracker = tracker;

            if let Some(new_data) = maybe_new_data {
                cache_entry.data = new_data
            }

            return cache_entry.data.clone();
        }
        let cache_entry = crate::graphics::CachedGraphicsData::new(update_fn);
        self.cache_index.set(cache.borrow_mut().insert(cache_entry));
        self.cache_generation.set(cache.borrow().generation());
        cache.borrow().get(self.cache_index.get()).unwrap().data.clone()
    }

    /// This function can be used to remove an entry from the rendering cache for a given item, if it
    /// exists, i.e. if any data was ever cached. This is typically called by the graphics backend's
    /// implementation of the release_item_graphics_cache function.
    pub fn release<T>(&self, cache: &mut RenderingCache<T>) -> Option<T> {
        if self.cache_generation.get() == cache.generation() {
            let index = self.cache_index.get();
            self.cache_generation.set(0);
            Some(cache.remove(index).data)
        } else {
            None
        }
    }
}

/// Return true if the item might be a clipping item
pub(crate) fn is_clipping_item(item: Pin<ItemRef>) -> bool {
    //(FIXME: there should be some flag in the vtable instead of downcasting)
    ItemRef::downcast_pin::<Flickable>(item).is_some()
        || ItemRef::downcast_pin::<Clip>(item).is_some()
}

/// Renders the tree of items that component holds, using the specified renderer. Rendering is done
/// relative to the specified origin.
pub fn render_component_items(
    component: &ComponentRc,
    renderer: &mut dyn ItemRenderer,
    origin: crate::graphics::Point,
) {
    renderer.save_state();
    renderer.translate(origin.x, origin.y);

    let renderer = RefCell::new(renderer);

    crate::item_tree::visit_items_with_post_visit(
        component,
        crate::item_tree::TraversalOrder::BackToFront,
        |_, item, _, _| {
            renderer.borrow_mut().save_state();

            let (do_draw, item_geometry) = renderer.borrow_mut().filter_item(item);

            let item_origin = item_geometry.origin;
            renderer.borrow_mut().translate(item_origin.x, item_origin.y);

            // Don't render items that are clipped, with the exception of the Clip or Flickable since
            // they themselves clip their content.
            if !do_draw
                && !is_clipping_item(item)
                // HACK, the geometry of the box shadow does not include the shadow, because when the shadow is the root for repeated elements it would translate the children
                && ItemRef::downcast_pin::<BoxShadow>(item).is_none()
            {
                return (ItemVisitorResult::Continue(()), ());
            }
            item.as_ref().render(&mut (*renderer.borrow_mut() as &mut dyn ItemRenderer));

            (ItemVisitorResult::Continue(()), ())
        },
        |_, _, _, r| {
            renderer.borrow_mut().restore_state();
            r
        },
        (),
    );

    let renderer = renderer.into_inner();
    renderer.restore_state();
}

/// Trait used to render each items.
///
/// The item needs to be rendered relative to its (x,y) position. For example,
/// draw_rectangle should draw a rectangle in `(pos.x + rect.x, pos.y + rect.y)`
#[allow(missing_docs)]
pub trait ItemRenderer {
    fn draw_rectangle(&mut self, rect: Pin<&Rectangle>);
    fn draw_border_rectangle(&mut self, rect: Pin<&BorderRectangle>);
    fn draw_image(&mut self, image: Pin<&ImageItem>);
    fn draw_clipped_image(&mut self, image: Pin<&ClippedImage>);
    fn draw_text(&mut self, text: Pin<&Text>);
    fn draw_text_input(&mut self, text_input: Pin<&TextInput>);
    #[cfg(feature = "std")]
    fn draw_path(&mut self, path: Pin<&Path>);
    fn draw_box_shadow(&mut self, box_shadow: Pin<&BoxShadow>);
    /// Clip the further call until restore_state.
    /// radius/border_width can be used for border rectangle clip.
    /// (FIXME: consider removing radius/border_width and have another  function that take a path instead)
    fn combine_clip(&mut self, rect: Rect, radius: f32, border_width: f32);
    /// Get the current clip bounding box in the current transformed coordinate.
    fn get_current_clip(&self) -> Rect;

    fn translate(&mut self, x: f32, y: f32);
    fn rotate(&mut self, angle_in_degrees: f32);
    /// Apply the opacity (between 0 and 1) for all following items until the next call to restore_state.
    fn apply_opacity(&mut self, opacity: f32);

    fn save_state(&mut self);
    fn restore_state(&mut self);

    /// Returns the scale factor
    fn scale_factor(&self) -> f32;

    /// Draw a pixmap in position indicated by the `pos`.
    /// The pixmap will be taken from cache if the cache is valid, otherwise, update_fn will be called
    /// with a callback that need to be called once with `fn (width, height, data)` where data are the
    /// RGBA premultiplied pixel values
    fn draw_cached_pixmap(
        &mut self,
        item_cache: &CachedRenderingData,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    );

    /// Draw the given string with the specified color at current (0, 0) with the default font. Mainly
    /// used by the performance counter overlay.
    fn draw_string(&mut self, string: &str, color: crate::Color);

    /// This is called before it is being rendered (before the draw_* function).
    /// Returns
    ///  - if the item needs to be drawn (false means it is clipped or doesn't need to be drawn)
    ///  - the geometry of the item
    fn filter_item(&mut self, item: Pin<ItemRef>) -> (bool, Rect) {
        let item_geometry = item.as_ref().geometry();
        (self.get_current_clip().intersects(&item_geometry), item_geometry)
    }

    fn window(&self) -> crate::window::WindowRc;

    /// Return the internal renderer
    fn as_any(&mut self) -> &mut dyn core::any::Any;
}
