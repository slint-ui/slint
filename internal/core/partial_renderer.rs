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
use crate::item_rendering::{
    ItemRenderer, ItemRendererFeatures, RenderBorderRectangle, RenderImage, RenderRectangle,
    RenderText,
};
use crate::item_tree::{
    ItemTreeRc, ItemTreeWeak, ItemVisitor, ItemVisitorVTable, VisitChildrenResult,
};
#[cfg(feature = "path")]
use crate::items::Path;
use crate::items::{
    BackdropBlur, BoxShadow, Clip, ItemRc, ItemRef, Layer, Opacity, RenderingResult, TextInput,
};
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

    /// The item's rank among its z-ordered siblings when it was last visited.
    fn sibling_index(&self) -> u16 {
        match self {
            CachedItemBoundingBoxAndTransform::RegularItem { sibling_index, .. }
            | CachedItemBoundingBoxAndTransform::ItemWithTransform { sibling_index, .. }
            | CachedItemBoundingBoxAndTransform::ClipItem { sibling_index, .. } => *sibling_index,
        }
    }

    fn set_sibling_index(&mut self, index: u16) {
        match self {
            CachedItemBoundingBoxAndTransform::RegularItem { sibling_index, .. }
            | CachedItemBoundingBoxAndTransform::ItemWithTransform { sibling_index, .. }
            | CachedItemBoundingBoxAndTransform::ClipItem { sibling_index, .. } => {
                *sibling_index = index
            }
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
}
impl PartialRenderingCachedData {
    fn new(data: CachedItemBoundingBoxAndTransform) -> Self {
        Self {
            data,
            tracker: None,
        }
    }
}

/// The cache that needs to be held by the Window for the partial rendering
struct PartialRendererCache {
    slab: slab::Slab<PartialRenderingCachedData>,
    generation: usize,
    /// Per ItemTree (keyed by its instance pointer), the union of the clipped screen-space
    /// regions of the tree's own items, as of the tree's last visit by
    /// [`PartialRenderer::compute_dirty_regions`]. Nested trees have their own entry.
    /// When a tree is destroyed, this is the region that needs to be repainted
    /// (see [`PartialRenderingState::free_graphics_resources`]).
    tree_screen_rects: alloc::collections::BTreeMap<usize, LogicalRect>,
}

impl Default for PartialRendererCache {
    fn default() -> Self {
        Self { slab: Default::default(), generation: 1, tree_screen_rects: Default::default() }
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
        self.tree_screen_rects.clear();
    }
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
    /// What each `BackdropBlur` item reads, including the parts outside of its clip,
    /// see [`Self::expand_to_backdrop_blurs`].
    backdrop_blurs: alloc::vec::Vec<LogicalRect>,
}

#[derive(Clone, Copy)]
struct ComputeDirtyRegionState {
    transform_to_screen: ItemTransform,
    old_transform_to_screen: ItemTransform,
    clipped: LogicalRect,
    must_refresh_children: bool,
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

/// The rank of an item among its z-ordered siblings, counted per tree depth.
#[derive(Default, Clone, Copy)]
struct SiblingRankCounters {
    /// Rank among all the siblings visited at this depth, which is the rank stored in the
    /// item's cache entry.
    visited: u16,
    /// Rank among only the siblings that already had a cache entry, which is what a stored
    /// rank is compared against.
    /// Leaving new items out of this count keeps an appearing sibling from shifting the ranks
    /// of existing ones; their overlap with it is already covered by its own dirty rect.
    /// Two existing items can never trade places without at least one of these ranks changing.
    cached: u16,
}

/// The [`SiblingRankCounters`] of one walk, indexed by tree depth.
#[derive(Default)]
struct SiblingRanks(alloc::vec::Vec<SiblingRankCounters>);

impl SiblingRanks {
    /// The rank of the next item at `depth`, which also restarts the ranks of that item's children.
    fn next_visited(&mut self, depth: usize) -> u16 {
        if self.0.len() <= depth + 1 {
            self.0.resize(depth + 2, SiblingRankCounters::default());
        }
        self.0[depth + 1] = SiblingRankCounters::default();
        let rank = self.0[depth].visited;
        self.0[depth].visited = rank.saturating_add(1);
        rank
    }

    /// The rank of the next item at `depth` that already had a cache entry.
    fn next_cached(&mut self, depth: usize) -> u16 {
        let rank = self.0[depth].cached;
        self.0[depth].cached = rank.saturating_add(1);
        rank
    }
}

/// Transform `rect` by `transform` and intersect it with `clip_rect`.
/// `None` if `rect` is empty, non-finite, or entirely clipped away.
fn transformed_and_clipped(
    rect: &LogicalRect,
    transform: ItemTransform,
    clip_rect: &LogicalRect,
) -> Option<LogicalRect> {
    screen_rect(rect, transform)?.intersection(clip_rect)
}

/// Map `rect` (relative to its parent) to screen space through `transform`.
fn screen_rect(rect: &LogicalRect, transform: ItemTransform) -> Option<LogicalRect> {
    #[cfg(not(slint_int_coord))]
    if !rect.origin.is_finite() {
        // Account for NaN
        return None;
    }
    if rect.is_empty() {
        return None;
    }
    let rect = rect.cast();
    // Fast path for the common case of a pure translation, so that the per-item,
    // per-frame calls of `compute_dirty_regions` skip the four-corner transform.
    let transformed =
        if (transform.m11, transform.m12, transform.m21, transform.m22) == (1., 0., 0., 1.) {
            rect.translate(euclid::vec2(transform.m31, transform.m32))
        } else {
            transform.outer_transformed_rect(&rect)
        };
    Some(transformed.cast())
}

/// Narrow `clipped` to the screen region a clipping item's `geometry` lets through, at both its
/// current and its previous position, so an item that moved keeps painting at either.
fn clipped_by(
    clipped: LogicalRect,
    geometry: &LogicalRect,
    transform: ItemTransform,
    old_transform: ItemTransform,
) -> LogicalRect {
    let screen_rect = transform
        .outer_transformed_rect(&geometry.cast())
        .cast()
        .union(&old_transform.outer_transformed_rect(&geometry.cast()).cast());
    clipped.intersection(&screen_rect).unwrap_or_default()
}

/// What one [`PartialRenderer::compute_dirty_regions`] pass carries across the recursion.
struct DirtyRegionWalk<'a> {
    cache: &'a RefCell<PartialRendererCache>,
    window_adapter: &'a Rc<dyn WindowAdapter>,
    dirty_region: &'a mut DirtyRegion,
    backdrop_blurs: &'a mut alloc::vec::Vec<LogicalRect>,
    sibling_ranks: SiblingRanks,
}

impl DirtyRegionWalk<'_> {
    /// Visit `index`'s children depth-first, marking everything that changed since the last
    /// frame as dirty.
    fn visit_children<T: ItemRendererFeatures>(
        &mut self,
        component: &ItemTreeRc,
        index: isize,
        state: ComputeDirtyRegionState,
    ) {
        let mut child_visitor = |child_component: &ItemTreeRc,
                                 child_index: u32,
                                 item: Pin<ItemRef>|
         -> VisitChildrenResult {
            let item_rc = ItemRc::new(child_component.clone(), child_index);

            if let Some(child_state) = self.update_item::<T>(item, &item_rc, state) {
                self.visit_children::<T>(child_component, child_index as isize, child_state);
            }

            VisitChildrenResult::CONTINUE
        };
        vtable::new_vref!(let mut child_visitor : VRefMut<ItemVisitorVTable> for ItemVisitor = &mut child_visitor);
        VRc::borrow_pin(component).as_ref().visit_children_item(
            index,
            crate::item_tree::TraversalOrder::BackToFront,
            child_visitor,
        );
    }

    /// Mark `rect` (in the coordinate system `transform` maps to the screen, clipped by
    /// `clip_rect`) as needing to be repainted this frame.
    fn mark_dirty(
        &mut self,
        rect: &LogicalRect,
        transform: ItemTransform,
        clip_rect: &LogicalRect,
    ) {
        if let Some(rect) = transformed_and_clipped(rect, transform, clip_rect) {
            self.dirty_region.add_rect(rect);
        }
    }

    /// Refresh `item`'s cache entry and mark what changed about it as dirty.
    /// Returns the state to visit its children with, or `None` if they are all clipped away.
    fn update_item<T: ItemRendererFeatures>(
        &mut self,
        item: Pin<ItemRef>,
        item_rc: &ItemRc,
        state: ComputeDirtyRegionState,
    ) -> Option<ComputeDirtyRegionState> {
        let sibling_index = self.sibling_ranks.next_visited(state.depth);
        let new_geom = CachedItemBoundingBoxAndTransform::new::<T>(
            item_rc,
            self.window_adapter,
            sibling_index,
        );

        let mut new_state = state;
        new_state.depth = state.depth + 1;

        // The region the item covers on screen at its current position. It is merged into the
        // owning tree's entry in `tree_screen_rects` so that destroying the tree can repaint
        // that region (see `PartialRenderingState::free_graphics_resources`), and reused below
        // as the item's current-position dirty rect wherever `mark_dirty` would recompute the
        // same transform and clip.
        let new_screen_rect_or_empty = transformed_and_clipped(
            new_geom.bounding_rect(),
            state.transform_to_screen,
            &state.clipped,
        )
        .unwrap_or_default();

        let rendering_data = item.cached_rendering_data_offset();
        let mut cache_ref = self.cache.borrow_mut();

        let tree_key =
            vtable::VRef::as_ptr(vtable::VRc::borrow(item_rc.item_tree())).as_ptr() as usize;
        if item_rc.index() == 0
            && let Some(acc) = cache_ref.tree_screen_rects.get_mut(&tree_key)
        {
            // Entering the tree: rebuild its screen region from this pass's visits.
            *acc = LogicalRect::default();
        }
        if !new_screen_rect_or_empty.is_empty() {
            let acc = cache_ref.tree_screen_rects.entry(tree_key).or_default();
            *acc = acc.union(&new_screen_rect_or_empty);
        }
        if T::SUPPORTS_BACKDROP_BLUR
            && !new_screen_rect_or_empty.is_empty()
            && ItemRef::downcast_pin::<BackdropBlur>(item).is_some()
            && let Some(sampled) = screen_rect(new_geom.bounding_rect(), state.transform_to_screen)
        {
            self.backdrop_blurs.push(sampled);
        }

        match rendering_data.get_entry(&mut cache_ref) {
            Some(entry) => self.update_cached_item(
                item,
                entry,
                new_geom,
                state,
                new_state,
                new_screen_rect_or_empty,
            ),
            None => {
                let cache_entry = PartialRenderingCachedData::new(new_geom.clone());
                rendering_data.cache_index.set(cache_ref.insert(cache_entry));
                rendering_data.cache_generation.set(cache_ref.generation());

                new_state.adjust_transforms_for_child(&new_geom.transform(), &new_geom.transform());

                if let CachedItemBoundingBoxAndTransform::ClipItem { geometry, .. } = &new_geom {
                    new_state.clipped = clipped_by(
                        new_state.clipped,
                        geometry,
                        state.transform_to_screen,
                        state.transform_to_screen,
                    );
                }

                self.dirty_region.add_rect(new_screen_rect_or_empty);

                (!new_state.clipped.is_empty()).then_some(new_state)
            }
        }
    }

    /// Refresh the cache entry of an item that the previous frame already visited.
    fn update_cached_item(
        &mut self,
        item: Pin<ItemRef>,
        entry: &mut PartialRenderingCachedData,
        new_geom: CachedItemBoundingBoxAndTransform,
        state: ComputeDirtyRegionState,
        mut new_state: ComputeDirtyRegionState,
        new_screen_rect_or_empty: LogicalRect,
    ) -> Option<ComputeDirtyRegionState> {
        let PartialRenderingCachedData { data: cached_geom, tracker, .. } = entry;
        let rendering_dirty = tracker.as_ref().is_some_and(|tr| tr.is_dirty());

        // Repaint when the rank among the previously known siblings changed, in either
        // direction: in a permutation of three or more items, a pair can flip while one member
        // keeps its rank and the other only rises.
        // A saturated rank (more than `u16::MAX` siblings) always repaints.
        let comparison_sibling_index = self.sibling_ranks.next_cached(state.depth);
        let sibling_index = new_geom.sibling_index();
        let sibling_index_changed =
            sibling_index == u16::MAX || comparison_sibling_index != cached_geom.sibling_index();
        cached_geom.set_sibling_index(sibling_index);
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
            self.mark_dirty(
                cached_geom.bounding_rect(),
                state.old_transform_to_screen,
                &state.clipped,
            );
            self.dirty_region.add_rect(new_screen_rect_or_empty);

            new_state.adjust_transforms_for_child(&new_geom.transform(), &old_transform);
            *cached_geom = new_geom;
            return Some(new_state);
        }

        new_state.adjust_transforms_for_child(&cached_geom.transform(), &cached_geom.transform());

        let moved = state.must_refresh_children
            || sibling_index_changed
            || new_state.transform_to_screen != new_state.old_transform_to_screen;

        if rendering_dirty {
            self.dirty_region.add_rect(new_screen_rect_or_empty);
            if moved {
                self.mark_dirty(
                    cached_geom.bounding_rect(),
                    state.old_transform_to_screen,
                    &state.clipped,
                );
            }
            return Some(new_state);
        }

        if moved {
            self.mark_dirty(
                cached_geom.bounding_rect(),
                state.old_transform_to_screen,
                &state.clipped,
            );
            self.dirty_region.add_rect(new_screen_rect_or_empty);
        } else if let Some(tr) = tracker {
            // Nothing about the item changed, so it isn't repainted.
            // Keep its rendering tracker registered, so a later change to the properties it
            // covers triggers a redraw.
            tr.as_ref().register_as_dependency_to_current_binding();
        }

        if let CachedItemBoundingBoxAndTransform::ClipItem { geometry, .. } = &cached_geom {
            new_state.clipped = clipped_by(
                new_state.clipped,
                geometry,
                state.transform_to_screen,
                state.old_transform_to_screen,
            );
            return (!new_state.clipped.is_empty()).then_some(new_state);
        }

        Some(new_state)
    }
}

