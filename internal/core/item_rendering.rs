// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![warn(missing_docs)]
//! module for rendering the tree of items

use super::graphics::RenderingCache;
use super::items::*;
use crate::graphics::{CachedGraphicsData, Image, IntRect};
use crate::item_tree::{self, ItemTreeRc};
use crate::item_tree::{ItemVisitor, ItemVisitorVTable, VisitChildrenResult};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalPx, LogicalRect, LogicalSize,
    LogicalVector,
};
use crate::properties::PropertyTracker;
use crate::{Brush, Coord};
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
}

/// Return true if the item might be a clipping item
pub(crate) fn is_clipping_item(item: Pin<ItemRef>) -> bool {
    //(FIXME: there should be some flag in the vtable instead of down-casting)
    ItemRef::downcast_pin::<Flickable>(item).is_some()
        || ItemRef::downcast_pin::<Clip>(item).map_or(false, |clip_item| clip_item.as_ref().clip())
}

fn is_opaque_item(item: Pin<ItemRef>) -> bool {
    //(FIXME: should be a function in the item vtable)
    ItemRef::downcast_pin::<Rectangle>(item).map_or(false, |r| r.as_ref().background().is_opaque())
        || ItemRef::downcast_pin::<BasicBorderRectangle>(item)
            .map_or(false, |r| r.as_ref().background().is_opaque())
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
    fn draw_text(&mut self, text: Pin<&Text>, _self_rc: &ItemRc, _size: LogicalSize);
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
pub type PartialRenderingCache = RenderingCache<PartialRenderingCacheData>;
#[derive(Clone, Copy)]
pub struct PartialRenderingCacheData {
    geometry: LogicalRect,
    z_index_for_opacity: usize,
}

/// FIXME: Should actually be a region and not just a rectangle
pub type DirtyRegion = euclid::Box2D<Coord, LogicalPx>;

/// Put this structure in the renderer to help with partial rendering
pub struct PartialRenderer<'a, T> {
    cache: &'a RefCell<PartialRenderingCache>,
    /// The region of the screen which is considered dirty and that should be repainted
    pub space_partition: space_partition::SpacePartition,
    /// Basicaly increment for each item. Note that this is backwards as it increase back to front
    z_index_for_opacity: usize,
    /// The actual renderer which the drawing call will be forwarded to
    pub actual_renderer: T,
}

#[derive(Clone, Copy)]
struct ComputeDirtyRegionState {
    offset: euclid::Vector2D<Coord, LogicalPx>,
    old_offset: euclid::Vector2D<Coord, LogicalPx>,
    clipped: euclid::Box2D<Coord, LogicalPx>,
    must_refresh_children: bool,
    has_opacity: bool,
}

impl<'a, T> PartialRenderer<'a, T> {
    /// Create a new PartialRenderer
    pub fn new(
        cache: &'a RefCell<PartialRenderingCache>,
        initial_dirty_region: DirtyRegion,
        actual_renderer: T,
        size: LogicalSize,
    ) -> Self {
        let mut space_partition = space_partition::SpacePartition::new(size);
        space_partition.add_region(initial_dirty_region, true, None);
        Self { cache, actual_renderer, z_index_for_opacity: 0, space_partition }
    }

