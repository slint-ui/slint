// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Module for a renderer proxy that tries to render only the parts of the tree that have changed.
//!
//! This is the way the partial renderer work:
//!
//! 1. [`PartialRenderer::compute_dirty_regions`] will go over the items and try to compute the region that needs to be repainted.
//!    If either the bounding box has changed, or the PropertyTracker that tracks the rendering properties is dirty, then the
//!    region is marked dirty.
//!    That pass also register dependencies on every geometry, and on the non-dirty property trackers.
//! 2. The Renderer calls [`PartialRenderer::filter_item`] For most items.
//!    This assume that the cached geometry was requested in the previous step. So it will not register new dependencies.
//! 3. Then the renderer calls the rendering function for each item that needs to be rendered.
//!    This register dependencies only on the rendering tracker.
//!

use crate::Coord;
#[cfg(feature = "occlusion-culling")]
use crate::item_rendering::is_opaque_covering_rectangle;
use crate::item_rendering::{
    ItemRenderer, ItemRendererFeatures, RenderBorderRectangle, RenderImage, RenderRectangle,
    RenderText,
};
use crate::item_tree::{
    ItemTreeRc, ItemTreeWeak, ItemVisitor, ItemVisitorVTable, VisitChildrenResult,
};
#[cfg(feature = "path")]
use crate::items::Path;
use crate::items::{BoxShadow, Clip, ItemRc, ItemRef, Layer, Opacity, RenderingResult, TextInput};
use crate::lengths::{
    ItemTransform, LogicalBorderRadius, LogicalPoint, LogicalPx, LogicalRect, LogicalSize,
    LogicalVector, ScaleFactor,
};
use crate::properties::PropertyTracker;
use crate::window::WindowAdapter;
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
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
    fn release(
        &self,
        cache: &mut PartialRendererCache,
    ) -> Option<CachedItemBoundingBoxAndTransform> {
        if self.cache_generation.get() == cache.generation() {
            let index = self.cache_index.get();
            self.cache_generation.set(0);
            Some(cache.remove(index).data)
        } else {
            None
        }
    }

    /// Return the value if it is in the cache
    fn get_entry<'a>(
        &self,
        cache: &'a mut PartialRendererCache,
    ) -> Option<&'a mut PartialRenderingCachedData> {
        let index = self.cache_index.get();
        if self.cache_generation.get() == cache.generation() { cache.get_mut(index) } else { None }
    }
}

/// After rendering an item, we cache the geometry and the transform it applies to
/// children.
///
/// `sibling_index` (the item's rank among all its z-ordered siblings when it was last
/// visited; compared in `compute_dirty_regions` against the rank counted over the items
/// that already had a cache entry, so appearing siblings don't shift it) is a `u16` stored
/// in each variant, so it fits the enum's padding without growing the cache entry on 32- or
/// 64-bit. It is excluded from geometry comparisons.
#[derive(Clone)]
pub enum CachedItemBoundingBoxAndTransform {
    /// A regular item with a translation
    RegularItem {
        /// The item's bounding rect relative to its parent.
        bounding_rect: LogicalRect,
        /// The item's offset relative to its parent.
        offset: LogicalVector,
        sibling_index: u16,
    },
    /// An item such as Rotate that defines an additional transformation
    ItemWithTransform {
        /// The item's bounding rect relative to its parent.
        bounding_rect: LogicalRect,
        /// The item's transform to apply to children.
        transform: Box<ItemTransform>,
        sibling_index: u16,
    },
    /// A clip item.
    ClipItem {
        /// The item's geometry relative to its parent.
        geometry: LogicalRect,
        sibling_index: u16,
    },
}

impl CachedItemBoundingBoxAndTransform {
    fn bounding_rect(&self) -> &LogicalRect {
        match self {
            CachedItemBoundingBoxAndTransform::RegularItem { bounding_rect, .. } => bounding_rect,
            CachedItemBoundingBoxAndTransform::ItemWithTransform { bounding_rect, .. } => {
                bounding_rect
            }
            CachedItemBoundingBoxAndTransform::ClipItem { geometry, .. } => geometry,
        }
    }

    fn transform(&self) -> ItemTransform {
        match self {
            CachedItemBoundingBoxAndTransform::RegularItem { offset, .. } => {
                ItemTransform::translation(offset.x as f32, offset.y as f32)
            }
            CachedItemBoundingBoxAndTransform::ItemWithTransform { transform, .. } => **transform,
            CachedItemBoundingBoxAndTransform::ClipItem { geometry, .. } => {
                ItemTransform::translation(geometry.origin.x as f32, geometry.origin.y as f32)
            }
        }
    }

    fn sibling_index(&mut self) -> &mut u16 {
        match self {
            CachedItemBoundingBoxAndTransform::RegularItem { sibling_index, .. }
            | CachedItemBoundingBoxAndTransform::ItemWithTransform { sibling_index, .. }
            | CachedItemBoundingBoxAndTransform::ClipItem { sibling_index, .. } => sibling_index,
        }
    }

