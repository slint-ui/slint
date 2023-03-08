// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![warn(missing_docs)]
//! module for rendering the tree of items

use super::graphics::RenderingCache;
use super::items::*;
use crate::component::ComponentRc;
use crate::graphics::CachedGraphicsData;
use crate::item_tree::{
    ItemRc, ItemVisitor, ItemVisitorResult, ItemVisitorVTable, VisitChildrenResult,
};
use crate::lengths::{
    LogicalLength, LogicalPoint, LogicalPx, LogicalRect, LogicalSize, LogicalVector,
};
use crate::Coord;
use alloc::boxed::Box;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
#[cfg(feature = "std")]
use std::collections::HashMap;
use vtable::VRc;

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

/// A per-item cache.
///
/// Cache rendering information for a given item.
///
/// Use [`ItemCache::get_or_update_cache_entry`] to get or update the items, the
/// cache is automatically invalided when the property gets dirty.
/// [`ItemCache::component_destroyed`] must be called to clear the cache for that
/// component.
#[cfg(feature = "std")]
#[derive(Default)]
pub struct ItemCache<T> {
    /// The pointer is a pointer to a component
    map: RefCell<HashMap<*const vtable::Dyn, HashMap<usize, CachedGraphicsData<T>>>>,
}
#[cfg(feature = "std")]
impl<T: Clone> ItemCache<T> {
    /// Returns the cached value associated to the `item_rc` if it is still valid.
    /// Otherwise call the `update_fn` to compute that value, and track property access
    /// so it is automatically invalided when property becomes dirty.
    pub fn get_or_update_cache_entry(&self, item_rc: &ItemRc, update_fn: impl FnOnce() -> T) -> T {
        let component = &(*item_rc.component()) as *const _;
        let mut borrowed = self.map.borrow_mut();
        match borrowed.entry(component).or_default().entry(item_rc.index()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let mut tracker = entry.get_mut().dependency_tracker.take();
                drop(borrowed);
                let maybe_new_data = tracker
                    .get_or_insert_with(|| Box::pin(Default::default()))
                    .as_ref()
                    .evaluate_if_dirty(update_fn);
                let mut borrowed = self.map.borrow_mut();
                let e = borrowed.get_mut(&component).unwrap().get_mut(&item_rc.index()).unwrap();
                e.dependency_tracker = tracker;
                if let Some(new_data) = maybe_new_data {
                    e.data = new_data.clone();
                    new_data
                } else {
                    e.data.clone()
                }
            }
            std::collections::hash_map::Entry::Vacant(_) => {
                drop(borrowed);
                let new_entry = CachedGraphicsData::new(update_fn);
                let data = new_entry.data.clone();
                self.map
                    .borrow_mut()
                    .get_mut(&component)
                    .unwrap()
                    .insert(item_rc.index(), new_entry);
                data
            }
        }
    }

    /// Returns the cached value associated with the `item_rc` if it is in the cache
    /// and still valid.
    pub fn with_entry<U>(
        &self,
        item_rc: &ItemRc,
        callback: impl FnOnce(&T) -> Option<U>,
    ) -> Option<U> {
        let component = &(*item_rc.component()) as *const _;
        self.map
            .borrow()
            .get(&component)
            .and_then(|per_component_entries| per_component_entries.get(&item_rc.index()))
            .and_then(|entry| callback(&entry.data))
    }

    /// free the whole cache
    pub fn clear_all(&self) {
        self.map.borrow_mut().clear();
    }

    /// Function that must be called when a component is destroyed.
    ///
    /// Usually can be called from [`crate::window::WindowAdapterSealed::unregister_component`]
    pub fn component_destroyed(&self, component: crate::component::ComponentRef) {
        let component_ptr: *const _ =
            crate::component::ComponentRef::as_ptr(component).cast().as_ptr();
        self.map.borrow_mut().remove(&component_ptr);
    }

    /// free the cache for a given item
    pub fn release(&self, item_rc: &ItemRc) {
        let component = &(*item_rc.component()) as *const _;
        if let Some(sub) = self.map.borrow_mut().get_mut(&component) {
            sub.remove(&item_rc.index());
        }
    }
}