impl<'a, T: ItemRenderer + ItemRendererFeatures> PartialRenderer<'a, T> {
    /// Create a new PartialRenderer
    fn new(
        cache: &'a RefCell<PartialRendererCache>,
        initial_dirty_region: DirtyRegion,
        actual_renderer: T,
    ) -> Self {
        let window_adapter = actual_renderer.window().window_adapter();
        Self {
            cache,
            dirty_region: initial_dirty_region,
            actual_renderer,
            window_adapter,
            backdrop_blurs: Default::default(),
        }
    }

    /// Visit the tree of item and compute what are the dirty regions
    pub fn compute_dirty_regions(
        &mut self,
        component: &ItemTreeRc,
        origin: LogicalPoint,
        size: LogicalSize,
    ) {
        let initial_transform = euclid::Transform2D::translation(origin.x as f32, origin.y as f32);
        DirtyRegionWalk {
            cache: self.cache,
            window_adapter: &self.window_adapter,
            dirty_region: &mut self.dirty_region,
            backdrop_blurs: &mut self.backdrop_blurs,
            sibling_ranks: SiblingRanks::default(),
        }
        .visit_children::<T>(
            component,
            -1,
            ComputeDirtyRegionState {
                transform_to_screen: initial_transform,
                old_transform_to_screen: initial_transform,
                clipped: LogicalRect::from_size(size),
                must_refresh_children: false,
                depth: 0,
            },
        );
    }