    /// Compare the geometry (bounding rect, transform, clip), ignoring `sibling_index`.
    fn same_geometry(&self, other: &Self) -> bool {
        use CachedItemBoundingBoxAndTransform::*;
        match (self, other) {
            (
                RegularItem { bounding_rect: a, offset: oa, .. },
                RegularItem { bounding_rect: b, offset: ob, .. },
            ) => a == b && oa == ob,
            (
                ItemWithTransform { bounding_rect: a, transform: ta, .. },
                ItemWithTransform { bounding_rect: b, transform: tb, .. },
            ) => a == b && ta == tb,
            (ClipItem { geometry: a, .. }, ClipItem { geometry: b, .. }) => a == b,
            _ => false,
        }
    }

    fn new<T: ItemRendererFeatures>(
        item_rc: &ItemRc,
        window_adapter: &Rc<dyn WindowAdapter>,
        sibling_index: u16,
    ) -> Self {
        let geometry = item_rc.geometry();

        if item_rc.borrow().as_ref().clips_children() {
            return Self::ClipItem { geometry, sibling_index };
        }

        // Evaluate the bounding rect untracked, as properties that affect the bounding rect are already tracked
        // at rendering time.
        let bounding_rect = crate::properties::evaluate_no_tracking(|| {
            item_rc.bounding_rect(&geometry, window_adapter)
        });

        if let Some(complex_child_transform) = (T::SUPPORTS_TRANSFORMATIONS
            && window_adapter.renderer().supports_transformations())
        .then(|| item_rc.children_transform())
        .flatten()
        {
            Self::ItemWithTransform {
                bounding_rect,
                transform: complex_child_transform
                    .then_translate(geometry.origin.to_vector().cast())
                    .into(),
                sibling_index,
            }
        } else {
            Self::RegularItem { bounding_rect, offset: geometry.origin.to_vector(), sibling_index }
        }
    }
}

struct PartialRenderingCachedData {
    /// The geometry of the item as it was previously rendered.
    pub data: CachedItemBoundingBoxAndTransform,
    /// The property tracker that should be used to evaluate whether the item needs to be re-rendered
    pub tracker: Option<core::pin::Pin<Box<PropertyTracker>>>,
    /// Whether `PartialRenderer::compute_occlusion` found this item fully hidden behind opaque content painted after it.
    /// Recomputed every frame that pass runs.
    #[cfg(feature = "occlusion-culling")]
    pub occluded: bool,
    /// Screen-space bounding rect of this item's own rendering plus everything painted by its
    /// descendants this frame; `None` if nothing in the subtree is visible.
    #[cfg(feature = "occlusion-culling")]
    pub subtree_screen_bounds: Option<LogicalRect>,
    /// The `transform_to_screen` that `compute_dirty_regions_recursive` passed down to this item's children this frame.
    /// Cached here so `compute_occlusion_recursive` can reuse it instead of re-composing the same matrix a second time in the same frame.
    #[cfg(feature = "occlusion-culling")]
    pub child_transform_to_screen: ItemTransform,
}
impl PartialRenderingCachedData {
    fn new(data: CachedItemBoundingBoxAndTransform) -> Self {
        Self {
            data,
            tracker: None,
            #[cfg(feature = "occlusion-culling")]
            occluded: false,
            #[cfg(feature = "occlusion-culling")]
            subtree_screen_bounds: None,
            #[cfg(feature = "occlusion-culling")]
            child_transform_to_screen: ItemTransform::identity(),
        }
    }
}

/// The cache that needs to be held by the Window for the partial rendering
struct PartialRendererCache {
    slab: slab::Slab<PartialRenderingCachedData>,
    generation: usize,
}

impl Default for PartialRendererCache {
    fn default() -> Self {
        Self { slab: Default::default(), generation: 1 }
    }
}

impl PartialRendererCache {
    /// Returns the generation of the cache. The generation starts at 1 and is increased
    /// whenever the cache is cleared, for example when the GL context is lost.
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// Retrieves a mutable reference to the cached graphics data at index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut PartialRenderingCachedData> {
        self.slab.get_mut(index)
    }

    /// Inserts data into the cache and returns the index for retrieval later.
    pub fn insert(&mut self, data: PartialRenderingCachedData) -> usize {
        self.slab.insert(data)
    }

    /// Removes the cached graphics data at the given index.
    pub fn remove(&mut self, index: usize) -> PartialRenderingCachedData {
        self.slab.remove(index)
    }

    /// Removes all entries from the cache and increases the cache's generation count, so
    /// that stale index access can be avoided.
    pub fn clear(&mut self) {
        self.slab.clear();
        self.generation += 1;
    }
}

/// Scans `rectangles[..*count]` against `b`: returns `true` if `b` is already fully contained in one of them (nothing to do),
/// swap-removing along the way any rectangle that `b` itself fully contains (now redundant).
/// Returns `false` if `b` still needs to be inserted or merged in by the caller, which knows its own overflow policy.
fn scan_and_collapse(
    rectangles: &mut [euclid::Box2D<Coord, LogicalPx>],
    count: &mut usize,
    b: &euclid::Box2D<Coord, LogicalPx>,
) -> bool {
    let mut i = 0;
    while i < *count {
        let r = &rectangles[i];
        if r.contains_box(b) {
            return true;
        } else if b.contains_box(r) {
            rectangles.swap(i, *count - 1);
            *count -= 1;
            continue;
        }
        i += 1;
    }
    false
}

/// A region composed of a few rectangles that need to be redrawn.
#[derive(Default, Clone)]
pub struct DirtyRegion {
    rectangles: [euclid::Box2D<Coord, LogicalPx>; Self::MAX_COUNT],
    count: usize,
}