/// Return true if the item might be a clipping item
pub(crate) fn is_clipping_item(item: Pin<ItemRef>) -> bool {
    //(FIXME: there should be some flag in the vtable instead of down-casting)
    ItemRef::downcast_pin::<Flickable>(item).is_some()
        || ItemRef::downcast_pin::<Clip>(item).map_or(false, |clip_item| clip_item.as_ref().clip())
}

/// Renders the children of the item with the specified index into the renderer.
pub fn render_item_children(
    renderer: &mut dyn ItemRenderer,
    component: &ComponentRc,
    index: isize,
) {
    let mut actual_visitor =
        |component: &ComponentRc, index: usize, item: Pin<ItemRef>| -> VisitChildrenResult {
            renderer.save_state();

            let (do_draw, item_geometry) = renderer.filter_item(item);

            let item_origin = item_geometry.origin;
            renderer.translate(item_origin.to_vector());

            // Don't render items that are clipped, with the exception of the Clip or Flickable since
            // they themselves clip their content.
            let render_result = if do_draw
               || is_clipping_item(item)
               // HACK, the geometry of the box shadow does not include the shadow, because when the shadow is the root for repeated elements it would translate the children
               || ItemRef::downcast_pin::<BoxShadow>(item).is_some()
            {
                item.as_ref().render(
                    &mut (renderer as &mut dyn ItemRenderer),
                    &ItemRc::new(component.clone(), index),
                )
            } else {
                RenderingResult::ContinueRenderingChildren
            };

            if matches!(render_result, RenderingResult::ContinueRenderingChildren) {
                render_item_children(renderer, component, index as isize);
            }
            renderer.restore_state();
            VisitChildrenResult::CONTINUE
        };
    vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);
    VRc::borrow_pin(component).as_ref().visit_children_item(
        index,
        crate::item_tree::TraversalOrder::BackToFront,
        actual_visitor,
    );
}

/// Renders the tree of items that component holds, using the specified renderer. Rendering is done
/// relative to the specified origin.
pub fn render_component_items(
    component: &ComponentRc,
    renderer: &mut dyn ItemRenderer,
    origin: LogicalPoint,
) {
    renderer.save_state();
    renderer.translate(origin.to_vector());

    render_item_children(renderer, component, -1);

    renderer.restore_state();
}

/// Compute the bounding rect of all children. This does /not/ include item's own bounding rect. Remember to run this
/// via `evaluate_no_tracking`.
pub fn item_children_bounding_rect(
    component: &ComponentRc,
    index: isize,
    clip_rect: &LogicalRect,
) -> LogicalRect {
    let mut bounding_rect = LogicalRect::zero();

    let mut actual_visitor =
        |component: &ComponentRc, index: usize, item: Pin<ItemRef>| -> VisitChildrenResult {
            let item_geometry = item.as_ref().geometry();

            let local_clip_rect = clip_rect.translate(-item_geometry.origin.to_vector());

            if let Some(clipped_item_geometry) = item_geometry.intersection(clip_rect) {
                bounding_rect = bounding_rect.union(&clipped_item_geometry);
            }

            if !is_clipping_item(item) {
                bounding_rect = bounding_rect.union(&item_children_bounding_rect(
                    component,
                    index as isize,
                    &local_clip_rect,
                ));
            }
            VisitChildrenResult::CONTINUE
        };
    vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);
    VRc::borrow_pin(component).as_ref().visit_children_item(
        index,
        crate::item_tree::TraversalOrder::BackToFront,
        actual_visitor,
    );

    bounding_rect
}

