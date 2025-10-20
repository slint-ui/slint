// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![warn(missing_docs)]
//! module for rendering the tree of items

use super::items::*;
use crate::graphics::{FontRequest, Image, IntRect};
use crate::item_tree::ItemTreeRc;
use crate::item_tree::{ItemVisitor, ItemVisitorVTable, VisitChildrenResult};
use crate::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
};
pub use crate::partial_renderer::CachedRenderingData;
use crate::window::WindowAdapter;
use crate::{Brush, SharedString};
#[cfg(feature = "std")]
use alloc::boxed::Box;
use alloc::rc::Rc;
#[cfg(feature = "std")]
use core::cell::RefCell;
use core::pin::Pin;
#[cfg(feature = "std")]
use std::collections::HashMap;
use vtable::VRc;

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
    map: RefCell<HashMap<*const vtable::Dyn, HashMap<u32, crate::graphics::CachedGraphicsData<T>>>>,
    /// Track if the window scale factor changes; used to clear the cache if necessary.
    window_scale_factor_tracker: Pin<Box<crate::properties::PropertyTracker>>,
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
                let new_entry = crate::graphics::CachedGraphicsData::new(update_fn);
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

/// Renders the children of the item with the specified index into the renderer.
pub fn render_item_children(
    renderer: &mut dyn ItemRenderer,
    component: &ItemTreeRc,
    index: isize,
    window_adapter: &Rc<dyn WindowAdapter>,
) {
    let mut actual_visitor =
        |component: &ItemTreeRc, index: u32, item: Pin<ItemRef>| -> VisitChildrenResult {
            renderer.save_state();
            let item_rc = ItemRc::new(component.clone(), index);

            let (do_draw, item_geometry) = renderer.filter_item(&item_rc, window_adapter);

            let item_origin = item_geometry.origin;
            renderer.translate(item_origin.to_vector());

            // Don't render items that are clipped, with the exception of the Clip or Flickable since
            // they themselves clip their content.
            let render_result = if do_draw
               || item.as_ref().clips_children()
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
                render_item_children(renderer, component, index as isize, window_adapter);
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
    window_adapter: &Rc<dyn WindowAdapter>,
) {
    renderer.save_state();
    renderer.translate(origin.to_vector());

    render_item_children(renderer, component, -1, window_adapter);

    renderer.restore_state();
}

/// Compute the bounding rect of all children. This does /not/ include item's own bounding rect. Remember to run this
/// via `evaluate_no_tracking`.
pub fn item_children_bounding_rect(
    component: &ItemTreeRc,
    index: isize,
    clip_rect: &LogicalRect,
) -> LogicalRect {
    item_children_bounding_rect_inner(component, index, clip_rect, Default::default())
}

fn item_children_bounding_rect_inner(
    component: &ItemTreeRc,
    index: isize,
    clip_rect: &LogicalRect,
    transform: crate::lengths::ItemTransform,
) -> LogicalRect {
    let mut bounding_rect = LogicalRect::zero();

    let mut actual_visitor =
        |component: &ItemTreeRc, index: u32, item: Pin<ItemRef>| -> VisitChildrenResult {
            let item_geometry = transform.outer_transformed_rect(
                &ItemTreeRc::borrow_pin(component).as_ref().item_geometry(index).cast(),
            );
            let children_transform = ItemRc::new(component.clone(), index)
                .children_transform()
                .unwrap_or_default()
                .then_translate(item_geometry.origin.to_vector());

            let offset: LogicalPoint = item_geometry.origin.cast();
            let local_clip_rect = clip_rect.translate(-offset.to_vector());

            if let Some(clipped_item_geometry) = item_geometry.intersection(&clip_rect.cast()) {
                bounding_rect = bounding_rect.union(&clipped_item_geometry.cast());
            }

            if !item.as_ref().clips_children() {
                bounding_rect = bounding_rect.union(&item_children_bounding_rect_inner(
                    component,
                    index as isize,
                    &local_clip_rect,
                    transform.then(&children_transform),
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
pub trait RenderRectangle {
    fn background(self: Pin<&Self>) -> Brush;
}

/// Trait for an item that represent a Rectangle with a border to the Renderer
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
    fn font_request(self: Pin<&Self>, self_rc: &ItemRc) -> FontRequest;
    fn color(self: Pin<&Self>) -> Brush;
    fn alignment(self: Pin<&Self>) -> (TextHorizontalAlignment, TextVerticalAlignment);
    fn wrap(self: Pin<&Self>) -> TextWrap;
    fn overflow(self: Pin<&Self>) -> TextOverflow;
    fn letter_spacing(self: Pin<&Self>) -> LogicalLength;
    fn stroke(self: Pin<&Self>) -> (Brush, LogicalLength, TextStrokeStyle);
    fn is_markdown(self: Pin<&Self>) -> bool;
}

impl RenderText for (SharedString, Brush) {
    fn target_size(self: Pin<&Self>) -> LogicalSize {
        LogicalSize::default()
    }

    fn text(self: Pin<&Self>) -> SharedString {
        self.0.clone()
    }

    fn font_request(self: Pin<&Self>, _self_rc: &ItemRc) -> crate::graphics::FontRequest {
        Default::default()
    }

    fn color(self: Pin<&Self>) -> Brush {
        self.1.clone()
    }

    fn alignment(
        self: Pin<&Self>,
    ) -> (crate::items::TextHorizontalAlignment, crate::items::TextVerticalAlignment) {
        Default::default()
    }

    fn wrap(self: Pin<&Self>) -> crate::items::TextWrap {
        Default::default()
    }

    fn overflow(self: Pin<&Self>) -> crate::items::TextOverflow {
        Default::default()
    }

    fn letter_spacing(self: Pin<&Self>) -> LogicalLength {
        LogicalLength::default()
    }

    fn stroke(self: Pin<&Self>) -> (Brush, LogicalLength, TextStrokeStyle) {
        Default::default()
    }

    fn is_markdown(self: Pin<&Self>) -> bool {
        false
    }
}

/// Trait used to render each items.
///
/// The item needs to be rendered relative to its (x,y) position. For example,
/// draw_rectangle should draw a rectangle in `(pos.x + rect.x, pos.y + rect.y)`
#[allow(missing_docs)]
pub trait ItemRenderer {
    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    );
    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn RenderBorderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    );
    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        self_rc: &ItemRc,
        size: LogicalSize,
        cache: &CachedRenderingData,
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
        _item_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        if clip_item.clip() {
            let clip_region_valid = self.combine_clip(
                LogicalRect::new(LogicalPoint::default(), size),
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
    fn scale(&mut self, scale_x_factor: f32, scale_y_factor: f32);
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
    fn filter_item(
        &mut self,
        item: &ItemRc,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) -> (bool, LogicalRect) {
        let item_geometry = item.geometry();
        // Query bounding rect untracked, as properties that affect the bounding rect are already tracked
        // when rendering the item.
        let bounding_rect = crate::properties::evaluate_no_tracking(|| {
            item.bounding_rect(&item_geometry, window_adapter)
        });
        (self.get_current_clip().intersects(&bounding_rect), item_geometry)
    }

    fn window(&self) -> &crate::window::WindowInner;

    /// Return the internal renderer
    fn as_any(&mut self) -> Option<&mut dyn core::any::Any>;
}

/// Helper trait to express the features of an item renderer.
pub trait ItemRendererFeatures {
    /// The renderer supports applying 2D transformations to items.
    const SUPPORTS_TRANSFORMATIONS: bool;
}