impl core::fmt::Debug for DirtyRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", &self.rectangles[..self.count])
    }
}

impl DirtyRegion {
    /// The maximum number of rectangles that can be stored in a DirtyRegion
    pub const MAX_COUNT: usize = 3;

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
        if scan_and_collapse(&mut self.rectangles, &mut self.count, &b) {
            return;
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
                ret.count -= 1;
                ret.rectangles.swap(i, ret.count);
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

/// A region of screen space conservatively known to be fully covered by opaque content,
/// built up by [`PartialRenderer::compute_occlusion`] to skip drawing items that can never be visible.
#[cfg(feature = "occlusion-culling")]
#[derive(Default)]
struct OccludedRegion {
    rectangles: [euclid::Box2D<Coord, LogicalPx>; Self::MAX_COUNT],
    count: usize,
}

#[cfg(feature = "occlusion-culling")]
impl OccludedRegion {
    // Larger than `DirtyRegion::MAX_COUNT` (3): a miss here only misses a culling opportunity
    // (see `contains_rect`), not correctness, so it's worth remembering more opaque covers.
    const MAX_COUNT: usize = 8;

    /// Record `rect` (in screen coordinates) as fully opaquely covered.
    fn add_rect(&mut self, rect: LogicalRect) {
        let b = rect.to_box2d();
        if b.is_empty() {
            return;
        }
        if scan_and_collapse(&mut self.rectangles, &mut self.count, &b) {
            return;
        }

        if self.count < Self::MAX_COUNT {
            self.rectangles[self.count] = b;
            self.count += 1;
        }
        // else: silently drop rather than merge
    }

    /// Whether `rect` is fully contained in a single recorded rectangle.
    /// Doesn't detect coverage split across multiple rectangles which is a missed optimization, not a soundness issue.
    fn contains_rect(&self, rect: LogicalRect) -> bool {
        let b = rect.to_box2d();
        if b.is_empty() {
            return true;
        }
        self.rectangles[..self.count].iter().any(|r| r.contains_box(&b))
    }
}

/// This enum describes which parts of the buffer passed to the `SoftwareRenderer` may be re-used to speed up painting.
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
///
/// This is constructed from a [`PartialRenderingState`]
pub struct PartialRenderer<'a, T> {
    cache: &'a RefCell<PartialRendererCache>,
    /// The region of the screen which is considered dirty and that should be repainted
    pub dirty_region: DirtyRegion,
    /// The actual renderer which the drawing call will be forwarded to
    pub actual_renderer: T,
    /// The window adapter the renderer is rendering into.
    pub window_adapter: Rc<dyn WindowAdapter>,
}

#[cfg(feature = "occlusion-culling")]
#[derive(Clone, Copy)]
struct ComputeOcclusionState {
    transform_to_screen: ItemTransform,
    clipped: LogicalRect,
    /// Like `clipped`, but also narrowed to the largest rectangle inscribed in every ancestor `Clip`'s rounded shape,
    /// not just its rectangular `geometry`.
    /// Content recorded as occluded must stay within this tighter bound, or rounded-off corners get wrongly culled.
    occluder_clip: LogicalRect,
    /// False once an ancestor makes opaque-coverage claims from descendants unsound
    may_contribute: bool,
}

/// Depth-first, postorder: recurse into `index`'s children before considering whether `index` itself is occluded or contributes to `accumulator`.
#[cfg(feature = "occlusion-culling")]
fn compute_occlusion_recursive(
    component: &ItemTreeRc,
    index: isize,
    cache: &RefCell<PartialRendererCache>,
    dirty_region: &DirtyRegion,
    accumulator: &mut OccludedRegion,
    state: ComputeOcclusionState,
) {
    let mut child_visitor = |child_component: &ItemTreeRc,
                             child_index: u32,
                             item: Pin<ItemRef>|
     -> VisitChildrenResult {
        let rendering_data = item.cached_rendering_data_offset();

        let (cached_geom, subtree_bounds, child_transform_to_screen) = {
            let mut cache = cache.borrow_mut();
            match rendering_data.get_entry(&mut cache) {
                Some(entry) => (
                    entry.data.clone(),
                    entry.subtree_screen_bounds,
                    entry.child_transform_to_screen,
                ),
                // Not in the cache yet (e.g. just created this frame) -- nothing sound to say.
                None => return VisitChildrenResult::CONTINUE,
            }
        };

        // `subtree_bounds` was computed fresh this frame by compute_dirty_regions's postorder
        // pass.
        match subtree_bounds {
            Some(bounds) if dirty_region.draw_intersects(bounds) => {}
            // If it doesn't overlap the dirty region, nothing will be drawn anyway
            _ => return VisitChildrenResult::CONTINUE,
        }

        let mut child_state = state;
        // Reuses the transform compute_dirty_regions_recursive already composed this frame for
        // the same purpose.
        child_state.transform_to_screen = child_transform_to_screen;
        child_state.may_contribute &=
            !matches!(cached_geom, CachedItemBoundingBoxAndTransform::ItemWithTransform { .. });
        if let Some(opacity) = ItemRef::downcast_pin::<Opacity>(item) {
            child_state.may_contribute &= opacity.opacity() >= 1.0;
        }
        if let CachedItemBoundingBoxAndTransform::ClipItem { geometry, .. } = &cached_geom {
            let screen_clip_rect =
                state.transform_to_screen.outer_transformed_rect(&geometry.cast()).cast();
            child_state.clipped =
                child_state.clipped.intersection(&screen_clip_rect).unwrap_or_default();

            // Only `Clip` items produce `ClipItem`, so this always downcasts.
            // Inset by the largest corner radius as an inscribed-rectangle
            // approximation of the rounded shape.
            let corner_inset = ItemRef::downcast_pin::<Clip>(item)
                .map(|clip| {
                    let r = clip.logical_border_radius();
                    r.top_left.max(r.top_right).max(r.bottom_left).max(r.bottom_right)
                })
                .unwrap_or_default();
            let occluder_bound = if corner_inset > 0 as Coord {
                screen_clip_rect.inflate(-corner_inset, -corner_inset)
            } else {
                screen_clip_rect
            };
            child_state.occluder_clip =
                child_state.occluder_clip.intersection(&occluder_bound).unwrap_or_default();
        }

        // Postorder: descendants paint in front of `item_rc` -- let them contribute to
        // `accumulator` before testing/recording `item_rc` itself.
        compute_occlusion_recursive(
            child_component,
            child_index as isize,
            cache,
            dirty_region,
            accumulator,
            child_state,
        );

        let visible_rect = state
            .transform_to_screen
            .outer_transformed_rect(&cached_geom.bounding_rect().cast())
            .cast()
            .intersection(&state.clipped);

        if let Some(visible_rect) = visible_rect {
            let occluded = accumulator.contains_rect(visible_rect);

            {
                let mut cache = cache.borrow_mut();
                if let Some(entry) = rendering_data.get_entry(&mut cache) {
                    entry.occluded = occluded;
                }
            }

            // Bounded by `occluder_clip`, not `clipped`: what gets *recorded* as occluded must
            // never over claim past a rounded `Clip` ancestor's actual painted shape.
            if !occluded
                && state.may_contribute
                && is_opaque_covering_rectangle(item)
                && let Some(occluder_rect) = visible_rect.intersection(&state.occluder_clip)
            {
                accumulator.add_rect(occluder_rect);
            }
        }

        VisitChildrenResult::CONTINUE
    };
    vtable::new_vref!(let mut child_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut child_visitor);
    VRc::borrow_pin(component).as_ref().visit_children_item(
        index,
        crate::item_tree::TraversalOrder::FrontToBack,
        child_visitor,
    );
}

#[derive(Clone, Copy)]
struct ComputeDirtyRegionState {
    transform_to_screen: ItemTransform,
    old_transform_to_screen: ItemTransform,
    clipped: LogicalRect,
    must_refresh_children: bool,
    /// Depth of the item in the tree, used to index `sibling_counters`.
    depth: usize,
}

impl ComputeDirtyRegionState {
    /// Adjust transform_to_screen and old_transform_to_screen to map from item coordinates
    /// to the screen when using it on a child, specified by its children transform.
    fn adjust_transforms_for_child(
        &mut self,
        children_transform: &ItemTransform,
        old_children_transform: &ItemTransform,
    ) {
        self.transform_to_screen = children_transform.then(&self.transform_to_screen);
        self.old_transform_to_screen = old_children_transform.then(&self.old_transform_to_screen);
    }
}

/// Transform `rect` by `transform` and intersect it with `clip_rect`.
/// `None` if `rect` is empty, non-finite, or entirely clipped away.
fn transformed_and_clipped(
    rect: &LogicalRect,
    transform: ItemTransform,
    clip_rect: &LogicalRect,
) -> Option<LogicalRect> {
    #[cfg(not(slint_int_coord))]
    if !rect.origin.is_finite() {
        // Account for NaN
        return None;
    }
    if rect.is_empty() {
        return None;
    }
    transform.outer_transformed_rect(&rect.cast()).cast().intersection(clip_rect)
}

/// Mark `rect` (in the coordinate system `transform` maps to the screen, clipped by `clip_rect`) as needing to be repainted this frame.
fn mark_dirty_rect(
    dirty_region: &mut DirtyRegion,
    rect: &LogicalRect,
    transform: ItemTransform,
    clip_rect: &LogicalRect,
) {
    if let Some(rect) = transformed_and_clipped(rect, transform, clip_rect) {
        dirty_region.add_rect(rect);
    }
}

/// Transform+clip `rect` to screen space like `mark_dirty_rect`, for folding into a subtree's aggregate screen bound.
/// `None` if `rect` is empty, non-finite, or entirely clipped away.
#[cfg(feature = "occlusion-culling")]
fn screen_rect_for(
    rect: &LogicalRect,
    transform: ItemTransform,
    clip_rect: &LogicalRect,
) -> Option<LogicalRect> {
    transformed_and_clipped(rect, transform, clip_rect)
}

#[cfg(not(feature = "occlusion-culling"))]
fn screen_rect_for(
    _rect: &LogicalRect,
    _transform: ItemTransform,
    _clip_rect: &LogicalRect,
) -> Option<LogicalRect> {
    None
}

/// Union two optional screen rects, treating `None` as "contributes nothing" rather than an empty rect at the origin (which would pull the union toward (0, 0)).
fn union_opt_rect(a: Option<LogicalRect>, b: Option<LogicalRect>) -> Option<LogicalRect> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(a), Some(b)) => Some(a.union(&b)),
    }
}