/// Trait used to render each items.
///
/// The item needs to be rendered relative to its (x,y) position. For example,
/// draw_rectangle should draw a rectangle in `(pos.x + rect.x, pos.y + rect.y)`
#[allow(missing_docs)]
pub trait ItemRenderer {
    fn draw_rectangle(&mut self, rect: Pin<&Rectangle>, _self_rc: &ItemRc);
    fn draw_border_rectangle(&mut self, rect: Pin<&BorderRectangle>, _self_rc: &ItemRc);
    fn draw_image(&mut self, image: Pin<&ImageItem>, _self_rc: &ItemRc);
    fn draw_clipped_image(&mut self, image: Pin<&ClippedImage>, _self_rc: &ItemRc);
    fn draw_text(&mut self, text: Pin<&Text>, _self_rc: &ItemRc);
    fn draw_text_input(&mut self, text_input: Pin<&TextInput>, _self_rc: &ItemRc);
    #[cfg(feature = "std")]
    fn draw_path(&mut self, path: Pin<&Path>, _self_rc: &ItemRc);
    fn draw_box_shadow(&mut self, box_shadow: Pin<&BoxShadow>, _self_rc: &ItemRc);
    fn visit_opacity(&mut self, opacity_item: Pin<&Opacity>, _self_rc: &ItemRc) -> RenderingResult {
        self.apply_opacity(opacity_item.opacity());
        RenderingResult::ContinueRenderingChildren
    }
    fn visit_layer(&mut self, _layer_item: Pin<&Layer>, _self_rc: &ItemRc) -> RenderingResult {
        // Not supported
        RenderingResult::ContinueRenderingChildren
    }

    // Apply the bounds of the Clip element, if enabled. The default implementation calls
    // combine_clip, but the render may choose an alternate way of implementing the clip.
    // For example the GL backend uses a layered rendering approach.
    fn visit_clip(&mut self, clip_item: Pin<&Clip>, _self_rc: &ItemRc) -> RenderingResult {
        if clip_item.clip() {
            let geometry = clip_item.geometry();

            let clip_region_valid = self.combine_clip(
                LogicalRect::new(LogicalPoint::default(), geometry.size),
                clip_item.border_radius(),
                clip_item.border_width(),
            );

            // If clipping is enabled but the clip element is outside the visible range, then we don't
            // need to bother doing anything, not even rendering the children.
            if !clip_region_valid {
                return RenderingResult::ContinueRenderingWithoutChildren;
            }
        }
        RenderingResult::ContinueRenderingChildren
    }

    /// Clip the further call until restore_state.
    /// radius/border_width can be used for border rectangle clip.
    /// (FIXME: consider removing radius/border_width and have another  function that take a path instead)
    /// Returns a boolean indicating the state of the new clip region: true if the clip region covers
    /// an area; false if the clip region is empty.
    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        radius: LogicalLength,
        border_width: LogicalLength,
    ) -> bool;
    /// Get the current clip bounding box in the current transformed coordinate.
    fn get_current_clip(&self) -> LogicalRect;

    fn translate(&mut self, distance: LogicalVector);
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
        item_cache: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    );

    /// Draw the given string with the specified color at current (0, 0) with the default font. Mainly
    /// used by the performance counter overlay.
    fn draw_string(&mut self, string: &str, color: crate::Color);

    /// This is called before it is being rendered (before the draw_* function).
    /// Returns
    ///  - if the item needs to be drawn (false means it is clipped or doesn't need to be drawn)
    ///  - the geometry of the item
    fn filter_item(&mut self, item: Pin<ItemRef>) -> (bool, LogicalRect) {
        let item_geometry = item.as_ref().geometry();
        (self.get_current_clip().intersects(&item_geometry), item_geometry)
    }

    fn window(&self) -> &crate::api::Window;

    /// Return the internal renderer
    fn as_any(&mut self) -> Option<&mut dyn core::any::Any>;

    /// Returns any rendering metrics collecting since the creation of the renderer (typically
    /// per frame)
    #[cfg(feature = "std")]
    fn metrics(&self) -> crate::graphics::rendering_metrics_collector::RenderingMetrics {
        Default::default()
    }
}

/// The cache that needs to be held by the Window for the partial rendering
pub type PartialRenderingCache = RenderingCache<LogicalRect>;

/// FIXME: Should actually be a region and not just a rectangle
pub type DirtyRegion = euclid::Box2D<Coord, LogicalPx>;