    /// Visit item and its children recursively back to front
    fn compute_dirty_regions_visit_item(
        &mut self,
        item_rc: ItemRc,
        state: &ComputeDirtyRegionState,
    ) {
        let mut new_state = *state;
        let mut borrowed = self.cache.borrow_mut();
        let item = item_rc.borrow();

        let actual_rect = |rect: LogicalRect, offset| {
            rect.translate(offset).to_box2d().intersection(&state.clipped)
        };

        let dirty_rects = match item.cached_rendering_data_offset().get_entry(&mut borrowed) {
            Some(CachedGraphicsData {
                data: PartialRenderingCacheData { geometry: cached_geom, .. },
                dependency_tracker: Some(tr),
            }) => {
                if tr.is_dirty() {
                    let old_geom = *cached_geom;
                    drop(borrowed);
                    let geom = crate::properties::evaluate_no_tracking(|| item_rc.geometry());

                    let dirty_rects = (
                        actual_rect(geom, state.offset),
                        actual_rect(old_geom, state.old_offset),
                        true,
                    );

                    new_state.offset += geom.origin.to_vector();
                    new_state.old_offset += old_geom.origin.to_vector();
                    if ItemRef::downcast_pin::<Clip>(item).is_some()
                        || ItemRef::downcast_pin::<Opacity>(item).is_some()
                    {
                        // When the opacity or the clip change, this will impact all the children, including
                        // the ones outside the element, regardless if they are themselves dirty or not.
                        new_state.must_refresh_children = true;
                    }

                    dirty_rects
                } else {
                    tr.as_ref().register_as_dependency_to_current_binding();

                    let dirty_rects = if state.must_refresh_children
                        || new_state.offset != new_state.old_offset
                    {
                        (
                            actual_rect(*cached_geom, state.offset),
                            actual_rect(*cached_geom, state.old_offset),
                            true,
                        )
                    } else {
                        (actual_rect(*cached_geom, state.offset), None, false)
                    };

                    new_state.offset += cached_geom.origin.to_vector();
                    new_state.old_offset += cached_geom.origin.to_vector();
                    drop(borrowed);
                    dirty_rects
                }
            }
            _ => {
                drop(borrowed);
                let geom = crate::properties::evaluate_no_tracking(|| {
                    let geom = item_rc.geometry();
                    new_state.offset += geom.origin.to_vector();
                    new_state.old_offset += geom.origin.to_vector();
                    geom
                });
                (actual_rect(geom, state.offset), None, true)
            }
        };

        let opaque = crate::properties::evaluate_no_tracking(|| {
            if is_clipping_item(item) {
                new_state.clipped = dirty_rects.0.unwrap_or_default();
            }
            if !new_state.has_opacity {
                new_state.has_opacity = ItemRef::downcast_pin::<Opacity>(item).map_or(false, |r| r.as_ref().opacity() < 1.);
                !new_state.has_opacity && is_opaque_item(item)
            } else {
                false
            }
        });



        if !new_state.clipped.is_empty() {
            let mut actual_visitor =
                |item_tree: &ItemTreeRc, index: u32, _: Pin<ItemRef>| -> VisitChildrenResult {
                    self.compute_dirty_regions_visit_item(
                        ItemRc::new(item_tree.clone(), index),
                        &new_state,
                    );
                    VisitChildrenResult::CONTINUE
                };
            vtable::new_vref!(let mut actual_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut actual_visitor);
            let item_tree = item_rc.item_tree();
            VRc::borrow_pin(item_tree).as_ref().visit_children_item(
                item_rc.index() as isize,
                item_tree::TraversalOrder::FrontToBack,
                actual_visitor,
            );
        }

        self.z_index_for_opacity += 1;

        let rendering_data = item.cached_rendering_data_offset();
        let mut cache = self.cache.borrow_mut();
        match rendering_data.get_entry(&mut cache) {
            Some(CachedGraphicsData { data, .. }) => {
                data.z_index_for_opacity = self.z_index_for_opacity;
            }
            None => {
                if let Some(geometry) = dirty_rects.0 {
                    let cache_entry = crate::graphics::CachedGraphicsData {
                        data: PartialRenderingCacheData {
                            geometry: geometry.to_rect(),
                            z_index_for_opacity: self.z_index_for_opacity,
                        },
                        dependency_tracker: None,
                    };
                    rendering_data.cache_index.set(cache.insert(cache_entry));
                    rendering_data.cache_generation.set(cache.generation());
                }
            }
        };

        let opacity_index = opaque.then_some(self.z_index_for_opacity);

        if let Some(r) = dirty_rects.0 {
            self.space_partition.add_region(r, dirty_rects.2, opacity_index)
        }
        if dirty_rects.0 != dirty_rects.1 {
            if let Some(r) = dirty_rects.1 {
                self.space_partition.add_region(r, true, None)
            }
        }
    }

    /// Visit the tree of item and compute what are the dirty regions
    pub fn compute_dirty_regions(
        &mut self,
        component: &ItemTreeRc,
        origin: LogicalPoint,
        size: LogicalSize,
    ) {
        self.z_index_for_opacity = 0;
        self.compute_dirty_regions_visit_item(
            ItemRc::new(component.clone(), 0),
            &ComputeDirtyRegionState {
                offset: origin.to_vector(),
                old_offset: origin.to_vector(),
                clipped: euclid::Box2D::from_size(size),
                must_refresh_children: false,
                has_opacity: true,
            },
        );
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
            let cache_entry = crate::graphics::CachedGraphicsData::new(|| {
                PartialRenderingCacheData { geometry: render_fn(), z_index_for_opacity: 0 }
            });
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
        let data = match rendering_data.get_entry(&mut cache) {
            Some(CachedGraphicsData { data, dependency_tracker }) => {
                self.z_index_for_opacity = data.z_index_for_opacity;
                dependency_tracker
                    .get_or_insert_with(|| Box::pin(PropertyTracker::default()))
                    .as_ref()
                    .evaluate_if_dirty(|| data.geometry = eval());
                *data
            }
            None => {
                let cache_entry =
                    crate::graphics::CachedGraphicsData::new(|| PartialRenderingCacheData {
                        geometry: eval(),
                        z_index_for_opacity: self.z_index_for_opacity,
                    });
                let data = cache_entry.data;
                rendering_data.cache_index.set(cache.insert(cache_entry));
                rendering_data.cache_generation.set(cache.generation());
                data
            }
        };

        let clipped_geom = self.get_current_clip().intersection(&data.geometry);
        let mut draw = clipped_geom.map_or(false, |clipped_geom| {
            let clipped_geom = clipped_geom.translate(self.translation());
            self.space_partition.draw_intersects(clipped_geom.to_box2d(), data.z_index_for_opacity)
        });

        (draw, data.geometry)
    }