/// Depth-first walk that computes dirty regions for `index`'s children
/// Returns the union of `index`'s children's subtree bounds, or `None` if nothing under `index` is visible this frame.
fn compute_dirty_regions_recursive<T: ItemRendererFeatures>(
    component: &ItemTreeRc,
    index: isize,
    cache: &RefCell<PartialRendererCache>,
    window_adapter: &Rc<dyn WindowAdapter>,
    dirty_region: &mut DirtyRegion,
    // Two counters per tree depth to give each item its rank among its z-ordered siblings.
    // `.0` counts every visited item and is what gets stored in the cache entry; `.1`
    // counts only the items that already have a cache entry and is what the stored rank
    // is compared against. New items are skipped in the comparison rank so that an
    // appearing sibling does not shift the ranks of the existing items (their overlap
    // with the new sibling is covered by the new item's own dirty rect), while two
    // existing items can never trade places without at least one comparison rank
    // changing.
    sibling_counters: &RefCell<alloc::vec::Vec<(u16, u16)>>,
    state: ComputeDirtyRegionState,
) -> Option<LogicalRect> {
    let mut aggregate: Option<LogicalRect> = None;

    let mut child_visitor = |child_component: &ItemTreeRc,
                             child_index: u32,
                             item: Pin<ItemRef>|
     -> VisitChildrenResult {
        let mut new_state = state;
        let item_rc = ItemRc::new(child_component.clone(), child_index);

        let my_sibling_index = {
            let depth = state.depth;
            let mut counters = sibling_counters.borrow_mut();
            if counters.len() <= depth + 1 {
                counters.resize(depth + 2, (0, 0));
            }
            counters[depth + 1] = (0, 0); // this item's children restart at zero
            let idx = counters[depth].0;
            counters[depth].0 = idx.saturating_add(1);
            idx
        };
        new_state.depth = state.depth + 1;

        let new_geom =
            CachedItemBoundingBoxAndTransform::new::<T>(&item_rc, window_adapter, my_sibling_index);

        let rendering_data = item.cached_rendering_data_offset();
        let own_screen_rect: Option<LogicalRect>;
        let recurse: Option<ComputeDirtyRegionState>;

        {
            let mut cache_ref = cache.borrow_mut();
            match rendering_data.get_entry(&mut cache_ref) {
                Some(PartialRenderingCachedData { data: cached_geom, tracker, .. }) => {
                    let rendering_dirty = tracker.as_ref().is_some_and(|tr| tr.is_dirty());

                    // Repaint when the rank among the previously known siblings changed,
                    // in either direction: two items cannot trade places in the stacking
                    // order with both comparison ranks unchanged, and since an overlap is
                    // within both items' rects, repainting the changed one(s) covers it.
                    // Only a decrease is not enough: in a permutation of three or more
                    // items a pair can flip while one member keeps its rank and the other
                    // only rises. A saturated rank (>65535 siblings) always repaints.
                    let comparison_sibling_index = {
                        let mut counters = sibling_counters.borrow_mut();
                        let idx = counters[state.depth].1;
                        counters[state.depth].1 = idx.saturating_add(1);
                        idx
                    };
                    let old_sibling_index =
                        core::mem::replace(cached_geom.sibling_index(), my_sibling_index);
                    let sibling_index_changed = my_sibling_index == u16::MAX
                        || comparison_sibling_index != old_sibling_index;
                    new_state.must_refresh_children |= sibling_index_changed;

                    let geometry_changed = !cached_geom.same_geometry(&new_geom);
                    if ItemRef::downcast_pin::<Clip>(item).is_some()
                        || ItemRef::downcast_pin::<Opacity>(item).is_some()
                    {
                        // When the opacity or the clip change, this will impact all the children, including
                        // the ones outside the element, regardless if they are themselves dirty or not.
                        new_state.must_refresh_children |= rendering_dirty || geometry_changed;

                        if rendering_dirty {
                            // Destroy the tracker as we we might not re-render this clipped item but it would stay dirty
                            *tracker = None;
                        }
                    }

                    if geometry_changed {
                        let old_transform = cached_geom.transform();
                        mark_dirty_rect(
                            dirty_region,
                            cached_geom.bounding_rect(),
                            state.old_transform_to_screen,
                            &state.clipped,
                        );
                        mark_dirty_rect(
                            dirty_region,
                            new_geom.bounding_rect(),
                            state.transform_to_screen,
                            &state.clipped,
                        );

                        new_state
                            .adjust_transforms_for_child(&new_geom.transform(), &old_transform);

                        own_screen_rect = screen_rect_for(
                            new_geom.bounding_rect(),
                            state.transform_to_screen,
                            &state.clipped,
                        );

                        *cached_geom = new_geom;
                        recurse = Some(new_state);
                    } else {
                        new_state.adjust_transforms_for_child(
                            &cached_geom.transform(),
                            &cached_geom.transform(),
                        );

                        let moved = state.must_refresh_children
                            || sibling_index_changed
                            || new_state.transform_to_screen != new_state.old_transform_to_screen;

                        if rendering_dirty {
                            mark_dirty_rect(
                                dirty_region,
                                cached_geom.bounding_rect(),
                                state.transform_to_screen,
                                &state.clipped,
                            );
                            if moved {
                                mark_dirty_rect(
                                    dirty_region,
                                    cached_geom.bounding_rect(),
                                    state.old_transform_to_screen,
                                    &state.clipped,
                                );
                            }

                            own_screen_rect = screen_rect_for(
                                cached_geom.bounding_rect(),
                                state.transform_to_screen,
                                &state.clipped,
                            );
                            recurse = Some(new_state);
                        } else {
                            if moved {
                                mark_dirty_rect(
                                    dirty_region,
                                    cached_geom.bounding_rect(),
                                    state.old_transform_to_screen,
                                    &state.clipped,
                                );
                                mark_dirty_rect(
                                    dirty_region,
                                    cached_geom.bounding_rect(),
                                    state.transform_to_screen,
                                    &state.clipped,
                                );
                            } else if let Some(tr) = &tracker {
                                tr.as_ref().register_as_dependency_to_current_binding();
                            }

                            own_screen_rect = screen_rect_for(
                                cached_geom.bounding_rect(),
                                state.transform_to_screen,
                                &state.clipped,
                            );

                            if let CachedItemBoundingBoxAndTransform::ClipItem {
                                geometry, ..
                            } = &cached_geom
                            {
                                new_state.clipped = new_state
                                    .clipped
                                    .intersection(
                                        &state
                                            .transform_to_screen
                                            .outer_transformed_rect(&geometry.cast())
                                            .cast()
                                            .union(
                                                &state
                                                    .old_transform_to_screen
                                                    .outer_transformed_rect(&geometry.cast())
                                                    .cast(),
                                            ),
                                    )
                                    .unwrap_or_default();
                                recurse = if new_state.clipped.is_empty() {
                                    None
                                } else {
                                    Some(new_state)
                                };
                            } else {
                                recurse = Some(new_state);
                            }
                        }
                    }
                }
                None => {
                    let cache_entry = PartialRenderingCachedData::new(new_geom.clone());
                    rendering_data.cache_index.set(cache_ref.insert(cache_entry));
                    rendering_data.cache_generation.set(cache_ref.generation());

                    new_state
                        .adjust_transforms_for_child(&new_geom.transform(), &new_geom.transform());

                    if let CachedItemBoundingBoxAndTransform::ClipItem { geometry, .. } = &new_geom
                    {
                        new_state.clipped = new_state
                            .clipped
                            .intersection(
                                &state
                                    .transform_to_screen
                                    .outer_transformed_rect(&geometry.cast())
                                    .cast(),
                            )
                            .unwrap_or_default();
                    }

                    mark_dirty_rect(
                        dirty_region,
                        new_geom.bounding_rect(),
                        state.transform_to_screen,
                        &state.clipped,
                    );
                    own_screen_rect = screen_rect_for(
                        new_geom.bounding_rect(),
                        state.transform_to_screen,
                        &state.clipped,
                    );

                    recurse = if new_state.clipped.is_empty() { None } else { Some(new_state) };
                }
            }
        }

        let descendants_bounds = recurse.and_then(|child_state| {
            compute_dirty_regions_recursive::<T>(
                child_component,
                child_index as isize,
                cache,
                window_adapter,
                dirty_region,
                sibling_counters,
                child_state,
            )
        });

        let subtree_bounds = union_opt_rect(own_screen_rect, descendants_bounds);

        #[cfg(feature = "occlusion-culling")]
        {
            let mut cache_ref = cache.borrow_mut();
            if let Some(entry) = rendering_data.get_entry(&mut cache_ref) {
                entry.subtree_screen_bounds = subtree_bounds;
                entry.child_transform_to_screen = new_state.transform_to_screen;
            }
        }

        aggregate = union_opt_rect(aggregate, subtree_bounds);

        VisitChildrenResult::CONTINUE
    };
    vtable::new_vref!(let mut child_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut child_visitor);
    VRc::borrow_pin(component).as_ref().visit_children_item(
        index,
        crate::item_tree::TraversalOrder::BackToFront,
        child_visitor,
    );

    aggregate
}

