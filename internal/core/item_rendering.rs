// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
//! module for rendering the tree of items

use super::graphics::RenderingCache;
use super::items::*;
use crate::graphics::{CachedGraphicsData, FontRequest, Image, IntRect};
use crate::item_tree::ItemTreeRc;
use crate::item_tree::{ItemVisitor, ItemVisitorResult, ItemVisitorVTable, VisitChildrenResult};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalPx, LogicalRect, LogicalSize,
    LogicalVector,
};
use crate::properties::PropertyTracker;
use crate::window::WindowInner;
use crate::{Brush, Coord, SharedString};
#[cfg(not(feature = "std"))]
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
pub struct ItemCache<T> {
    /// The pointer is a pointer to a component
    map: RefCell<HashMap<*const vtable::Dyn, HashMap<u32, CachedGraphicsData<T>>>>,
    /// Track if the window scale factor changes; used to clear the cache if necessary.
    window_scale_factor_tracker: Pin<Box<PropertyTracker>>,
}

#[cfg(feature = "std")]
impl<T> Default for ItemCache<T> {
    fn default() -> Self {
        Self { map: Default::default(), window_scale_factor_tracker: Box::pin(Default::default()) }
    }
}

#[cfg(feature = "std")]
impl<T: Clone> ItemCache<T> {
    /// Returns the cached value associated to the `item_rc` if it is still valid.
    /// Otherwise call the `update_fn` to compute that value, and track property access
    /// so it is automatically invalided when property becomes dirty.
    pub fn get_or_update_cache_entry(&self, item_rc: &ItemRc, update_fn: impl FnOnce() -> T) -> T {
        let component = &(**item_rc.item_tree()) as *const _;
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
        let component = &(**item_rc.item_tree()) as *const _;
        self.map
            .borrow()
            .get(&component)
            .and_then(|per_component_entries| per_component_entries.get(&item_rc.index()))
            .and_then(|entry| callback(&entry.data))
    }

    /// Clears the cache if the window's scale factor has changed since the last call.
    pub fn clear_cache_if_scale_factor_changed(&self, window: &crate::api::Window) {
        if self.window_scale_factor_tracker.is_dirty() {
            self.window_scale_factor_tracker
                .as_ref()
                .evaluate_as_dependency_root(|| window.scale_factor());
            self.clear_all();
        }
    }

    /// free the whole cache
    pub fn clear_all(&self) {
        self.map.borrow_mut().clear();
    }

    /// Function that must be called when a component is destroyed.
    ///
    /// Usually can be called from [`crate::window::WindowAdapterInternal::unregister_item_tree`]
    pub fn component_destroyed(&self, component: crate::item_tree::ItemTreeRef) {
        let component_ptr: *const _ =
            crate::item_tree::ItemTreeRef::as_ptr(component).cast().as_ptr();
        self.map.borrow_mut().remove(&component_ptr);
    }

    /// free the cache for a given item
    pub fn release(&self, item_rc: &ItemRc) {
        let component = &(**item_rc.item_tree()) as *const _;
        if let Some(sub) = self.map.borrow_mut().get_mut(&component) {
            sub.remove(&item_rc.index());
        }
    }

    /// Returns true if there are no entries in the cache; false otherwise.
    pub fn is_empty(&self) -> bool {
        self.map.borrow().is_empty()
    }
}

/// Return true if the item might be a clipping item
pub fn is_clipping_item(item: Pin<ItemRef>) -> bool {
    //(FIXME: there should be some flag in the vtable instead of down-casting)
    ItemRef::downcast_pin::<Flickable>(item).is_some()
        || ItemRef::downcast_pin::<Clip>(item).map_or(false, |clip_item| clip_item.as_ref().clip())
}