    forward_rendering_call!(fn draw_rectangle(Rectangle));
    forward_rendering_call2!(fn draw_border_rectangle(dyn RenderBorderRectangle));
    forward_rendering_call2!(fn draw_image(dyn RenderImage));
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

    fn window(&self) -> &crate::window::WindowInner {
        self.actual_renderer.window()
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        self.actual_renderer.as_any()
    }
}

#[allow(unused)]
mod space_partition {

    /*
        A region can be either:
          1. Opaque non-dirty. (the region cannot be split further, this is not going to be drawn)
          2. Opaque dirty (the region cannot be split further, but there is a opacity value to it)
          3. dirty not opaque the region can still be marked opaque
          4. not dirty, but may become dirty if items under are dirty
    */

    use crate::lengths::{LogicalPoint, LogicalPx, LogicalRect, LogicalSize};
    use crate::Coord;
    #[cfg(not(feature = "std"))]
    use alloc::boxed::Box;

    type Box2D = euclid::Box2D<Coord, LogicalPx>;

    #[derive(Debug)]
    pub enum Node {
        Leaf {
            opacity: Option<usize>,
            dirty: bool,
        },
        Division {
            horizontal: bool,
            split: Coord,
            left: Box<Node>,
            right: Box<Node>,
            has_dirty: bool,
        },
    }

    #[derive(Debug)]
    pub struct SpacePartition {
        root: Node,
        size: LogicalSize,
    }

    impl SpacePartition {
        pub fn new(size: LogicalSize) -> Self {
            Self { root: Node::Leaf { opacity: None, dirty: false }, size }
        }

        pub fn add_region(&mut self, region: Box2D, dirty: bool, opacity_index: Option<usize>) {
            let node_geometry = Box2D::from_size(self.size);
            let Some(region) = region.intersection(&node_geometry) else { return };
            if !dirty
                && (opacity_index.is_none()
                    || region.width() < self.size.width / (15 as Coord)
                    || region.height() < self.size.height / (15 as Coord))
            {
                return;
            }

            Self::add_region_impl(&mut self.root, node_geometry, region, dirty, opacity_index);
        }

        /// returns true if something in the subtree is becoming dirty
        fn add_region_impl(
            node: &mut Node,
            node_geometry: Box2D,
            b: Box2D,
            is_dirty: bool,
            opacity_index: Option<usize>,
        ) -> bool {
            let Some(intersection) = node_geometry.intersection(&b) else { return false };
            if intersection.is_empty() {
                return false;
            };
            match node {
                Node::Leaf { opacity, dirty } => {
                    if opacity.is_some() {
                        // We are already covered by another node, we don't care
                        return false;
                    }
                    if (*dirty || !is_dirty) && opacity_index.is_none() {
                        // Already marked dirty and no opacity, nothing to add
                        return false;
                    }

                    if b.contains_box(&node_geometry) {
                        // The node is entirely contained, no need to split
                        *dirty |= is_dirty;
                        *opacity = opacity_index;
                    } else {
                        // We need to split further
                        let old_leaf = || Box::new(Node::Leaf { opacity: *opacity, dirty: *dirty });
                        let has_dirty = *dirty | is_dirty;
                        let mut new_node = Node::Leaf { opacity: opacity_index, dirty: has_dirty };
                        if b.max.x < node_geometry.max.x {
                            new_node = Node::Division {
                                horizontal: true,
                                split: b.max.x,
                                left: new_node.into(),
                                right: old_leaf(),
                                has_dirty,
                            };
                        }
                        if b.min.x > node_geometry.min.x {
                            new_node = Node::Division {
                                horizontal: true,
                                split: b.min.x,
                                left: old_leaf(),
                                right: new_node.into(),
                                has_dirty,
                            };
                        }
                        if b.max.y < node_geometry.max.y {
                            new_node = Node::Division {
                                horizontal: false,
                                split: b.max.y,
                                left: new_node.into(),
                                right: old_leaf(),
                                has_dirty,
                            };
                        }
                        if b.min.y > node_geometry.min.y {
                            new_node = Node::Division {
                                horizontal: false,
                                split: b.min.y,
                                left: old_leaf(),
                                right: new_node.into(),
                                has_dirty,
                            };
                        }
                        *node = new_node;
                    }
                    is_dirty
                }
                Node::Division { horizontal, split, left, right, has_dirty } => {
                    if *horizontal {
                        debug_assert!(*split < node_geometry.max.x, "{split} in {node_geometry:?}");
                        debug_assert!(*split > node_geometry.min.x, "{split} in {node_geometry:?}");
                    } else {
                        debug_assert!(*split < node_geometry.max.y, "{split} in {node_geometry:?}");
                        debug_assert!(*split > node_geometry.min.y, "{split} in {node_geometry:?}");
                    }

                    let set_coord = |point: &mut LogicalPoint, val| {
                        if *horizontal {
                            point.x = val
                        } else {
                            point.y = val
                        }
                    };
                    let mut sub = node_geometry;
                    set_coord(&mut sub.max, *split);
                    *has_dirty |=
                        Self::add_region_impl(&mut *left, sub, b, is_dirty, opacity_index);
                    let mut sub = node_geometry;
                    set_coord(&mut sub.min, *split);
                    *has_dirty |=
                        Self::add_region_impl(&mut *right, sub, b, is_dirty, opacity_index);
                    *has_dirty
                }
            }
        }