impl<'a, T: ItemRenderer + ItemRendererFeatures> PartialRenderer<'a, T> {
    /// Create a new PartialRenderer
    fn new(
        cache: &'a RefCell<PartialRendererCache>,
        initial_dirty_region: DirtyRegion,
        actual_renderer: T,
    ) -> Self {
        let window_adapter = actual_renderer.window().window_adapter();
        Self { cache, dirty_region: initial_dirty_region, actual_renderer, window_adapter }
    }

    /// Visit the tree of item and compute what are the dirty regions
    pub fn compute_dirty_regions(
        &mut self,
        component: &ItemTreeRc,
        origin: LogicalPoint,
        size: LogicalSize,
    ) {
        let initial_transform = euclid::Transform2D::translation(origin.x as f32, origin.y as f32);
        let sibling_counters = RefCell::new(alloc::vec::Vec::<(u16, u16)>::new());
        compute_dirty_regions_recursive::<T>(
            component,
            -1,
            self.cache,
            &self.window_adapter,
            &mut self.dirty_region,
            &sibling_counters,
            ComputeDirtyRegionState {
                transform_to_screen: initial_transform,
                old_transform_to_screen: initial_transform,
                clipped: LogicalRect::from_size(size),
                must_refresh_children: false,
                depth: 0,
            },
        );
    }

