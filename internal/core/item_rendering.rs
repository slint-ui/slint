// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![warn(missing_docs)]
//! module for rendering the tree of items

use super::graphics::RenderingCache;
use super::items::*;
use crate::component::ComponentRc;
use crate::graphics::{CachedGraphicsData, Point, Rect};
use crate::item_tree::ItemVisitorResult;
use alloc::boxed::Box;
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
        let mut cache_borrow = cache.borrow_mut();
        if let Some(entry) = self.get_entry(&mut cache_borrow) {
            let index = self.cache_index.get();
            let tracker = entry.dependency_tracker.take();
            drop(cache_borrow);

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
        drop(cache_borrow);
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

    /// Return the value if it is in the cache
    pub fn get_entry<'a, T>(
        &self,
        cache: &'a mut RenderingCache<T>,
    ) -> Option<&'a mut crate::graphics::CachedGraphicsData<T>> {
        let index = self.cache_index.get();
        if self.cache_generation.get() == cache.generation() {
            cache.get_mut(index)
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
    origin: Point,
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

/// The cache that needs to be held by the Window for the partial rendering
pub type PartialRenderingCache = RenderingCache<Rect>;

/// FIXME: Should actually be a region and not just a rectangle
pub type DirtyRegion = euclid::default::Box2D<f32>;

/// Put this structure in the renderer to help with partial rendering
pub struct PartialRenderer<'a, T> {
    cache: &'a mut PartialRenderingCache,
    /// The region of the screen which is considered dirty and that should be repainted
    pub dirty_region: DirtyRegion,
    actual_renderer: T,
}

impl<'a, T> PartialRenderer<'a, T> {
    /// Create a new PartialRenderer
    pub fn new(cache: &'a mut PartialRenderingCache, actual_renderer: T) -> Self {
        Self { cache, dirty_region: Default::default(), actual_renderer }
    }

    /// Visit the tree of item and compute what are the dirty regions
    pub fn compute_dirty_regions(&mut self, component: &ComponentRc, origin: Point) {
        crate::properties::evaluate_no_tracking(|| {
            crate::item_tree::visit_items(
                component,
                crate::item_tree::TraversalOrder::BackToFront,
                |_, item, _, offset| match item
                    .cached_rendering_data_offset()
                    .get_entry(&mut self.cache)
                {
                    Some(CachedGraphicsData { data, dependency_tracker: Some(tr) }) => {
                        if tr.is_dirty() {
                            let geom = item.as_ref().geometry();
                            let old_geom = *data;
                            self.mark_dirty_rect(old_geom, *offset);
                            self.mark_dirty_rect(geom, *offset);
                            ItemVisitorResult::Continue(*offset + geom.origin.to_vector())
                        } else {
                            ItemVisitorResult::Continue(*offset + data.origin.to_vector())
                        }
                    }
                    _ => {
                        let geom = item.as_ref().geometry();
                        self.mark_dirty_rect(geom, *offset);
                        ItemVisitorResult::Continue(*offset + geom.origin.to_vector())
                    }
                },
                origin.to_vector(),
            )
        });
    }

    fn mark_dirty_rect(&mut self, rect: Rect, offset: euclid::default::Vector2D<f32>) {
        if !rect.is_empty() {
            self.dirty_region = self.dirty_region.union(&rect.translate(offset).to_box2d());
        }
    }

    fn do_rendering(
        cache: &mut PartialRenderingCache,
        rendering_data: &CachedRenderingData,
        render_fn: impl FnOnce() -> Rect,
    ) {
        if let Some(entry) = rendering_data.get_entry(cache) {
            entry
                .dependency_tracker
                .get_or_insert_with(|| Box::pin(crate::properties::PropertyTracker::default()))
                .as_ref()
                .evaluate(render_fn);
        } else {
            let cache_entry = crate::graphics::CachedGraphicsData::new(render_fn);
            rendering_data.cache_index.set(cache.insert(cache_entry));
            rendering_data.cache_generation.set(cache.generation());
        }
    }