/// Put this structure in the renderer to help with partial rendering
pub struct PartialRenderer<'a, T> {
    cache: &'a RefCell<PartialRenderingCache>,
    /// The region of the screen which is considered dirty and that should be repainted
    pub dirty_region: DirtyRegion,
    /// The actual renderer which the drawing call will be forwarded to
    pub actual_renderer: T,
}

impl<'a, T> PartialRenderer<'a, T> {
    /// Create a new PartialRenderer
    pub fn new(
        cache: &'a RefCell<PartialRenderingCache>,
        initial_dirty_region: DirtyRegion,
        actual_renderer: T,
    ) -> Self {
        Self { cache, dirty_region: initial_dirty_region, actual_renderer }
    }

    /// Visit the tree of item and compute what are the dirty regions
    pub fn compute_dirty_regions(&mut self, component: &ComponentRc, origin: LogicalPoint) {
        #[derive(Clone, Copy)]
        struct ComputeDirtyRegionState {
            offset: euclid::Vector2D<Coord, LogicalPx>,
            old_offset: euclid::Vector2D<Coord, LogicalPx>,
            clipped: LogicalRect,
            must_refresh_children: bool,
        }

        crate::item_tree::visit_items(
            component,
            crate::item_tree::TraversalOrder::BackToFront,
            |_, item, _, state| {
                let mut new_state = *state;
                let mut borrowed = self.cache.borrow_mut();

                match item.cached_rendering_data_offset().get_entry(&mut borrowed) {
                    Some(CachedGraphicsData {
                        data: cached_geom,
                        dependency_tracker: Some(tr),
                    }) => {
                        if tr.is_dirty() {
                            let old_geom = *cached_geom;
                            drop(borrowed);
                            let geom = crate::properties::evaluate_no_tracking(|| {
                                item.as_ref().geometry()
                            });

                            self.mark_dirty_rect(old_geom, state.old_offset, &state.clipped);
                            self.mark_dirty_rect(geom, state.offset, &state.clipped);

                            new_state.offset += geom.origin.to_vector();
                            new_state.old_offset += old_geom.origin.to_vector();
                            if ItemRef::downcast_pin::<Clip>(item).is_some()
                                || ItemRef::downcast_pin::<Opacity>(item).is_some()
                            {
                                // When the opacity or the clip change, this will impact all the children, including
                                // the ones outside the element, regardless if they are themselves dirty or not.
                                new_state.must_refresh_children = true;
                            }

                            ItemVisitorResult::Continue(new_state)
                        } else {
                            tr.as_ref().register_as_dependency_to_current_binding();

                            if state.must_refresh_children
                                || new_state.offset != new_state.old_offset
                            {
                                self.mark_dirty_rect(
                                    *cached_geom,
                                    state.old_offset,
                                    &state.clipped,
                                );
                                self.mark_dirty_rect(*cached_geom, state.offset, &state.clipped);
                            }

                            new_state.offset += cached_geom.origin.to_vector();
                            new_state.old_offset += cached_geom.origin.to_vector();
                            if crate::properties::evaluate_no_tracking(|| is_clipping_item(item)) {
                                new_state.clipped = new_state
                                    .clipped
                                    .intersection(
                                        &cached_geom
                                            .translate(state.offset)
                                            .union(&cached_geom.translate(state.old_offset)),
                                    )
                                    .unwrap_or_default();
                            }
                            ItemVisitorResult::Continue(new_state)
                        }
                    }
                    _ => {
                        drop(borrowed);
                        let geom = crate::properties::evaluate_no_tracking(|| {
                            let geom = item.as_ref().geometry();
                            new_state.offset += geom.origin.to_vector();
                            new_state.old_offset += geom.origin.to_vector();
                            if is_clipping_item(item) {
                                new_state.clipped = new_state
                                    .clipped
                                    .intersection(&geom.translate(state.offset))
                                    .unwrap_or_default();
                            }
                            geom
                        });
                        self.mark_dirty_rect(geom, state.offset, &state.clipped);
                        ItemVisitorResult::Continue(new_state)
                    }
                }
            },
            ComputeDirtyRegionState {
                offset: origin.to_vector(),
                old_offset: origin.to_vector(),
                clipped: euclid::rect(0 as Coord, 0 as Coord, Coord::MAX, Coord::MAX),
                must_refresh_children: false,
            },
        );
    }