    /// Visit the tree and mark, for each item, whether it's fully hidden behind opaque content painted after it.
    /// Must run after `Self::compute_dirty_regions` for the same `component` in the same frame.
    /// Only accounts for occlusion *within* `component`; an opaque item in one `component` can't hide content in a different, earlier `component`
    /// from the same `PartialRenderingState::apply_dirty_region` call.
    #[cfg(feature = "occlusion-culling")]
    pub fn compute_occlusion(
        &mut self,
        component: &ItemTreeRc,
        origin: LogicalPoint,
        size: LogicalSize,
    ) {
        let initial_transform = euclid::Transform2D::translation(origin.x as f32, origin.y as f32);
        let mut accumulator = OccludedRegion::default();
        compute_occlusion_recursive(
            component,
            -1,
            self.cache,
            &self.dirty_region,
            &mut accumulator,
            ComputeOcclusionState {
                transform_to_screen: initial_transform,
                clipped: LogicalRect::from_size(size),
                occluder_clip: LogicalRect::from_size(size),
                may_contribute: true,
            },
        );
    }

    fn do_rendering(
        cache: &RefCell<PartialRendererCache>,
        rendering_data: &CachedRenderingData,
        item_rc: &ItemRc,
        render_fn: impl FnOnce(),
    ) {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = rendering_data.get_entry(&mut cache) {
            entry
                .tracker
                .get_or_insert_with(|| Box::pin(PropertyTracker::default()))
                .as_ref()
                .evaluate(render_fn);
        } else {
            // This item was created between the computation of the dirty region and the actual rendering.
            // Register a dependency to the geometry since this wasn't done before
            item_rc.geometry();
            render_fn();
        }
    }