/// Renders the children of the item with the specified index into the renderer.
pub fn render_item_children(renderer: &mut dyn ItemRenderer, component: &ItemTreeRc, index: isize) {
    let mut actual_visitor =
        |component: &ItemTreeRc, index: u32, item: Pin<ItemRef>| -> VisitChildrenResult {
            renderer.save_state();
            let item_rc = ItemRc::new(component.clone(), index);

            let (do_draw, item_geometry) = renderer.filter_item(&item_rc);

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
                    &item_rc,
                    item_geometry.size,
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
    component: &ItemTreeRc,
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
    component: &ItemTreeRc,
    index: isize,
    clip_rect: &LogicalRect,
) -> LogicalRect {
    let mut bounding_rect = LogicalRect::zero();

    let mut actual_visitor =
        |component: &ItemTreeRc, index: u32, item: Pin<ItemRef>| -> VisitChildrenResult {
            let item_geometry = ItemTreeRc::borrow_pin(component).as_ref().item_geometry(index);

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

/// Trait for an item that represent a Rectangle to the Renderer
#[allow(missing_docs)]
pub trait RenderBorderRectangle {
    fn background(self: Pin<&Self>) -> Brush;
    fn border_width(self: Pin<&Self>) -> LogicalLength;
    fn border_radius(self: Pin<&Self>) -> LogicalBorderRadius;
    fn border_color(self: Pin<&Self>) -> Brush;
}

/// Trait for an item that represents an Image towards the renderer
#[allow(missing_docs)]
pub trait RenderImage {
    fn target_size(self: Pin<&Self>) -> LogicalSize;
    fn source(self: Pin<&Self>) -> Image;
    fn source_clip(self: Pin<&Self>) -> Option<IntRect>;
    fn image_fit(self: Pin<&Self>) -> ImageFit;
    fn rendering(self: Pin<&Self>) -> ImageRendering;
    fn colorize(self: Pin<&Self>) -> Brush;
    fn alignment(self: Pin<&Self>) -> (ImageHorizontalAlignment, ImageVerticalAlignment);
    fn tiling(self: Pin<&Self>) -> (ImageTiling, ImageTiling);
}

/// Trait for an item that represents an Text towards the renderer
#[allow(missing_docs)]
pub trait RenderText {
    fn target_size(self: Pin<&Self>) -> LogicalSize;
    fn text(self: Pin<&Self>) -> SharedString;
    fn font_request(self: Pin<&Self>, window: &WindowInner) -> FontRequest;
    fn color(self: Pin<&Self>) -> Brush;
    fn alignment(self: Pin<&Self>) -> (TextHorizontalAlignment, TextVerticalAlignment);
    fn wrap(self: Pin<&Self>) -> TextWrap;
    fn overflow(self: Pin<&Self>) -> TextOverflow;
    fn letter_spacing(self: Pin<&Self>) -> LogicalLength;
    fn stroke(self: Pin<&Self>) -> (Brush, LogicalLength, TextStrokeStyle);
}

/// Trait used to render each items.
///
/// The item needs to be rendered relative to its (x,y) position. For example,
/// draw_rectangle should draw a rectangle in `(pos.x + rect.x, pos.y + rect.y)`
#[allow(missing_docs)]
pub trait ItemRenderer {
    fn draw_rectangle(&mut self, rect: Pin<&Rectangle>, _self_rc: &ItemRc, _size: LogicalSize);
    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn RenderBorderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    );
    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    );
    fn draw_text(
        &mut self,
        text: Pin<&dyn RenderText>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    );
    fn draw_text_input(
        &mut self,
        text_input: Pin<&TextInput>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    );
    #[cfg(feature = "std")]
    fn draw_path(&mut self, path: Pin<&Path>, _self_rc: &ItemRc, _size: LogicalSize);
    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&BoxShadow>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    );
    fn visit_opacity(
        &mut self,
        opacity_item: Pin<&Opacity>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        self.apply_opacity(opacity_item.opacity());
        RenderingResult::ContinueRenderingChildren
    }
    fn visit_layer(
        &mut self,
        _layer_item: Pin<&Layer>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        // Not supported
        RenderingResult::ContinueRenderingChildren
    }

    // Apply the bounds of the Clip element, if enabled. The default implementation calls
    // combine_clip, but the render may choose an alternate way of implementing the clip.
    // For example the GL backend uses a layered rendering approach.
    fn visit_clip(
        &mut self,
        clip_item: Pin<&Clip>,
        item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        if clip_item.clip() {
            let geometry = item_rc.geometry();

            let clip_region_valid = self.combine_clip(
                LogicalRect::new(LogicalPoint::default(), geometry.size),
                clip_item.logical_border_radius(),
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
        radius: LogicalBorderRadius,
        border_width: LogicalLength,
    ) -> bool;
    /// Get the current clip bounding box in the current transformed coordinate.
    fn get_current_clip(&self) -> LogicalRect;

    fn translate(&mut self, distance: LogicalVector);
    fn translation(&self) -> LogicalVector {
        unimplemented!()
    }
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

    fn draw_image_direct(&mut self, image: crate::graphics::Image);

    /// Fills a rectangle at (0,0) with the given size. This is used for example by the Skia renderer to
    /// handle window backgrounds with a brush (gradient).
    fn draw_rect(&mut self, _size: LogicalSize, _brush: Brush) {
        unimplemented!()
    }

    /// This is called before it is being rendered (before the draw_* function).
    /// Returns
    ///  - if the item needs to be drawn (false means it is clipped or doesn't need to be drawn)
    ///  - the geometry of the item
    fn filter_item(&mut self, item: &ItemRc) -> (bool, LogicalRect) {
        let item_geometry = item.geometry();
        (self.get_current_clip().intersects(&item_geometry), item_geometry)
    }

    fn window(&self) -> &crate::window::WindowInner;

    /// Return the internal renderer
    fn as_any(&mut self) -> Option<&mut dyn core::any::Any>;

    /// Returns any rendering metrics collecting since the creation of the renderer (typically
    /// per frame)
    fn metrics(&self) -> crate::graphics::rendering_metrics_collector::RenderingMetrics {
        Default::default()
    }
}

/// The cache that needs to be held by the Window for the partial rendering
pub type PartialRenderingCache = RenderingCache<LogicalRect>;

/// A region composed of a few rectangles that need to be redrawn.
#[derive(Default, Clone, Debug)]
pub struct DirtyRegion {
    rectangles: [euclid::Box2D<Coord, LogicalPx>; Self::MAX_COUNT],
    count: usize,
}

impl DirtyRegion {
    /// The maximum number of rectangles that can be stored in a DirtyRegion
    pub(crate) const MAX_COUNT: usize = 3;

    /// An iterator over the part of the region (they can overlap)
    pub fn iter(&self) -> impl Iterator<Item = euclid::Box2D<Coord, LogicalPx>> + '_ {
        (0..self.count).map(|x| self.rectangles[x])
    }

    /// Add a rectangle to the region.
    ///
    /// Note that if the region becomes too complex, it might be simplified by being bigger than the actual union.
    pub fn add_rect(&mut self, rect: LogicalRect) {
        self.add_box(rect.to_box2d());
    }

    /// Add a box to the region
    ///
    /// Note that if the region becomes too complex, it might be simplified by being bigger than the actual union.
    pub fn add_box(&mut self, b: euclid::Box2D<Coord, LogicalPx>) {
        if b.is_empty() {
            return;
        }
        let mut i = 0;
        while i < self.count {
            let r = &self.rectangles[i];
            if r.contains_box(&b) {
                // the rectangle is already in the union
                return;
            } else if b.contains_box(r) {
                self.rectangles.swap(i, self.count - 1);
                self.count -= 1;
                continue;
            }
            i += 1;
        }

        if self.count < Self::MAX_COUNT {
            self.rectangles[self.count] = b;
            self.count += 1;
        } else {
            let best_merge = (0..self.count)
                .map(|i| (i, self.rectangles[i].union(&b).area() - self.rectangles[i].area()))
                .min_by(|a, b| PartialOrd::partial_cmp(&a.1, &b.1).unwrap())
                .expect("There should always be rectangles")
                .0;
            self.rectangles[best_merge] = self.rectangles[best_merge].union(&b);
        }
    }

    /// Make an union of two regions.
    ///
    /// Note that if the region becomes too complex, it might be simplified by being bigger than the actual union
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        let mut s = self.clone();
        for o in other.iter() {
            s.add_box(o)
        }
        s
    }

    /// Bounding rectangle of the region.
    #[must_use]
    pub fn bounding_rect(&self) -> LogicalRect {
        if self.count == 0 {
            return Default::default();
        }
        let mut r = self.rectangles[0];
        for i in 1..self.count {
            r = r.union(&self.rectangles[i]);
        }
        r.to_rect()
    }

    /// Intersection of a region and a rectangle.
    #[must_use]
    pub fn intersection(&self, other: LogicalRect) -> DirtyRegion {
        let mut ret = self.clone();
        let other = other.to_box2d();
        let mut i = 0;
        while i < ret.count {
            if let Some(x) = ret.rectangles[i].intersection(&other) {
                ret.rectangles[i] = x;
            } else {
                ret.rectangles.swap(i, ret.count);
                ret.count -= 1;
                continue;
            }
            i += 1;
        }
        ret
    }

    fn draw_intersects(&self, clipped_geom: LogicalRect) -> bool {
        let b = clipped_geom.to_box2d();
        self.iter().any(|r| r.intersects(&b))
    }
}