    /// Adds everything that each backdrop blur reading from `region` reads,
    /// because a blur changes as a whole when anything beneath it changes,
    /// and it must read the same freshly drawn pixels as in a full redraw.
    fn expand_to_backdrop_blurs(&self, mut region: DirtyRegion) -> DirtyRegion {
        let mut blurs = self.backdrop_blurs.clone();
        while let Some(index) = blurs.iter().position(|sampled| region.draw_intersects(*sampled)) {
            region.add_rect(blurs.swap_remove(index));
        }
        region
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
                let draw = self.item_is_drawn(e.data.bounding_rect());
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
    forward_rendering_call!(fn draw_backdrop_blur(BackdropBlur));

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
        PartialRenderer::new(&self.partial_cache, DirtyRegion::default(), renderer)
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
            }
        }

        let screen_region = LogicalRect::from_size(logical_window_size);

        // Collect the regions accumulated in `force_dirty` (destroyed item trees,
        // `mark_dirty_region` calls) only now: repeater instances are dropped by
        // `ensure_tree_instantiated` inside `draw_contents`, after the partial renderer
        // for the frame was already created.
        partial_renderer.dirty_region =
            partial_renderer.dirty_region.union(&self.force_dirty.take());

        if self.force_screen_refresh.take() {
            partial_renderer.dirty_region = screen_region.into();
        }

        let region_to_repaint =
            partial_renderer.expand_to_backdrop_blurs(partial_renderer.dirty_region.clone());

        partial_renderer.dirty_region = match dirty_region_of_existing_buffer {
            Some(dirty_region) => {
                partial_renderer.expand_to_backdrop_blurs(region_to_repaint.union(&dirty_region))
            }
            None => region_to_repaint.clone(),
        }
        .intersection(screen_region);

        region_to_repaint
    }

    /// Add the specified region to the list of regions to include in the next rendering.
    pub fn mark_dirty_region(&self, region: DirtyRegion) {
        self.force_dirty.replace_with(|r| r.union(&region));
    }

    /// Call this from your renderer's `free_graphics_resources` function to ensure that the cached item geometries
    /// are cleared for the destroyed items in the item tree, and that the screen region the tree
    /// covered is repainted in the next frame.
    pub fn free_graphics_resources(
        &self,
        component: crate::item_tree::ItemTreeRef,
        items: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>,
    ) {
        let mut cache = self.partial_cache.borrow_mut();

        let tree_key = vtable::VRef::as_ptr(component).as_ptr() as usize;
        if let Some(rect) = cache.tree_screen_rects.remove(&tree_key) {
            self.force_dirty.borrow_mut().add_rect(rect);
        }

        for item in items {
            item.cached_rendering_data_offset().release(&mut cache);
        }
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
fn dirty_region_ignores_empty_rects() {
    // `compute_dirty_regions` feeds the empty rect of an invisible item into
    // `add_rect`; it must not drag the region towards the empty rect's origin.
    let mut region = DirtyRegion::default();
    region.add_rect(LogicalRect::default());
    assert_eq!(region.iter().count(), 0);
    let real = LogicalRect::new(LogicalPoint::new(10., 10.), LogicalSize::new(16., 16.));
    region.add_rect(real);
    region.add_rect(LogicalRect::default());
    assert_eq!(region.bounding_rect(), real);
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