    /// Move the actual renderer
    pub fn into_inner(self) -> T {
        self.actual_renderer
    }

    /// Whether an item with this bounding rect is visible in the clip and dirty region.
    fn item_is_drawn(&self, item_bounding_rect: &LogicalRect) -> bool {
        self.get_current_clip().intersection(item_bounding_rect).is_some_and(|clipped_geom| {
            let screen_geom =
                self.current_transform().outer_transformed_rect(&clipped_geom.cast()).cast();
            self.dirty_region.draw_intersects(screen_geom)
        })
    }
}

macro_rules! forward_rendering_call {
    (fn $fn:ident($Ty:ty) $(-> $Ret:ty)?) => {
        fn $fn(&mut self, obj: Pin<&$Ty>, item_rc: &ItemRc, size: LogicalSize) $(-> $Ret)? {
            let mut ret = None;
            Self::do_rendering(&self.cache, &obj.cached_rendering_data, item_rc, || {
                ret = Some(self.actual_renderer.$fn(obj, item_rc, size));
            });
            ret.unwrap_or_default()
        }
    };
}

macro_rules! forward_rendering_call2 {
    (fn $fn:ident($Ty:ty) $(-> $Ret:ty)?) => {
        fn $fn(&mut self, obj: Pin<&$Ty>, item_rc: &ItemRc, size: LogicalSize, cache: &CachedRenderingData) $(-> $Ret)? {
            let mut ret = None;
            Self::do_rendering(&self.cache, &cache, item_rc, || {
                ret = Some(self.actual_renderer.$fn(obj, item_rc, size, &cache));
            });
            ret.unwrap_or_default()
        }
    };
}