        /// return true if part of the region should be drawn
        pub fn draw_intersects(&self, region: Box2D, z_index: usize) -> bool {
            Self::draw_intersects_impl(&self.root, Box2D::from_size(self.size), region, z_index)
        }

        fn draw_intersects_impl(
            node: &Node,
            node_geometry: Box2D,
            b: Box2D,
            z_index: usize,
        ) -> bool {
            let Some(intersection) = node_geometry.intersection(&b) else { return false };
            if intersection.is_empty() {
                return false;
            };
            match node {
                Node::Leaf { opacity, dirty } => {
                    if !dirty {
                        return false;
                    }
                    opacity.map_or(true, |v| v >= z_index)
                }
                Node::Division { horizontal, split, left, right, has_dirty } => {
                    if *horizontal {
                        debug_assert!(*split < node_geometry.max.x, "{split} in {node_geometry:?}");
                        debug_assert!(*split > node_geometry.min.x, "{split} in {node_geometry:?}");
                    } else {
                        debug_assert!(*split < node_geometry.max.y, "{split} in {node_geometry:?}");
                        debug_assert!(*split > node_geometry.min.y, "{split} in {node_geometry:?}");
                    }

                    if !has_dirty {
                        return false;
                    }

                    let set_coord = |point: &mut LogicalPoint, val| {
                        if *horizontal {
                            point.x = val
                        } else {
                            point.y = val
                        }
                    };
                    ({
                        let mut sub = node_geometry;
                        set_coord(&mut sub.max, *split);
                        Self::draw_intersects_impl(&left, sub, b, z_index)
                    } || {
                        let mut sub = node_geometry;
                        set_coord(&mut sub.min, *split);
                        Self::draw_intersects_impl(&right, sub, b, z_index)
                    })
                }
            }
        }

        pub fn bounding_box(&self) -> Box2D {
            Self::bounding_box_impl(&self.root, Box2D::from_size(self.size))
        }

        fn bounding_box_impl(node: &Node, node_geometry: Box2D) -> Box2D {
            if node_geometry.is_empty() {
                return node_geometry;
            };
            match node {
                Node::Leaf { dirty, .. } if *dirty => node_geometry,
                Node::Leaf { dirty, .. } => Box2D::zero(),
                Node::Division { has_dirty, .. } if !*has_dirty => Box2D::zero(),
                Node::Division { horizontal, split, left, right, .. } => {
                    if *horizontal {
                        debug_assert!(*split < node_geometry.max.x, "{split} in {node_geometry:?}");
                        debug_assert!(*split > node_geometry.min.x, "{split} in {node_geometry:?}");
                    } else {
                        debug_assert!(*split < node_geometry.max.y, "{split} in {node_geometry:?}");
                        debug_assert!(*split > node_geometry.min.y, "{split} in {node_geometry:?}");
                    }

                    let set_coord = |point: &mut LogicalPoint, val| {
                        if *horizontal {
                            point.x = val
                        } else {
                            point.y = val
                        }
                    };

                    let mut sub = node_geometry;
                    set_coord(&mut sub.max, *split);
                    let a = Self::bounding_box_impl(&left, sub);

                    let mut sub = node_geometry;
                    set_coord(&mut sub.min, *split);
                    let b = Self::bounding_box_impl(&right, sub);
                    a.union(&b)
                }
            }
        }

        pub fn debug(&self) -> impl core::fmt::Debug {
            Self::debug_impl(&self.root)
        }

        fn debug_impl(node: &Node) -> (usize, usize) {
            match node {
                Node::Leaf { opacity, dirty } => (1, 1),
                Node::Division { left, right, .. } => {
                    let a = Self::debug_impl(&left);
                    let b = Self::debug_impl(&right);
                    (a.0.max(b.0) + 1, a.1 + b.1)
                }
            }
        }
    }
}