impl From<LogicalRect> for DirtyRegion {
    fn from(value: LogicalRect) -> Self {
        let mut s = Self::default();
        s.add_rect(value);
        s
    }
}

/// This enum describes which parts of the buffer passed to the [`SoftwareRenderer`](crate::software_renderer::SoftwareRenderer) may be re-used to speed up painting.
// FIXME: #[non_exhaustive] #3023
#[derive(PartialEq, Eq, Debug, Clone, Default, Copy)]
pub enum RepaintBufferType {
    #[default]
    /// The full window is always redrawn. No attempt at partial rendering will be made.
    NewBuffer,
    /// Only redraw the parts that have changed since the previous call to render().
    ///
    /// This variant assumes that the same buffer is passed on every call to render() and
    /// that it still contains the previously rendered frame.
    ReusedBuffer,

    /// Redraw the part that have changed since the last two frames were drawn.
    ///
    /// This is used when using double buffering and swapping of the buffers.
    SwappedBuffers,
}

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
    pub fn compute_dirty_regions(
        &mut self,
        component: &ItemTreeRc,
        origin: LogicalPoint,
        size: LogicalSize,
    ) {
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
            |component, item, index, state| {
                let mut new_state = *state;
                let mut borrowed = self.cache.borrow_mut();
                let item_rc = ItemRc::new(component.clone(), index);

                match item.cached_rendering_data_offset().get_entry(&mut borrowed) {
                    Some(CachedGraphicsData {
                        data: cached_geom,
                        dependency_tracker: Some(tr),
                    }) => {
                        if tr.is_dirty() {
                            let old_geom = *cached_geom;
                            drop(borrowed);
                            let geom =
                                crate::properties::evaluate_no_tracking(|| item_rc.geometry());

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
                            let geom = item_rc.geometry();
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
                clipped: LogicalRect::from_size(size),
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
                self.dirty_region.add_rect(rect);
            }
        }
    }

    fn do_rendering(
        cache: &RefCell<PartialRenderingCache>,
        rendering_data: &CachedRenderingData,
        render_fn: impl FnOnce() -> LogicalRect,
    ) {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = rendering_data.get_entry(&mut cache) {
            entry
                .dependency_tracker
                .get_or_insert_with(|| Box::pin(PropertyTracker::default()))
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
    (fn $fn:ident($Ty:ty) $(-> $Ret:ty)?) => {
        fn $fn(&mut self, obj: Pin<&$Ty>, item_rc: &ItemRc, size: LogicalSize) $(-> $Ret)? {
            let mut ret = None;
            Self::do_rendering(&self.cache, &obj.cached_rendering_data, || {
                ret = Some(self.actual_renderer.$fn(obj, item_rc, size));
                item_rc.geometry()
            });
            ret.unwrap_or_default()
        }
    };
}

macro_rules! forward_rendering_call2 {
    (fn $fn:ident($Ty:ty) $(-> $Ret:ty)?) => {
        fn $fn(&mut self, obj: Pin<&$Ty>, item_rc: &ItemRc, size: LogicalSize, cache: &CachedRenderingData) $(-> $Ret)? {
            let mut ret = None;
            Self::do_rendering(&self.cache, &cache, || {
                ret = Some(self.actual_renderer.$fn(obj, item_rc, size, &cache));
                item_rc.geometry()
            });
            ret.unwrap_or_default()
        }
    };
}

impl<'a, T: ItemRenderer> ItemRenderer for PartialRenderer<'a, T> {
    fn filter_item(&mut self, item_rc: &ItemRc) -> (bool, LogicalRect) {
        let item = item_rc.borrow();
        let eval = || {
            if let Some(clip) = ItemRef::downcast_pin::<Clip>(item) {
                // Make sure we register a dependency on the clip
                clip.clip();
            }
            item_rc.geometry()
        };

        let rendering_data = item.cached_rendering_data_offset();
        let mut cache = self.cache.borrow_mut();
        let item_geometry = match rendering_data.get_entry(&mut cache) {
            Some(CachedGraphicsData { data, dependency_tracker }) => {
                dependency_tracker
                    .get_or_insert_with(|| Box::pin(PropertyTracker::default()))
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

        let clipped_geom = self.get_current_clip().intersection(&item_geometry);
        let draw = clipped_geom.map_or(false, |clipped_geom| {
            let clipped_geom = clipped_geom.translate(self.translation());
            self.dirty_region.draw_intersects(clipped_geom)
        });

        (draw, item_geometry)
    }

    forward_rendering_call!(fn draw_rectangle(Rectangle));
    forward_rendering_call2!(fn draw_border_rectangle(dyn RenderBorderRectangle));
    forward_rendering_call2!(fn draw_image(dyn RenderImage));
    forward_rendering_call2!(fn draw_text(dyn RenderText));
    forward_rendering_call!(fn draw_text_input(TextInput));
    #[cfg(feature = "std")]
    forward_rendering_call!(fn draw_path(Path));
    forward_rendering_call!(fn draw_box_shadow(BoxShadow));

    forward_rendering_call!(fn visit_clip(Clip) -> RenderingResult);
    forward_rendering_call!(fn visit_opacity(Opacity) -> RenderingResult);

    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        radius: LogicalBorderRadius,
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
    fn translation(&self) -> LogicalVector {
        self.actual_renderer.translation()
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

    fn draw_image_direct(&mut self, image: crate::graphics::image::Image) {
        self.actual_renderer.draw_image_direct(image)
    }

    fn draw_rect(&mut self, size: LogicalSize, brush: Brush) {
        self.actual_renderer.draw_rect(size, brush);
    }

    fn window(&self) -> &crate::window::WindowInner {
        self.actual_renderer.window()
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        self.actual_renderer.as_any()
    }
}

/// This struct holds the state of the partial renderer between different frames, in particular the cache of the bounding rect
/// of each item. This permits a more fine-grained computation of the region that needs to be repainted.
#[derive(Default)]
pub struct PartialRenderingState {
    partial_cache: RefCell<PartialRenderingCache>,
    /// This is the area which we are going to redraw in the next frame, no matter if the items are dirty or not
    force_dirty: RefCell<DirtyRegion>,
    repaint_buffer_type: Cell<RepaintBufferType>,
    /// This is the area which was dirty on the previous frame.
    /// Only used if repaint_buffer_type == RepaintBufferType::SwappedBuffers
    prev_frame_dirty: Cell<DirtyRegion>,
    /// Force a redraw in the next frame, no matter what's dirty. Use only as a last resort.
    force_screen_refresh: Cell<bool>,
}

impl PartialRenderingState {
    /// Sets the repaint type of the back buffer used for the next rendering. This helps to compute the partial
    /// rendering region correctly, for example when using swapped buffers, the region will include the dirty region
    /// of the previous frame.
    pub fn set_repaint_buffer_type(&self, repaint_buffer_type: RepaintBufferType) {
        if self.repaint_buffer_type.replace(repaint_buffer_type) != repaint_buffer_type {
            self.partial_cache.borrow_mut().clear();
        }
    }

    /// Returns the current repaint buffer type.
    pub fn repaint_buffer_type(&self) -> RepaintBufferType {
        self.repaint_buffer_type.get()
    }

    /// Creates a partial renderer that's initialized with the partial rendering caches maintained in this state structure.
    /// Call [`Self::apply_dirty_region`] after this function to compute the correct partial rendering region.
    pub fn create_partial_renderer<'a, T: ItemRenderer>(
        &'a self,
        renderer: T,
    ) -> PartialRenderer<'a, T> {
        PartialRenderer::new(&self.partial_cache, self.force_dirty.take(), renderer)
    }

    /// Compute the correct partial rendering region based on the components to be drawn, the bounding rectangles of
    /// changes items within, and the current repaint buffer type.
    pub fn apply_dirty_region<T: ItemRenderer>(
        &self,
        partial_renderer: &mut PartialRenderer<'_, T>,
        components: &[(&ItemTreeRc, LogicalPoint)],
        logical_window_size: LogicalSize,
    ) {
        for (component, origin) in components {
            partial_renderer.compute_dirty_regions(component, *origin, logical_window_size);
        }

        let screen_region = LogicalRect::from_size(logical_window_size);

        if self.force_screen_refresh.take() {
            partial_renderer.dirty_region = screen_region.into();
        }

        partial_renderer.dirty_region = match self.repaint_buffer_type.get() {
            RepaintBufferType::NewBuffer => screen_region.into(),
            RepaintBufferType::ReusedBuffer => partial_renderer.dirty_region.clone(),
            RepaintBufferType::SwappedBuffers => partial_renderer
                .dirty_region
                .union(&self.prev_frame_dirty.replace(partial_renderer.dirty_region.clone())),
        }
        .intersection(screen_region);
    }

    /// Add the specified region to the list of regions to include in the next rendering.
    pub fn mark_dirty_region(&self, region: DirtyRegion) {
        self.force_dirty.replace_with(|r| r.union(&region));
    }

    /// Call this from your renderer's `free_graphics_resources` function to ensure that the cached item geometries
    /// are cleared for the destroyed items in the item tree.
    pub fn free_graphics_resources(&self, items: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>) {
        for item in items {
            item.cached_rendering_data_offset().release(&mut self.partial_cache.borrow_mut());
        }

        // We don't have a way to determine the screen region of the delete items, what's in the cache is relative. So
        // as a last resort, refresh everything.
        self.force_screen_refresh.set(true)
    }

    /// Clears the partial rendering cache. Use this for example when the entire undering window surface changes.
    pub fn clear_cache(&self) {
        self.partial_cache.borrow_mut().clear();
    }

    /// Force re-rendering of the entire window region the next time a partial renderer is created.
    pub fn force_screen_refresh(&self) {
        self.force_screen_refresh.set(true);
    }
}