impl<T: ItemRenderer + ItemRendererFeatures> ItemRenderer for PartialRenderer<'_, T> {
    fn filter_item(
        &mut self,
        item_rc: &ItemRc,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) -> (bool, LogicalPoint, Option<LogicalSize>) {
        let item = item_rc.borrow();
        let rendering_data = item.cached_rendering_data_offset();

        // The entry is fresh: compute_dirty_regions() refreshes it every frame.
        let cached = {
            let mut cache = self.cache.borrow_mut();
            rendering_data.get_entry(&mut cache).map(|e| {
                #[cfg(feature = "occlusion-culling")]
                let occluded = e.occluded;
                #[cfg(not(feature = "occlusion-culling"))]
                let occluded = false;
                let draw = !occluded && self.item_is_drawn(e.data.bounding_rect());
                let offset = match &e.data {
                    CachedItemBoundingBoxAndTransform::RegularItem { offset, .. } => Some(*offset),
                    _ => None,
                };
                (draw, offset)
            })
        };

        // Items that are not drawn only need their origin; this skips e.g. shaping off-screen text.
        if let Some((false, Some(offset))) = cached {
            return (false, offset.to_point(), None);
        }

        // Query untracked, as the bounding rect calculation already registers a dependency on the geometry.
        let item_geometry = crate::properties::evaluate_no_tracking(|| item_rc.geometry());
        let draw = cached.map(|(draw, _)| draw).unwrap_or_else(|| {
            // The item was created between the computation of the dirty region and the
            // actual rendering.
            self.item_is_drawn(&item_rc.bounding_rect(&item_geometry, window_adapter))
        });

        (draw, item_geometry.origin, Some(item_geometry.size))
    }

    forward_rendering_call2!(fn draw_rectangle(dyn RenderRectangle));
    forward_rendering_call2!(fn draw_border_rectangle(dyn RenderBorderRectangle));
    forward_rendering_call2!(fn draw_window_background(dyn RenderRectangle));
    forward_rendering_call2!(fn draw_image(dyn RenderImage));
    forward_rendering_call2!(fn draw_text(dyn RenderText));
    forward_rendering_call!(fn draw_text_input(TextInput));
    #[cfg(feature = "path")]
    forward_rendering_call!(fn draw_path(Path));
    forward_rendering_call!(fn draw_box_shadow(BoxShadow));

    forward_rendering_call!(fn visit_clip(Clip) -> RenderingResult);
    forward_rendering_call!(fn visit_opacity(Opacity) -> RenderingResult);
    forward_rendering_call!(fn visit_layer(Layer) -> RenderingResult);

    fn combine_clip(&mut self, rect: LogicalRect, radius: LogicalBorderRadius) -> bool {
        self.actual_renderer.combine_clip(rect, radius)
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.actual_renderer.get_current_clip()
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.actual_renderer.translate(distance)
    }
    fn current_transform(&self) -> ItemTransform {
        self.actual_renderer.current_transform()
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        self.actual_renderer.rotate(angle_in_degrees)
    }

    fn scale(&mut self, x_factor: f32, y_factor: f32) {
        self.actual_renderer.scale(x_factor, y_factor)
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.actual_renderer.apply_opacity(opacity)
    }

    fn global_alpha_transparent(&self) -> bool {
        self.actual_renderer.global_alpha_transparent()
    }

    fn save_state(&mut self) {
        self.actual_renderer.save_state()
    }

    fn restore_state(&mut self) {
        self.actual_renderer.restore_state()
    }

    fn scale_factor(&self) -> ScaleFactor {
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

/// This struct holds the state of the partial renderer between different frames, in particular the cache of the bounding rect
/// of each item. This permits a more fine-grained computation of the region that needs to be repainted.
#[derive(Default)]
pub struct PartialRenderingState {
    partial_cache: RefCell<PartialRendererCache>,
    /// This is the area which we are going to redraw in the next frame, no matter if the items are dirty or not
    force_dirty: RefCell<DirtyRegion>,
    /// Force a redraw in the next frame, no matter what's dirty. Use only as a last resort.
    force_screen_refresh: Cell<bool>,
}

impl PartialRenderingState {
    /// Creates a partial renderer that's initialized with the partial rendering caches maintained in this state structure.
    /// Call [`Self::apply_dirty_region`] after this function to compute the correct partial rendering region.
    pub fn create_partial_renderer<T: ItemRenderer + ItemRendererFeatures>(
        &self,
        renderer: T,
    ) -> PartialRenderer<'_, T> {
        PartialRenderer::new(&self.partial_cache, self.force_dirty.take(), renderer)
    }

    /// Compute the correct partial rendering region based on the components to be drawn, the bounding rectangles of
    /// changes items within, and the current repaint buffer type. Returns the computed dirty region just for this frame.
    /// The provided buffer_dirty_region specifies which area of the buffer is known to *additionally* require repainting,
    /// where `None` means that buffer is not known to be dirty beyond what applies to this frame (reused buffer).
    pub fn apply_dirty_region<T: ItemRenderer + ItemRendererFeatures>(
        &self,
        partial_renderer: &mut PartialRenderer<'_, T>,
        components: &[(ItemTreeWeak, LogicalPoint)],
        logical_window_size: LogicalSize,
        dirty_region_of_existing_buffer: Option<DirtyRegion>,
    ) -> DirtyRegion {
        for (component, origin) in components {
            if let Some(component) = crate::item_tree::ItemTreeWeak::upgrade(component) {
                partial_renderer.compute_dirty_regions(&component, *origin, logical_window_size);
                #[cfg(feature = "occlusion-culling")]
                partial_renderer.compute_occlusion(&component, *origin, logical_window_size);
            }
        }

        let screen_region = LogicalRect::from_size(logical_window_size);

        if self.force_screen_refresh.take() {
            partial_renderer.dirty_region = screen_region.into();
        }

        let region_to_repaint = partial_renderer.dirty_region.clone();

        partial_renderer.dirty_region = match dirty_region_of_existing_buffer {
            Some(dirty_region) => partial_renderer.dirty_region.union(&dirty_region),
            None => partial_renderer.dirty_region.clone(),
        }
        .intersection(screen_region);

        region_to_repaint
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

    /// Clears the partial rendering cache. Use this for example when the entire underlying window surface changes.
    pub fn clear_cache(&self) {
        self.partial_cache.borrow_mut().clear();
    }

    /// Force re-rendering of the entire window region the next time a partial renderer is created.
    pub fn force_screen_refresh(&self) {
        self.force_screen_refresh.set(true);
    }
}

#[test]
fn dirty_region_no_intersection() {
    let mut region = DirtyRegion::default();
    region.add_rect(LogicalRect::new(LogicalPoint::new(10., 10.), LogicalSize::new(16., 16.)));
    region.add_rect(LogicalRect::new(LogicalPoint::new(100., 100.), LogicalSize::new(16., 16.)));
    region.add_rect(LogicalRect::new(LogicalPoint::new(200., 100.), LogicalSize::new(16., 16.)));
    let i = region
        .intersection(LogicalRect::new(LogicalPoint::new(50., 50.), LogicalSize::new(10., 10.)));
    assert_eq!(i.iter().count(), 0);
}