    /// Move the actual renderer
    pub fn into_inner(self) -> T {
        self.actual_renderer
    }
}

macro_rules! forward_rendering_call {
    (fn $fn:ident($Ty:ty)) => {
        fn $fn(&mut self, obj: Pin<&$Ty>) {
            Self::do_rendering(&mut self.cache, &obj.cached_rendering_data, || {
                self.actual_renderer.$fn(obj);
                type Ty = $Ty;
                let width = Ty::FIELD_OFFSETS.width.apply_pin(obj).get_untracked();
                let height = Ty::FIELD_OFFSETS.height.apply_pin(obj).get_untracked();
                let x = Ty::FIELD_OFFSETS.x.apply_pin(obj).get_untracked();
                let y = Ty::FIELD_OFFSETS.y.apply_pin(obj).get_untracked();
                euclid::rect(x, y, width, height)
            })
        }
    };
}

impl<'a, T: ItemRenderer> ItemRenderer for PartialRenderer<'a, T> {
    fn filter_item(&mut self, item: Pin<ItemRef>) -> (bool, Rect) {
        let rendering_data = item.cached_rendering_data_offset();
        let item_geometry = match rendering_data.get_entry(&mut self.cache) {
            Some(CachedGraphicsData { data, dependency_tracker }) => {
                dependency_tracker
                    .get_or_insert_with(|| Box::pin(crate::properties::PropertyTracker::default()))
                    .as_ref()
                    .evaluate_if_dirty(|| *data = item.as_ref().geometry());
                *data
            }
            None => {
                let cache_entry =
                    crate::graphics::CachedGraphicsData::new(|| item.as_ref().geometry());
                let geom = cache_entry.data;
                rendering_data.cache_index.set(self.cache.insert(cache_entry));
                rendering_data.cache_generation.set(self.cache.generation());
                geom
            }
        };

        //let clip = self.get_current_clip().intersection(&self.dirty_region.to_rect());
        //let draw = clip.map_or(false, |r| r.intersects(&item_geometry));
        //FIXME: the dirty_region is in global coordinate but item_geometry and current_clip is not
        let draw = self.get_current_clip().intersects(&item_geometry);
        (draw, item_geometry)
    }

    forward_rendering_call!(fn draw_rectangle(Rectangle));
    forward_rendering_call!(fn draw_border_rectangle(BorderRectangle));
    forward_rendering_call!(fn draw_image(ImageItem));
    forward_rendering_call!(fn draw_clipped_image(ClippedImage));
    forward_rendering_call!(fn draw_text(Text));
    forward_rendering_call!(fn draw_text_input(TextInput));
    #[cfg(feature = "std")]
    forward_rendering_call!(fn draw_path(Path));
    forward_rendering_call!(fn draw_box_shadow(BoxShadow));

    fn combine_clip(&mut self, rect: Rect, radius: f32, border_width: f32) {
        self.actual_renderer.combine_clip(rect, radius, border_width)
    }

    fn get_current_clip(&self) -> Rect {
        self.actual_renderer.get_current_clip()
    }

    fn translate(&mut self, x: f32, y: f32) {
        self.actual_renderer.translate(x, y)
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        self.actual_renderer.rotate(angle_in_degrees)
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.actual_renderer.apply_opacity(opacity)
    }

    fn save_state(&mut self) {
        self.actual_renderer.save_state()
    }

    fn restore_state(&mut self) {
        self.actual_renderer.restore_state()
    }

    fn scale_factor(&self) -> f32 {
        self.actual_renderer.scale_factor()
    }

    fn draw_cached_pixmap(
        &mut self,
        item_cache: &CachedRenderingData,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        self.actual_renderer.draw_cached_pixmap(item_cache, update_fn)
    }

    fn draw_string(&mut self, string: &str, color: crate::Color) {
        self.actual_renderer.draw_string(string, color)
    }

    fn window(&self) -> crate::window::WindowRc {
        self.actual_renderer.window()
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self.actual_renderer.as_any()
    }
}