    fn mark_dirty_rect(
        &mut self,
        rect: LogicalRect,
        offset: euclid::Vector2D<Coord, LogicalPx>,
        clip_rect: &LogicalRect,
    ) {
        if !rect.is_empty() {
            if let Some(rect) = rect.translate(offset).intersection(clip_rect) {
                self.dirty_region = self.dirty_region.union(&rect.to_box2d());
            }
        }
    }

    fn do_rendering(
        cache: &RefCell<PartialRenderingCache>,
        rendering_data: &CachedRenderingData,
        render_fn: impl FnOnce() -> LogicalRect,
    ) {
        if let Some(entry) = rendering_data.get_entry(&mut cache.borrow_mut()) {
            entry
                .dependency_tracker
                .get_or_insert_with(|| Box::pin(crate::properties::PropertyTracker::default()))
                .as_ref()
                .evaluate(render_fn);
        } else {
            let cache_entry = crate::graphics::CachedGraphicsData::new(render_fn);
            let mut cache = cache.borrow_mut();
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
    (fn $fn:ident($Ty:ty) $(-> $Ret:ty)?) => {
        fn $fn(&mut self, obj: Pin<&$Ty>, item_rc: &ItemRc) $(-> $Ret)? {
            let mut ret = None;
            Self::do_rendering(&self.cache, &obj.cached_rendering_data, || {
                ret = Some(self.actual_renderer.$fn(obj, item_rc));
                type Ty = $Ty;
                let width = Ty::FIELD_OFFSETS.width.apply_pin(obj).get_untracked();
                let height = Ty::FIELD_OFFSETS.height.apply_pin(obj).get_untracked();
                let x = Ty::FIELD_OFFSETS.x.apply_pin(obj).get_untracked();
                let y = Ty::FIELD_OFFSETS.y.apply_pin(obj).get_untracked();
                LogicalRect::new(
                    LogicalPoint::from_lengths(x, y),
                    LogicalSize::from_lengths(width, height),
                )
            });
            ret.unwrap_or_default()
        }
    };
}

impl<'a, T: ItemRenderer> ItemRenderer for PartialRenderer<'a, T> {
    fn filter_item(&mut self, item: Pin<ItemRef>) -> (bool, LogicalRect) {
        let eval = || {
            if let Some(clip) = ItemRef::downcast_pin::<Clip>(item) {
                // Make sure we register a dependency on the clip
                clip.clip();
            }
            item.as_ref().geometry()
        };

        let rendering_data = item.cached_rendering_data_offset();
        let mut cache = self.cache.borrow_mut();
        let item_geometry = match rendering_data.get_entry(&mut cache) {
            Some(CachedGraphicsData { data, dependency_tracker }) => {
                dependency_tracker
                    .get_or_insert_with(|| Box::pin(crate::properties::PropertyTracker::default()))
                    .as_ref()
                    .evaluate_if_dirty(|| *data = eval());
                *data
            }
            None => {
                let cache_entry = crate::graphics::CachedGraphicsData::new(eval);
                let geom = cache_entry.data;
                rendering_data.cache_index.set(cache.insert(cache_entry));
                rendering_data.cache_generation.set(cache.generation());
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

    forward_rendering_call!(fn visit_clip(Clip) -> RenderingResult);
    forward_rendering_call!(fn visit_opacity(Opacity) -> RenderingResult);

    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        radius: LogicalLength,
        border_width: LogicalLength,
    ) -> bool {
        self.actual_renderer.combine_clip(rect, radius, border_width)
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.actual_renderer.get_current_clip()
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.actual_renderer.translate(distance)
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
        item_rc: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        self.actual_renderer.draw_cached_pixmap(item_rc, update_fn)
    }

    fn draw_string(&mut self, string: &str, color: crate::Color) {
        self.actual_renderer.draw_string(string, color)
    }

    fn window(&self) -> &crate::api::Window {
        self.actual_renderer.window()
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        self.actual_renderer.as_any()
    }
}
