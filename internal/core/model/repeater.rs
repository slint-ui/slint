// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the [`Repeater`] and [`Conditional`] types that are
//! used by generated code to instantiate items from a model using the `for`
//! syntax, and [`RepeatedItemTree`] which is the trait implemented by the
//! generated repeated components.
//!
//! The [`RepeaterInstanceOps`] trait abstracts over instance storage so the
//! update algorithm can be shared between Rust and C++ (via FFI).

use super::model_peer::{ModelChangeListener, ModelChangeListenerContainer};
use super::{Model, ModelExt, ModelRc};
use crate::item_tree::{ItemTreeVTable, TraversalOrder};
use crate::layout::Orientation;
use crate::lengths::{LogicalLength, RectLengths};
use crate::{Coord, Property};
use alloc::vec::Vec;
use core::cell::RefCell;
use core::pin::Pin;
#[allow(unused)]
use euclid::num::Floor;
use pin_project::pin_project;

type ItemTreeRc<C> = vtable::VRc<crate::item_tree::ItemTreeVTable, C>;

/// Represents the relationship between model row and instance collection index
/// of an element
/// This is required for components like the listview. The listview does not instantiate all
/// items at once but instantiates only the visible once. If the current item is in the center
/// of the screen we have in the instance list a few items before the current item and a few after
/// `instance_index` indicates the position of the current item in this instance list
/// while `row` is the index of the current item in the complete model
#[derive(Default, Clone, Debug, PartialEq)]
#[repr(C)]
pub struct ItemIndexRelationShip {
    /// The position of the item in the model
    row: usize,
    /// The index of the item in the instances collection
    instance_index: usize,
}

impl ItemIndexRelationShip {
    /// The row of the first instance
    fn first_row(&self) -> usize {
        self.row - self.instance_index
    }

    /// get the instance index from the model index `row`
    fn get_instance_index(&self, row: usize) -> usize {
        self.instance_index.wrapping_add(row.wrapping_sub(self.row))
    }

    fn get_instance_index_opt(&self, row: usize) -> Option<usize> {
        row.checked_sub(self.row).map_or_else(
            || self.instance_index.checked_add(row).and_then(|res| res.checked_sub(self.row)),
            |res| self.instance_index.checked_add(res),
        )
    }
}

/// ItemTree that can be instantiated by a repeater.
pub trait RepeatedItemTree:
    crate::item_tree::ItemTree + vtable::HasStaticVTable<ItemTreeVTable> + 'static
{
    /// The data corresponding to the model
    type Data: Default + 'static;

    /// Update this ItemTree at the given index and the given data
    fn update(&self, index: usize, data: Self::Data);

    /// Called once after the ItemTree has been instantiated and update()
    /// was called once.
    fn init(&self) {}

    /// Layout this item in the listview
    ///
    /// offset_y is the `y` position where this item should be placed.
    /// it should be updated to be to the y position of the next item.
    ///
    /// Returns the minimum item width which will be used to compute the listview's content width
    fn listview_layout(self: Pin<&Self>, _offset_y: &mut LogicalLength) -> LogicalLength {
        LogicalLength::default()
    }

    /// Returns what's needed to perform the layout if this ItemTree is in a layout
    /// In case of repeated Rows, the index of a child item is set
    fn layout_item_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _child_index: Option<usize>,
    ) -> crate::layout::LayoutItemInfo {
        crate::layout::LayoutItemInfo::default()
    }

    /// Vertical layout info measured at the given cross-axis (container) width.
    /// A box layout calls this so a height-for-width instance wraps to the
    /// real width. The default ignores the width (non-height-for-width cells);
    /// the generated code overrides it for height-for-width instances.
    fn layout_item_info_at_cross_width(
        self: Pin<&Self>,
        _cross_width: f32,
    ) -> crate::layout::LayoutItemInfo {
        self.layout_item_info(Orientation::Vertical, None)
    }

    /// Returns what's needed to perform a flexbox layout if this ItemTree is in a FlexboxLayout.
    /// Includes flex-specific properties (layout-order).
    fn flexbox_layout_item_info(
        self: Pin<&Self>,
        orientation: Orientation,
        child_index: Option<usize>,
    ) -> crate::layout::FlexboxLayoutItemInfo {
        self.layout_item_info(orientation, child_index).into()
    }

    /// Vertical flexbox info measured at the given cross-axis (container) width.
    /// A column FlexboxLayout calls this so a height-for-width instance wraps to
    /// the real width. The default ignores the width (non-height-for-width
    /// cells); the generated code overrides it for height-for-width instances.
    fn flexbox_layout_item_info_at_cross_width(
        self: Pin<&Self>,
        _cross_width: f32,
    ) -> crate::layout::FlexboxLayoutItemInfo {
        self.flexbox_layout_item_info(Orientation::Vertical, None)
    }

    /// Fills in the grid layout input data for this ItemTree if it is in a grid layout.
    /// This will be a single GridLayoutInputData if the repeated item is a single cell,
    /// or multiple GridLayoutInputData if the repeated item is a full Row.
    /// The slice must have the exact size required (known at compile time).
    fn grid_layout_input_data(
        self: Pin<&Self>,
        _new_row: bool,
        _result: &mut [crate::layout::GridLayoutInputData],
    ) {
        crate::debug_log!(
            "Internal error in Slint: RepeatedItemTree::grid_layout_input_data() not implemented for {}",
            core::any::type_name::<Self>()
        );
        // the actual implementation is in the code generated by generate_repeated_component()
    }

    /// The z value used to sort this instance among the other instances of the repeater,
    /// when the repeated element has a dynamic z binding
    fn z_order(self: Pin<&Self>) -> Option<f32> {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RepeatedInstanceState {
    /// The item is in a clean state
    Clean,
    /// The model data is stale and needs to be refreshed
    Dirty,
}
struct RepeaterInner<C: RepeatedItemTree> {
    instances: Vec<(RepeatedInstanceState, Option<ItemTreeRc<C>>)>,
    /// ListView-specific layout state (offset, cached heights, scroll position).
    layout_state: RepeaterLayoutState,
}

impl<C: RepeatedItemTree> RepeaterInner<C> {
    fn current_row(&self) -> usize {
        self.layout_state.item_index.row
    }

    /// Set the row to which layout state should point
    fn set_current_row(&mut self, row: usize) {
        if self.instances.is_empty() {
            self.layout_state.item_index.row = row;
            return;
        }
        if row == self.layout_state.item_index.row {
            return;
        } else if row > self.layout_state.item_index.row {
            let new_instance_index = self.layout_state.item_index.instance_index
                + (row - self.layout_state.item_index.row);

            let number_new = (new_instance_index + 1).saturating_sub(self.instances.len());
            if number_new > 0 {
                // Append dirty elements
                self.instances.splice(
                    self.instances.len()..self.instances.len(),
                    core::iter::repeat_n((RepeatedInstanceState::Dirty, None), number_new),
                );
            }

            self.layout_state.item_index.instance_index = new_instance_index;
        } else {
            let diff = self.layout_state.item_index.row - row;
            if diff <= self.layout_state.item_index.instance_index {
                // The instances already exist so we don't have to add any new
                self.layout_state.item_index.instance_index -= diff;
            } else {
                // Prepend dirty items
                self.instances.splice(
                    0..0,
                    core::iter::repeat_n(
                        (RepeatedInstanceState::Dirty, None),
                        diff - self.layout_state.item_index.instance_index,
                    ),
                );
                self.layout_state.item_index.instance_index = 0;
            }
        }

        self.layout_state.item_index.row = row;
    }
}

impl<C: RepeatedItemTree> Default for RepeaterInner<C> {
    fn default() -> Self {
        RepeaterInner { instances: Default::default(), layout_state: Default::default() }
    }
}

/// Persistent layout state for a ListView repeater.
#[derive(Default, Clone, Debug)]
#[repr(C)]
pub struct RepeaterLayoutState {
    /// The relation between the model row index and the instance collection index
    pub item_index: ItemIndexRelationShip,
    /// The average visible item height (cached between frames).
    pub cached_item_height: Coord,
    /// The content_y value from the previous layout pass.
    /// It is used to detect if we are scrolling up or down
    pub previous_content_y: Coord,
    /// The y position of the item at `item_index`.
    pub anchor_y: Coord,
}

/// Abstraction over a repeater's instance collection so the same algorithm
/// works for both native Rust repeaters and C++ repeaters via FFI.
trait RepeaterInstanceOps {
    /// Number of currently instantiated items.
    fn len(&self) -> usize;

    /// Remove all instances.
    fn clear(&mut self);

    /// Append `count` empty, dirty slots.
    fn push(&mut self, count: usize);

    /// Insert `count` empty, dirty slots before the first instance.
    fn prepend(&mut self, count: usize);

    /// Remove the first `count` instances.
    fn remove_first(&mut self, count: usize);

    /// Keep only the first `len` instances.
    fn truncate(&mut self, len: usize);

    /// If dirty, ensure the instance is created, initialized, and updated
    /// for `row`. Returns `true` if freshly created.
    fn ensure_updated(&mut self, instance_idx: usize, row: usize) -> bool;

    /// Height of the instance, or `None` if not yet created.
    fn height(&self, instance_idx: usize) -> Option<Coord>;

    /// Call `listview_layout` on the instance.
    /// Advances `*y` to the next item position. Returns item width.
    fn listview_layout(&self, instance_idx: usize, y: &mut Coord) -> Coord;
}

/// More rows than this is presumably a bug (e.g. a division by zero) and would run out of memory
const MAX_EAGER_INSTANCE_COUNT: usize = 1 << 20;

/// Update all instances in the repeater, creating any that are missing.
fn update_all_instances(ops: &mut impl RepeaterInstanceOps, offset: usize, count: usize) {
    let count = if count > MAX_EAGER_INSTANCE_COUNT {
        crate::debug_log!("A repeater's model has {count} rows: too many to instantiate");
        0
    } else {
        count
    };
    let cur = ops.len();
    if count > cur {
        ops.push(count - cur);
    } else if count < cur {
        ops.truncate(count);
    }
    for instance_index in 0..count {
        ops.ensure_updated(instance_index, instance_index + offset);
    }
}

/// Access to the ListView content properties.
///
/// `update_visible_instances` reads and writes the content geometry at
/// several points; this trait abstracts whether the storage is a
/// strongly-typed `Property<LogicalLength>` (rust and C++ generated code,
/// see `TypedListViewProps`) or another backing such as the interpreter's
/// `Property<Value>`, or a native-item property accessed through rtti.
pub trait ListViewProperties {
    fn content_y_get(&self) -> LogicalLength;
    /// Read `content-y` without evaluating a binding on it
    /// (see [`Property::get_internal`]).
    fn content_y_get_internal(&self) -> LogicalLength;
    fn content_y_set(&self, value: LogicalLength);
    fn content_y_has_binding(&self) -> bool;
    /// True when the ListView computes `content-height` from the rows; false
    /// when the user explicitly sets it, so it doesn't track the rows and the
    /// past-the-end seek heuristic must not rely on it.
    fn computes_content_height(&self) -> bool;
    /// Set `content-width`. A no-op when the user explicitly sets it.
    fn content_width_set(&self, value: LogicalLength);
    /// Set `content-height`. A no-op when [`Self::computes_content_height`] is false.
    fn content_height_set(&self, value: LogicalLength);
    /// Register the content properties as dependencies of the current
    /// binding evaluation without reading them.
    fn register_as_dependencies(&self);
}

struct TypedListViewProps<'a> {
    /// The content width if it was fixed on the flickable, otherwise None and it must be calculated
    /// from the layout widths
    content_width: Option<Pin<&'a Property<LogicalLength>>>,
    /// The content height if it was fixed on the flickable, otherwise None and it must be calculated
    /// from the layout heights
    content_height: Option<Pin<&'a Property<LogicalLength>>>,
    content_y: Pin<&'a Property<LogicalLength>>,
}

impl ListViewProperties for TypedListViewProps<'_> {
    fn content_y_get(&self) -> LogicalLength {
        self.content_y.get()
    }
    fn content_y_get_internal(&self) -> LogicalLength {
        self.content_y.get_internal()
    }
    fn content_y_set(&self, value: LogicalLength) {
        self.content_y.set(value);
    }
    fn content_y_has_binding(&self) -> bool {
        self.content_y.has_binding()
    }
    fn computes_content_height(&self) -> bool {
        self.content_height.is_some()
    }
    fn content_width_set(&self, value: LogicalLength) {
        if let Some(content_width) = self.content_width {
            content_width.set(value);
        }
    }
    fn content_height_set(&self, value: LogicalLength) {
        if let Some(content_height) = self.content_height {
            content_height.set(value);
        }
    }
    fn register_as_dependencies(&self) {
        if let Some(content_width) = self.content_width {
            content_width.register_as_dependency();
        }
        if let Some(content_height) = self.content_height {
            content_height.register_as_dependency();
        }
        self.content_y.register_as_dependency();
    }
}

/// Update only the instances visible in the ListView viewport.
///
/// This is the core virtualization algorithm: it estimates which model rows
/// are visible, instantiates/updates those, lays them out, and cleans up
/// off-screen instances. Returns whether any instance was created.
fn update_visible_instances(
    ops: &mut impl RepeaterInstanceOps,
    state: &mut RepeaterLayoutState,
    row_count: usize,
    props: &dyn ListViewProperties,
    listview_width: LogicalLength,
    listview_height: LogicalLength,
) -> bool {
    let zero = LogicalLength::default();
    let mut content_width_value = listview_width.get();
    let listview_height = listview_height.get();

    if row_count == 0 {
        ops.clear();
        props.content_height_set(zero);
        props.content_y_set(zero);
        props.content_width_set(listview_width);
        return false;
    }

    let mut content_y_value = props.content_y_get().get();
    if !props.content_y_has_binding() {
        content_y_value = content_y_value.min(0 as Coord);
    }

    let mut changed = false;

    // Estimate element height from cached value or by measuring existing instances.
    let element_height = if state.cached_item_height > 0 as Coord {
        state.cached_item_height
    } else {
        let mut total_height: Coord = 0 as Coord;
        let mut count = 0usize;
        for i in 0..ops.len() {
            if let Some(h) = ops.height(i) {
                total_height += h;
                count += 1;
            }
        }

        if count > 0 {
            total_height / count as Coord
        } else {
            // No items exist yet. Create one to measure.
            state.item_index.row = state.item_index.row.min(row_count - 1);
            ops.clear();
            ops.push(1);
            changed |= ops.ensure_updated(0, state.item_index.row);
            ops.height(0).unwrap_or(0 as Coord)
        }
    };

    if state.item_index.row >= row_count {
        state.item_index.row = row_count - 1;
    }

    let one_and_a_half_screen = listview_height * 3 as Coord / 2 as Coord;
    let first_item_y = state.anchor_y;
    let last_item_bottom = first_item_y + element_height * ops.len() as Coord;

    let (mut new_offset, mut new_offset_y) = if first_item_y
        > -content_y_value + one_and_a_half_screen
        || (props.computes_content_height() && last_item_bottom + element_height < -content_y_value)
    {
        // Jumping more than 1.5 screens: random seek.
        ops.clear();
        state.item_index.row =
            ((-content_y_value / element_height).floor() as usize).min(row_count - 1);
        (state.item_index.row, 0 as Coord)
    } else if content_y_value < state.previous_content_y {
        // Scrolled down: find the new offset by walking existing instances.
        let mut it_y = first_item_y + content_y_value;
        let mut new_off = state.item_index.row;
        for i in 0..ops.len() {
            changed |= ops.ensure_updated(i, new_off);
            let h = ops.height(i).unwrap_or(0 as Coord);
            if it_y + h > 0 as Coord || new_off + 1 >= row_count {
                break;
            }
            it_y += h;
            new_off += 1;
        }
        (new_off, it_y)
    } else {
        // Scrolled up: will instantiate items before offset in the loop below.
        (state.item_index.row, first_item_y + content_y_value)
    };

    let mut loop_count = 0;
    loop {
        // Fill gap before new_offset using already-instantiated items.
        while new_offset > state.item_index.row && new_offset_y > 0 as Coord {
            new_offset -= 1;
            new_offset_y -= ops.height(new_offset - state.item_index.row).unwrap_or(0 as Coord);
        }
        // If there is still a gap, create new instances before the current ones.
        let mut prepend_count = 0;
        while new_offset > 0 && new_offset_y > 0 as Coord {
            new_offset -= 1;
            ops.prepend(1);
            changed |= ops.ensure_updated(0, new_offset);
            new_offset_y -= ops.height(0).unwrap_or(0 as Coord);
            prepend_count += 1;
        }
        if prepend_count > 0 {
            state.item_index.row = new_offset;
        }
        debug_assert!(
            new_offset >= state.item_index.row && new_offset <= state.item_index.row + ops.len()
        );

        // Layout items until we fill the view, starting with already-instantiated ones.
        let mut y = new_offset_y;
        let mut idx = new_offset;
        let instances_begin = new_offset - state.item_index.row;
        for i in instances_begin..ops.len() {
            if idx >= row_count {
                break;
            }
            changed |= ops.ensure_updated(i, idx);
            content_width_value = content_width_value.max(ops.listview_layout(i, &mut y));
            idx += 1;
            if y >= listview_height {
                break;
            }
        }

        // Create more items until there is no more room.
        while y < listview_height && idx < row_count {
            let i = ops.len();
            ops.push(1);
            changed |= ops.ensure_updated(i, idx);
            content_width_value = content_width_value.max(ops.listview_layout(i, &mut y));
            idx += 1;
        }

        if y < listview_height && content_y_value < 0 as Coord && loop_count < 3 {
            debug_assert!(idx >= row_count);
            // Reached end of model with room to spare. Scroll up.
            content_y_value += listview_height - y;
            loop_count += 1;
            continue;
        }

        // Clean up instances that are not shown.
        if new_offset != state.item_index.row {
            let remove_count = new_offset - state.item_index.row;
            ops.remove_first(remove_count);
            state.item_index.row = new_offset;
        }
        let keep = idx - new_offset;
        ops.truncate(keep);

        if ops.len() == 0 {
            break;
        }

        // Recompute coordinates for the scrollbar.
        state.cached_item_height = (y - new_offset_y) / ops.len() as Coord;
        state.anchor_y = state.cached_item_height * state.item_index.row as Coord;
        props.content_height_set(LogicalLength::new(state.cached_item_height * row_count as Coord));
        props.content_width_set(LogicalLength::new(content_width_value));
        let new_content_y = -state.anchor_y + new_offset_y;
        // Important: Use get_internal here, the content_y may have a binding on it (especially
        // a physical animation).
        // We must not yet trigger a re-evaluation of that binding, as we have already updated the
        // content_width and content_height, but the content_y is not yet consistent.
        // So the physics animations limit value may be inconsistent.
        if new_content_y != props.content_y_get_internal().get() {
            // If a physics animation is ongoing (e.g. due to a flick), we should not interrupt it.
            // The physics animation implements intercept_set, and is therefore not interrupted by
            // a call to set() - so it's okay to just use a normal set here.
            props.content_y_set(LogicalLength::new(new_content_y));
        }
        state.previous_content_y = new_content_y;

        break;
    }

    changed
}

fn empty_slots<C: RepeatedItemTree>(
    count: usize,
) -> impl Iterator<Item = (RepeatedInstanceState, Option<ItemTreeRc<C>>)> {
    core::iter::repeat_with(|| (RepeatedInstanceState::Dirty, None)).take(count)
}

/// Adapter implementing [`RepeaterInstanceOps`] for the native Rust repeater.
struct RustRepeaterOps<'a, C: RepeatedItemTree> {
    inner: &'a RefCell<RepeaterInner<C>>,
    init: &'a dyn Fn() -> ItemTreeRc<C>,
    model: &'a ModelRc<C::Data>,
}

impl<C: RepeatedItemTree> RepeaterInstanceOps for RustRepeaterOps<'_, C> {
    fn len(&self) -> usize {
        self.inner.borrow().instances.len()
    }

    fn clear(&mut self) {
        let mut inner = self.inner.borrow_mut();
        inner.instances.clear();
        inner.layout_state.item_index.instance_index = 0;
    }

    fn push(&mut self, count: usize) {
        self.inner.borrow_mut().instances.extend(empty_slots(count));
    }

    fn prepend(&mut self, count: usize) {
        let mut inner = self.inner.borrow_mut();
        if !inner.instances.is_empty() {
            inner.layout_state.item_index.instance_index += count;
        }
        inner.instances.splice(0..0, empty_slots(count));
    }

    fn remove_first(&mut self, count: usize) {
        if count >= self.len() {
            return self.clear();
        }
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        inner.instances.drain(..count);
        let current = &mut inner.layout_state.item_index;
        if count > current.instance_index {
            // During a ListView update `row` lags behind the ListView's own copy of the layout
            // state, hence the saturating_sub
            current.row = current.row.saturating_sub(current.instance_index) + count;
            current.instance_index = 0;
        } else {
            current.instance_index -= count;
        }
    }

    fn truncate(&mut self, len: usize) {
        if len == 0 {
            return self.clear();
        }
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        if len >= inner.instances.len() {
            return;
        }
        inner.instances.truncate(len);
        let current = &mut inner.layout_state.item_index;
        if current.instance_index >= len {
            current.row = current.row.saturating_sub(current.instance_index) + len - 1;
            current.instance_index = len - 1;
        }
    }

    fn ensure_updated(&mut self, instance_idx: usize, row: usize) -> bool {
        let (created, instance) = {
            let mut inner = self.inner.borrow_mut();
            let c = &mut inner.instances[instance_idx];
            if c.0 != RepeatedInstanceState::Dirty {
                return false;
            }
            let created = c.1.is_none();
            if created {
                c.1 = Some((self.init)());
            }
            c.1.as_ref().unwrap().update(row, self.model.row_data(row).unwrap_or_default());
            c.0 = RepeatedInstanceState::Clean;
            (created, c.1.as_ref().unwrap().clone())
        };
        if created {
            crate::properties::evaluate_no_tracking(|| instance.init());
        }
        crate::item_tree::ensure_item_tree_instantiated(&vtable::VRc::into_dyn(instance));
        created
    }

    fn height(&self, instance_idx: usize) -> Option<Coord> {
        self.inner.borrow().instances[instance_idx]
            .1
            .as_ref()
            .map(|x| x.as_pin_ref().item_geometry(0).height_length().get())
    }

    fn listview_layout(&self, instance_idx: usize, y: &mut Coord) -> Coord {
        let inner = self.inner.borrow();
        let mut y_len = LogicalLength::new(*y);
        let w = inner.instances[instance_idx]
            .1
            .as_ref()
            .unwrap()
            .as_pin_ref()
            .listview_layout(&mut y_len);
        *y = y_len.get();
        w.get()
    }
}

/// This struct is put in a component when using the `for` syntax
/// It helps instantiating the ItemTree `T`
#[pin_project]
pub struct RepeaterTracker<T: RepeatedItemTree> {
    inner: RefCell<RepeaterInner<T>>,
    #[pin]
    model: Property<ModelRc<T::Data>>,
    #[pin]
    /// Set to true when the model becomes dirty.
    is_dirty: Property<bool>,
    #[pin]
    /// Marked dirty by `ensure_updated` when instances are added or
    /// removed.  Layout and visit code register this as a dependency so
    /// they re-evaluate only after the update pass materializes the
    /// change, not when the model first becomes dirty.
    instance_generation: Property<()>,
    /// Only used for the list view to track if the scrollbar has changed and item needs to be laid out again.
    #[pin]
    listview_geometry_tracker: crate::properties::PropertyTracker,
}

impl<T: RepeatedItemTree> ModelChangeListener for RepeaterTracker<T> {
    /// Notify the peers that a specific row was changed
    fn row_changed(self: Pin<&Self>, row: usize) {
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        if let Some(c) =
            inner.instances.get_mut(inner.layout_state.item_index.get_instance_index(row))
        {
            if !self.model.is_dirty() {
                if let Some(comp) = c.1.as_ref() {
                    let model = self.project_ref().model.get_untracked();
                    comp.update(row, model.row_data(row).unwrap_or_default());
                    c.0 = RepeatedInstanceState::Clean;
                }
            } else {
                c.0 = RepeatedInstanceState::Dirty;
            }
        }
    }
    /// Notify the peers that rows were added
    fn row_added(self: Pin<&Self>, index: usize, count: usize) {
        if count == 0 {
            return;
        }
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let current = &mut inner.layout_state.item_index;
        let first_row = current.first_row();
        if index > first_row + inner.instances.len() {
            // A slot for these rows would split the window
            return;
        }
        self.is_dirty.set(true);
        if index < first_row {
            current.row += count;
            for c in inner.instances.iter_mut() {
                c.0 = RepeatedInstanceState::Dirty;
            }
            return;
        }
        if index <= current.row && !inner.instances.is_empty() {
            current.row += count;
            current.instance_index += count;
        }
        let position = index - first_row;
        inner.instances.splice(
            position..position,
            core::iter::repeat_n((RepeatedInstanceState::Dirty, None), count),
        );
        for c in inner.instances[position + count..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedInstanceState::Dirty;
        }
    }
    /// Notify the peers that rows were removed
    fn row_removed(self: Pin<&Self>, index: usize, count: usize) {
        if count == 0 {
            return;
        }
        let row_count = self.model.get_internal().row_count();
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        let current = &mut inner.layout_state.item_index;
        let first_row = current.first_row();
        let end_row = first_row + inner.instances.len();
        let removed_end = index + count;
        if inner.instances.is_empty() {
            if removed_end <= current.row {
                current.row -= count;
            } else if index <= current.row {
                current.row = index.min(row_count.saturating_sub(1));
            }
            return;
        }
        if index >= end_row {
            return;
        }
        self.is_dirty.set(true);
        let start = index.clamp(first_row, end_row) - first_row;
        let end = removed_end.clamp(first_row, end_row) - first_row;
        inner.instances.drain(start..end);
        for c in inner.instances[start..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedInstanceState::Dirty;
        }
        if removed_end <= current.row {
            current.row -= count;
            current.instance_index -= end - start;
        } else if index <= current.row {
            // The current item moves to the next remaining instance, or else to the previous one
            *current = if start < inner.instances.len() {
                ItemIndexRelationShip { row: index, instance_index: start }
            } else if start > 0 {
                ItemIndexRelationShip { row: first_row + start - 1, instance_index: start - 1 }
            } else {
                ItemIndexRelationShip {
                    row: index.min(row_count.saturating_sub(1)),
                    instance_index: 0,
                }
            };
        }
    }

    fn reset(self: Pin<&Self>) {
        self.is_dirty.set(true);
        let mut inner = self.inner.borrow_mut();
        inner.instances.clear();
        let current = &mut inner.layout_state.item_index;
        *current = ItemIndexRelationShip { row: current.first_row(), instance_index: 0 };
    }
}

impl<C: RepeatedItemTree> Default for RepeaterTracker<C> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            model: Property::new_named(ModelRc::default(), "i_slint_core::Repeater::model"),
            is_dirty: Property::new_named(false, "i_slint_core::Repeater::is_dirty"),
            instance_generation: Property::new_named(
                (),
                "i_slint_core::Repeater::instance_generation",
            ),
            listview_geometry_tracker: Default::default(),
        }
    }
}

#[pin_project]
pub struct Repeater<C: RepeatedItemTree>(#[pin] ModelChangeListenerContainer<RepeaterTracker<C>>);

impl<C: RepeatedItemTree> Default for Repeater<C> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<C: RepeatedItemTree + 'static> Repeater<C> {
    fn data(self: Pin<&Self>) -> Pin<&RepeaterTracker<C>> {
        self.project_ref().0.get()
    }

    /// Register the model and dirty flag as dependencies of the current
    /// tracking scope (e.g. the redraw tracker) so it is notified when the
    /// model or its data changes.
    pub fn track_model_changes(self: Pin<&Self>) {
        self.data().project_ref().model.register_as_dependency();
        self.data().project_ref().is_dirty.register_as_dependency();
    }

    /// Register the instance generation as a dependency of the current
    /// tracking scope. This is for layout and visit code that should
    /// re-evaluate only after `ensure_updated` has materialized instance
    /// changes, not when the model first becomes dirty.
    pub fn track_instance_changes(self: Pin<&Self>) {
        self.data().project_ref().instance_generation.register_as_dependency();
    }

    /// Get the model of the repeater
    /// If the model is dirty the tracker to this repeater will be set up
    fn model(self: Pin<&Self>) -> ModelRc<C::Data> {
        let model = self.data().project_ref().model;

        if model.is_dirty() {
            let old_model = model.get_internal();
            let m = model.get();
            if old_model != m {
                *self.data().inner.borrow_mut() = RepeaterInner::default();
                self.data().is_dirty.set(true);
                let peer = self.project_ref().0.model_peer();
                m.model_tracker().attach_peer(peer);
            }
            m
        } else {
            model.get()
        }
    }

    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a ItemTree.
    /// Returns `true` if instances were actually created or removed.
    /// Also recurses into child instances to ensure they are instantiated.
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ItemTreeRc<C>) -> bool {
        let model = self.model();
        let changed = if self.data().project_ref().is_dirty.get() {
            let count = model.row_count();
            let offset = {
                let mut inner = self.0.inner.borrow_mut();
                if inner.instances.is_empty() {
                    // Outside a ListView the instances start at row 0
                    inner.layout_state.item_index = Default::default();
                }
                inner.layout_state.item_index.first_row()
            };
            let mut ops = RustRepeaterOps { inner: &self.0.inner, init: &init, model: &model };
            self.data().is_dirty.set(false);
            update_all_instances(&mut ops, offset, count);
            self.data().instance_generation.mark_dirty();
            true
        } else {
            false
        };
        self.ensure_children_instantiated() || changed
    }

    /// Recurse into child instances to ensure they are instantiated.
    fn ensure_children_instantiated(&self) -> bool {
        let mut changed = false;
        for instance in self.instances_vec() {
            changed |=
                crate::item_tree::ensure_item_tree_instantiated(&vtable::VRc::into_dyn(instance));
        }
        changed
    }

    /// Register the ListView content properties as dependencies so that
    /// scrolling triggers a redraw.  Model dependencies are registered by
    /// [`Self::visit`], so this only covers the content geometry.
    pub fn track_changes_listview(
        self: Pin<&Self>,
        content_width: Option<Pin<&Property<LogicalLength>>>,
        content_height: Option<Pin<&Property<LogicalLength>>>,
        content_y: Pin<&Property<LogicalLength>>,
        listview_width: LogicalLength,
        listview_height: Pin<&Property<LogicalLength>>,
    ) {
        let props = TypedListViewProps { content_width, content_height, content_y };
        self.track_changes_listview_callback(&props, listview_width);
        listview_height.register_as_dependency();
    }

    /// Trait-based variant of [`Self::track_changes_listview`] for runtime
    /// consumers that can't expose the content storage as strongly-typed
    /// `Pin<&Property<LogicalLength>>` references. The caller is
    /// responsible for registering the listview height as a dependency.
    pub fn track_changes_listview_callback(
        self: Pin<&Self>,
        props: &dyn ListViewProperties,
        listview_width: LogicalLength,
    ) {
        props.register_as_dependencies();
        // listview_width is passed as a value, not a property, so it cannot
        // be registered as a dependency. Kept in the signature for symmetry
        // with ensure_updated_listview.
        let _ = listview_width;
    }

    /// Same as `Self::ensure_updated` but for a ListView.
    /// Returns `true` if any instances were created or any child changed.
    pub fn ensure_updated_listview(
        self: Pin<&Self>,
        init: impl Fn() -> ItemTreeRc<C>,
        content_width: Option<Pin<&Property<LogicalLength>>>,
        content_height: Option<Pin<&Property<LogicalLength>>>,
        content_y: Pin<&Property<LogicalLength>>,
        listview_width: LogicalLength,
        listview_height: Pin<&Property<LogicalLength>>,
    ) -> bool {
        let props = TypedListViewProps { content_width, content_height, content_y };
        self.ensure_updated_listview_callback(init, &props, listview_width, listview_height.get())
    }

    /// Trait-based variant of [`Self::ensure_updated_listview`] for runtime
    /// consumers (the interpreter) that can't expose the content storage as
    /// strongly-typed `Pin<&Property<LogicalLength>>` references — for
    /// instance when the content is backed by a native item property
    /// accessed through rtti.
    pub fn ensure_updated_listview_callback(
        self: Pin<&Self>,
        init: impl Fn() -> ItemTreeRc<C>,
        props: &dyn ListViewProperties,
        listview_width: LogicalLength,
        listview_height: LogicalLength,
    ) -> bool {
        self.data().project_ref().is_dirty.set(false);

        let model = self.model();
        let row_count = model.row_count();

        let data = self.data();
        let mut layout_state = {
            let mut inner = data.inner.borrow_mut();
            let current = &mut inner.layout_state.item_index;
            // `update_visible_instances` keeps the current item on the first instance
            *current = ItemIndexRelationShip { row: current.first_row(), instance_index: 0 };
            inner.layout_state.clone()
        };
        let mut ops = RustRepeaterOps { inner: &data.inner, init: &init, model: &model };
        let changed = update_visible_instances(
            &mut ops,
            &mut layout_state,
            row_count,
            props,
            listview_width,
            listview_height,
        );
        data.inner.borrow_mut().layout_state = layout_state;

        if changed {
            self.data().instance_generation.mark_dirty();
        }
        self.ensure_children_instantiated() || changed
    }

    /// Sets the data directly in the model
    pub fn model_set_row_data(self: Pin<&Self>, row: usize, data: C::Data) {
        let model = self.model();
        model.set_row_data(row, data);
    }

    /// Read a row from the model, registering a dependency on it when
    /// called from a binding evaluation.
    pub fn model_row_data(self: Pin<&Self>, row: usize) -> Option<C::Data> {
        self.model().row_data_tracked(row)
    }

    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> ModelRc<C::Data> + 'static) {
        self.0.model.set_binding(binding);
    }

    /// Call the visitor for the root of each instance.
    /// Also registers model dependencies so the current tracking scope
    /// (e.g. the redraw tracker) is notified when the model changes.
    pub fn visit(
        self: Pin<&Self>,
        order: TraversalOrder,
        mut visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        self.track_model_changes();
        // We can't keep self.inner borrowed because the event might modify the model
        let count = self.0.inner.borrow().instances.len() as u32;
        for i in 0..count {
            let i = if order == TraversalOrder::BackToFront { i } else { count - i - 1 };
            let c = self.0.inner.borrow().instances.get(i as usize).and_then(|c| c.1.clone());
            if let Some(c) = c
                && c.as_pin_ref().visit_children_item(-1, order, visitor.borrow_mut()).has_aborted()
            {
                return crate::item_tree::VisitChildrenResult::abort(i, 0);
            }
        }
        crate::item_tree::VisitChildrenResult::CONTINUE
    }

    /// Call `cb` with the model row index and the z value of every instance, when the
    /// repeated element has a dynamic z binding. The row index is the one accepted by
    /// [`Self::instance_at`] (and thus by the `get_subtree` vtable entry).
    /// Also registers model dependencies so the current tracking scope is notified
    /// when the model changes.
    pub fn for_each_instance_z(self: Pin<&Self>, cb: &mut dyn FnMut(u32, f32)) {
        self.track_model_changes();
        // Read the z values without holding the borrow: evaluating the z property's
        // binding can run user code
        let (offset, instances): (usize, Vec<_>) = {
            let inner = self.0.inner.borrow();
            (
                inner.layout_state.item_index.first_row(),
                inner.instances.iter().map(|c| c.1.clone()).collect(),
            )
        };
        for (i, c) in instances.iter().enumerate() {
            let z = c.as_ref().and_then(|c| c.as_pin_ref().z_order()).unwrap_or_default();
            cb((offset + i) as u32, z);
        }
    }

    /// Return the amount of instances currently in the repeater
    pub fn len(&self) -> usize {
        self.0.inner.borrow().instances.len()
    }

    /// Return the range of indices used by this Repeater.
    ///
    /// Two values are necessary here since the Repeater can start to insert the data from its
    /// model at an offset.
    pub fn range(&self) -> core::ops::Range<usize> {
        let inner = self.0.inner.borrow();
        let start = inner.layout_state.item_index.first_row();
        core::ops::Range { start, end: start + inner.instances.len() }
    }

    /// Return the instance for the given model index.
    /// The index should be within [`Self::range()`]
    pub fn instance_at(&self, index: usize) -> Option<ItemTreeRc<C>> {
        let inner = self.0.inner.borrow();
        inner
            .instances
            .get(inner.layout_state.item_index.get_instance_index_opt(index)?)
            .and_then(|c| c.1.clone())
    }

    /// Return true if the Repeater as empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all instances
    pub fn instances_vec(&self) -> Vec<ItemTreeRc<C>> {
        self.0.inner.borrow().instances.iter().flat_map(|x| x.1.clone()).collect()
    }
}

#[pin_project]
pub struct Conditional<C: RepeatedItemTree> {
    #[pin]
    model: Property<bool>,
    #[pin]
    instance_generation: Property<()>,
    instance: RefCell<Option<ItemTreeRc<C>>>,
}

impl<C: RepeatedItemTree> Default for Conditional<C> {
    fn default() -> Self {
        Self {
            model: Property::new_named(false, "i_slint_core::Conditional::model"),
            instance_generation: Property::new_named(
                (),
                "i_slint_core::Conditional::instance_generation",
            ),
            instance: RefCell::new(None),
        }
    }
}

impl<C: RepeatedItemTree + 'static> Conditional<C> {
    /// Register the condition as a dependency of the current tracking scope
    /// (e.g. the redraw tracker) so it is notified when the condition changes.
    pub fn track_model_changes(self: Pin<&Self>) {
        self.project_ref().model.register_as_dependency();
    }

    /// Register the instance generation as a dependency of the current
    /// tracking scope. Layout code uses this to re-evaluate only after
    /// `ensure_updated` materializes instance changes.
    pub fn track_instance_changes(self: Pin<&Self>) {
        self.project_ref().instance_generation.register_as_dependency();
    }

    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a ItemTree.
    /// Returns `true` if the instance was created or removed, or any child changed.
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ItemTreeRc<C>) -> bool {
        let model = self.project_ref().model.get();

        let changed = if !model {
            self.instance.take().is_some()
        } else if self.instance.borrow().is_none() {
            let i = init();
            self.instance.replace(Some(i.clone()));
            i.init();
            true
        } else {
            false
        };
        if changed {
            self.instance_generation.mark_dirty();
        }
        if let Some(instance) = self.instance.borrow().as_ref() {
            crate::item_tree::ensure_item_tree_instantiated(&vtable::VRc::into_dyn(
                instance.clone(),
            )) || changed
        } else {
            changed
        }
    }

    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> bool + 'static) {
        self.model.set_binding(binding);
    }

    /// Call the visitor for the root of each instance.
    /// Also registers model dependencies so the current tracking scope
    /// (e.g. the redraw tracker) is notified when the condition changes.
    pub fn visit(
        self: Pin<&Self>,
        order: TraversalOrder,
        mut visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        self.track_model_changes();
        // We can't keep self.inner borrowed because the event might modify the model
        let instance = self.instance.borrow().clone();
        if let Some(c) = instance
            && c.as_pin_ref().visit_children_item(-1, order, visitor.borrow_mut()).has_aborted()
        {
            return crate::item_tree::VisitChildrenResult::abort(0, 0);
        }

        crate::item_tree::VisitChildrenResult::CONTINUE
    }

    /// Call `cb` with the index and the z value of the instance if the condition is
    /// active, when the conditional element has a dynamic z binding.
    /// Also registers the condition as a dependency of the current tracking scope.
    pub fn for_each_instance_z(self: Pin<&Self>, cb: &mut dyn FnMut(u32, f32)) {
        self.track_model_changes();
        let instance = self.instance.borrow().clone();
        if let Some(c) = instance {
            cb(0, c.as_pin_ref().z_order().unwrap_or_default());
        }
    }

    /// Return the amount of instances (1 if the conditional is active, 0 otherwise)
    pub fn len(&self) -> usize {
        self.instance.borrow().is_some() as usize
    }

    /// Return the range of indices used by this Conditional.
    ///
    /// Similar to Repeater::range, but the range is always [0, 1] if the Conditional is active.
    pub fn range(&self) -> core::ops::Range<usize> {
        0..self.len()
    }

    /// Return the instance for the given model index.
    /// The index should be within [`Self::range()`]
    pub fn instance_at(&self, index: usize) -> Option<ItemTreeRc<C>> {
        if index != 0 {
            return None;
        }
        self.instance.borrow().clone()
    }

    /// Return true if the Repeater as empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all instances
    pub fn instances_vec(&self) -> Vec<ItemTreeRc<C>> {
        self.instance.borrow().clone().into_iter().collect()
    }
}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    /// C++ callback table for [`RepeaterInstanceOps`], including the opaque
    /// user_data pointer that is passed to each callback.
    #[repr(C)]
    pub struct RepeaterInstanceOpsVTable {
        pub user_data: *mut core::ffi::c_void,
        pub len: unsafe extern "C" fn(user_data: *mut core::ffi::c_void) -> usize,
        pub clear: unsafe extern "C" fn(user_data: *mut core::ffi::c_void),
        pub push: unsafe extern "C" fn(user_data: *mut core::ffi::c_void, count: usize),
        pub prepend: unsafe extern "C" fn(user_data: *mut core::ffi::c_void, count: usize),
        pub remove_first: unsafe extern "C" fn(user_data: *mut core::ffi::c_void, count: usize),
        pub truncate: unsafe extern "C" fn(user_data: *mut core::ffi::c_void, len: usize),
        pub ensure_updated: unsafe extern "C" fn(
            user_data: *mut core::ffi::c_void,
            instance_idx: usize,
            row: usize,
        ) -> bool,
        /// Height of instance, or NaN if not yet created.
        pub height:
            unsafe extern "C" fn(user_data: *mut core::ffi::c_void, instance_idx: usize) -> Coord,
        pub listview_layout: Option<
            unsafe extern "C" fn(
                user_data: *mut core::ffi::c_void,
                instance_idx: usize,
                y: &mut Coord,
            ) -> Coord,
        >,
        pub init: unsafe extern "C" fn(user_data: *mut core::ffi::c_void, instance_idx: usize),
    }

    impl RepeaterInstanceOps for RepeaterInstanceOpsVTable {
        fn len(&self) -> usize {
            unsafe { (self.len)(self.user_data) }
        }
        fn clear(&mut self) {
            unsafe { (self.clear)(self.user_data) }
        }
        fn push(&mut self, count: usize) {
            unsafe { (self.push)(self.user_data, count) }
        }
        fn prepend(&mut self, count: usize) {
            unsafe { (self.prepend)(self.user_data, count) }
        }
        fn remove_first(&mut self, count: usize) {
            unsafe { (self.remove_first)(self.user_data, count) }
        }
        fn truncate(&mut self, len: usize) {
            unsafe { (self.truncate)(self.user_data, len) }
        }
        fn ensure_updated(&mut self, instance_idx: usize, row: usize) -> bool {
            let created = unsafe { (self.ensure_updated)(self.user_data, instance_idx, row) };
            if created {
                unsafe { (self.init)(self.user_data, instance_idx) };
            }
            created
        }
        fn height(&self, instance_idx: usize) -> Option<Coord> {
            let h = unsafe { (self.height)(self.user_data, instance_idx) };
            if h.is_nan() { None } else { Some(h) }
        }
        fn listview_layout(&self, instance_idx: usize, y: &mut Coord) -> Coord {
            self.listview_layout
                .map_or(0 as Coord, |f| unsafe { f(self.user_data, instance_idx, y) })
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_repeater_ensure_updated(
        ops: &mut RepeaterInstanceOpsVTable,
        offset: usize,
        count: usize,
    ) {
        update_all_instances(ops, offset, count);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_repeater_ensure_updated_listview(
        ops: &mut RepeaterInstanceOpsVTable,
        state: &mut RepeaterLayoutState,
        row_count: usize,
        content_width: Option<Pin<&Property<LogicalLength>>>,
        content_height: Option<Pin<&Property<LogicalLength>>>,
        content_y: Pin<&Property<LogicalLength>>,
        listview_width: LogicalLength,
        listview_height: LogicalLength,
    ) -> bool {
        let props = TypedListViewProps { content_width, content_height, content_y };
        update_visible_instances(ops, state, row_count, &props, listview_width, listview_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SharedString;
    use crate::accessibility::{
        AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
    };
    use crate::cursor::MouseCursorInner;
    use crate::input::{
        FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, InternalKeyEvent,
        KeyEventResult, MouseEvent,
    };
    use crate::item_tree::{
        IndexRange, ItemTree, ItemTreeNode, ItemTreeWeak, ItemVisitorVTable, ItemWeak,
        VisitChildrenResult,
    };
    use crate::items::{AccessibleRole, ItemRc, ItemVTable, RenderingResult};
    use crate::layout::LayoutInfo;
    use crate::lengths::{LogicalRect, LogicalSize};
    use crate::model::{ModelNotify, ModelTracker, VecModel};
    use crate::slice::Slice;
    use crate::window::{WindowAdapter, WindowAdapterRc};
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use core::sync::atomic;
    use std::{println, vec};
    use vtable::VRc;

    type ItemRendererRef<'a> = &'a mut dyn crate::item_rendering::ItemRenderer;

    // Simple Item used in the tests for mocking
    #[derive(Default)]
    struct SimpleItem {
        value: crate::model::Cell<i32>,
        instantiated: bool,
    }
    crate::item_tree::ItemTreeVTable_static!(static TEST_COMPONENT_VT for SimpleItem);
    impl RepeatedItemTree for SimpleItem {
        type Data = i32;

        fn update(&self, _index: usize, data: Self::Data) {
            self.value.set(data);
        }

        fn listview_layout(self: Pin<&Self>, offset_y: &mut LogicalLength) -> LogicalLength {
            *offset_y += LogicalLength::new(ITEM_HEIGHT);
            LogicalLength::default()
        }
    }
    impl SimpleItem {
        fn new(data: i32) -> VRc<ItemTreeVTable, Self> {
            let mut _self = Self::default();
            _self.value.set(data);
            let self_rc = VRc::new(_self);
            self_rc
        }
    }

    fn ensure_updated_window(repeater: &Repeater<SimpleItem>) {
        let current = repeater.0.inner.borrow().layout_state.item_index.clone();
        let model = repeater.0.model.get_internal();
        let mut ops = RustRepeaterOps { inner: &repeater.0.inner, init: &new_item, model: &model };
        for instance_index in 0..ops.len() {
            ops.ensure_updated(
                instance_index,
                current.row - current.instance_index + instance_index,
            );
        }
    }

    // Get value stored in SimpleItem
    fn get_instance_inner_value(
        inner: &core::cell::Ref<'_, RepeaterInner<SimpleItem>>,
        index: usize,
    ) -> i32 {
        inner.instances[index].1.as_ref().unwrap().as_pin_ref().value.get()
    }

    /// Remove elements from the model using the repeater op
    #[test]
    fn test_repeater_ops_splice() {
        let repeater: Repeater<SimpleItem> = Repeater::default();
        let repeater = core::pin::pin!(repeater);

        let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(vec![1, 2, 3, 4, 5, 6, 7, 8]));

        repeater.set_model_binding({
            let model = model.clone();
            move || ModelRc::from(model.clone())
        });
        repeater.as_ref().model(); // Setup tracker

        repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 8);
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(inner.layout_state.item_index.instance_index, 0);

            for (index, (state, item)) in inner.instances.iter().enumerate() {
                assert_eq!(index as i32 + 1, item.as_ref().unwrap().value.get());
                assert_ne!(*state, RepeatedInstanceState::Dirty);
            }
        }

        {
            // Remove after current row -> current row still is the same
            let mut ops = RustRepeaterOps {
                inner: &repeater.0.inner,
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            };
            // Remove 4 at index 4
            ops.truncate(4);

            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 4);
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
            for (index, (state, item)) in inner.instances.iter().enumerate() {
                assert_eq!(index as i32 + 1, item.as_ref().unwrap().value.get());
                assert_ne!(*state, RepeatedInstanceState::Dirty);
            }
        }

        {
            // Remove current row
            let mut ops = RustRepeaterOps {
                inner: &repeater.0.inner,
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            };
            // Remove first 2 items
            ops.remove_first(2);

            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 2);
            assert_eq!(
                inner.layout_state.item_index.row, 2,
                "Current row instance was removed. So we move to the first available instance"
            );
            assert_eq!(
                inner.layout_state.item_index.instance_index, 0,
                "Didn't change. Points now to the first remaining instance"
            );
            const OFFSET: i32 = 2; // First instance must now be the element with value 3
            for (index, (state, item)) in inner.instances.iter().enumerate() {
                assert_eq!(index as i32 + 1 + OFFSET, item.as_ref().unwrap().value.get());
                assert_ne!(*state, RepeatedInstanceState::Dirty);
            }
        }
    }

    #[test]
    fn test_repeater() {
        let repeater: Repeater<SimpleItem> = Repeater::default();
        let repeater = core::pin::pin!(repeater);

        let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(vec![1, 2, 3, 4, 5, 6, 7, 8]));

        repeater.set_model_binding({
            let model = model.clone();
            move || ModelRc::from(model.clone())
        });
        repeater.as_ref().model(); // Setup tracker

        repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 8);
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
            for (index, (state, item)) in inner.instances.iter().enumerate() {
                assert_eq!(index as i32 + 1, item.as_ref().unwrap().value.get());
                assert_ne!(*state, RepeatedInstanceState::Dirty);
            }
        }

        // All items are there already
        repeater.0.inner.borrow_mut().set_current_row(2);

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 8);
            assert_eq!(inner.layout_state.item_index.row, 2);
            assert_eq!(inner.layout_state.item_index.instance_index, 2);
            // None are dirty
            for (index, (state, item)) in inner.instances.iter().enumerate() {
                assert_eq!(index as i32 + 1, item.as_ref().unwrap().value.get());
                assert_ne!(*state, RepeatedInstanceState::Dirty);
            }
        }

        {
            // Remove multiple instances so we have less
            let mut ops = RustRepeaterOps {
                inner: &repeater.0.inner,
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            };
            ops.truncate(4);

            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 4);
            assert_eq!(inner.layout_state.item_index.row, 2);
            assert_eq!(inner.layout_state.item_index.instance_index, 2);
        }

        // We are past the last instance so new instances must be created
        repeater.0.inner.borrow_mut().set_current_row(6);

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 7);
            assert_eq!(inner.layout_state.item_index.row, 6);
            assert_eq!(inner.layout_state.item_index.instance_index, 6);
            for (index, (state, item)) in inner.instances.iter().enumerate() {
                if index < 4 {
                    assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                    assert_eq!(index as i32 + 1, item.as_ref().unwrap().value.get());
                } else {
                    assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                }
            }
        }

        const REMOVE_COUNT: usize = 2;
        {
            // Remove first 3 instances -> current row changes to still point to the same item
            let mut ops = RustRepeaterOps {
                inner: &repeater.0.inner,
                init: &|| SimpleItem::new(-1),
                model: &ModelRc::from(model.clone()),
            };
            ops.remove_first(REMOVE_COUNT);

            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 7 - REMOVE_COUNT);
            assert_eq!(inner.layout_state.item_index.row, 6);
            assert_eq!(inner.layout_state.item_index.instance_index, 6 - REMOVE_COUNT);

            for (index, (state, item)) in inner.instances.iter().enumerate() {
                if index < 2 {
                    assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                    assert_eq!(
                        index as i32 + 1 + REMOVE_COUNT as i32,
                        item.as_ref().unwrap().value.get(),
                        "Index: {index}"
                    );
                } else {
                    assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                }
            }
        }

        // Set to a row which is not yet instantiated because we removed before so new items must be created
        // we removed previously 2 elements, therefore there are 4 items left in the instance and the 4th is row 6
        // to get to row zero we have to add 2 elements (4 - 6 = -2)
        const ADDED_COUNT: usize = 2;
        repeater.0.inner.borrow_mut().set_current_row(0);

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 7 - REMOVE_COUNT + ADDED_COUNT);
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(
                inner.layout_state.item_index.instance_index, 0,
                "New items got instantiated to be able to represent row 0"
            );
            for (index, (state, item)) in inner.instances.iter().enumerate() {
                if index == 2 || index == 3 {
                    assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                    assert_eq!(
                        index as i32 + 1 + REMOVE_COUNT as i32 - ADDED_COUNT as i32,
                        item.as_ref().unwrap().value.get(),
                        "Index: {index}"
                    );
                } else {
                    assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                }
            }
        }
    }

    #[test]
    fn test_adding_element() {
        let repeater: Repeater<SimpleItem> = Repeater::default();
        let repeater = core::pin::pin!(repeater);
        let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(Vec::new()));

        repeater.set_model_binding({
            let model = model.clone();
            move || ModelRc::from(model.clone())
        });
        repeater.as_ref().model(); // Setup tracker

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 0, "We don't have any items yet in the model");
            assert_eq!(inner.layout_state.item_index.row, 0);
        }

        model.push(2);
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 1, "We added one element");
            assert_eq!(
                inner.instances[0].0,
                RepeatedInstanceState::Dirty,
                "The item was just added so it is still dirty"
            );
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
            // assert_eq!(get_instance_inner_value(&inner, 0), 2); // Still dirty
        }
        repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 1);
            assert_eq!(get_instance_inner_value(&inner, 0), 2);
        }

        // Append
        model.push(5);
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 2);
            assert_eq!(
                inner.layout_state.item_index.row, 0,
                "We appended the item so we don't have to adapt"
            );
            assert_ne!(
                inner.instances[0].0,
                RepeatedInstanceState::Dirty,
                "The item was already inside so it must not be dirty because of the previous ensure_updated"
            );
            assert_eq!(
                inner.instances[1].0,
                RepeatedInstanceState::Dirty,
                "The item was just added so it is still dirty"
            );
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
            assert_eq!(get_instance_inner_value(&inner, 0), 2);
            // assert_eq!(get_instance_inner_value(&inner, 1), 5); // It is still dirty!
        }
        repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

        // Check that appended by checking their internal values
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 2);
            assert_eq!(get_instance_inner_value(&inner, 0), 2);
            assert_eq!(get_instance_inner_value(&inner, 1), 5);
        }

        // Prepend
        model.insert(0, 9);
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.instances.len(), 3);
            assert_eq!(
                inner.layout_state.item_index.row, 1,
                "We inserted an element before the current element so we have to adapt"
            );
            assert_eq!(inner.layout_state.item_index.instance_index, 1);

            // All are dirty because the first was added and the others got moved because of the prepend
            for instance in &inner.instances {
                assert_eq!(instance.0, RepeatedInstanceState::Dirty);
            }
        }

        repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

        // Check that prepended by checking their internal values
        // All must got dirty because the first was prepended and the others shifted
        {
            assert_eq!(model.row_data(0).unwrap(), 9);
            assert_eq!(model.row_data(1).unwrap(), 2);
            assert_eq!(model.row_data(2).unwrap(), 5);

            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.layout_state.item_index.instance_index, 1); // One was prepended
            assert_eq!(inner.instances.len(), 3);
            assert_eq!(get_instance_inner_value(&inner, 0), 9, "Was prepended");
            assert_eq!(
                get_instance_inner_value(&inner, inner.layout_state.item_index.instance_index),
                2
            );
            assert_eq!(get_instance_inner_value(&inner, 2), 5);
        }
    }

    #[test]
    fn test_remove_elements() {
        let repeater: Repeater<SimpleItem> = Repeater::default();
        let repeater = core::pin::pin!(repeater);
        let model: Rc<VecModel<i32>> = Rc::new(VecModel::from(Vec::from_iter(2..6)));

        repeater.set_model_binding({
            let model = model.clone();
            move || ModelRc::from(model.clone())
        });
        repeater.as_ref().model(); // Setup tracker

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(model.row_count(), 4);
            assert_eq!(
                inner.instances.len(),
                0,
                "We didn't instantiate one yet because ensure updated was not yet called"
            );
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
        }

        // Create instances
        repeater.as_ref().ensure_updated(|| SimpleItem::new(-1));

        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(model.row_count(), 4, "We added 4 elements");
            assert_eq!(inner.instances.len(), 4, "4 instances are created for every element one");
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(inner.layout_state.item_index.instance_index, 0);

            for (index, (state, item)) in inner.instances.iter().enumerate() {
                assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                assert_eq!(index as i32 + 2, item.as_ref().unwrap().value.get(), "Index: {index}");
            }
        }

        // Point as current element to the 3th one but the instance index is still at 0.
        // That means for the listview that the current item is at the beginning and
        // the next visible elements are after and no element before the current item is visible
        const REMOVE_COUNT: usize = 2; // Count of instances to remove
        {
            let mut inner = repeater.0.inner.borrow_mut();
            inner.set_current_row(2);
            assert_eq!(
                inner.layout_state.item_index.instance_index, 2,
                "Must move together with the row"
            );
            assert_eq!(model.row_data(inner.layout_state.item_index.row), Some(4));
            drop(inner);
            {
                // Remove first two instances
                let mut ops = RustRepeaterOps {
                    inner: &repeater.0.inner,
                    init: &|| SimpleItem::new(-1),
                    model: &ModelRc::from(model.clone()),
                };
                // Remove first REMOVE_COUNT items
                ops.remove_first(REMOVE_COUNT);
                let inner = repeater.0.inner.borrow();
                assert_eq!(
                    inner.layout_state.item_index.instance_index, 0,
                    "Must point to the first item because the first two are removed"
                );
                assert_eq!(
                    model.row_data(inner.layout_state.item_index.row),
                    Some(4),
                    "The row must stay"
                );
            }
            let inner = repeater.0.inner.borrow();
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
            assert_eq!(
                get_instance_inner_value(&inner, inner.layout_state.item_index.instance_index),
                4
            );
            for (index, (state, item)) in inner.instances.iter().enumerate() {
                assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                assert_eq!(
                    index as i32 + 2 + REMOVE_COUNT as i32,
                    item.as_ref().unwrap().value.get(),
                    "Index: {index}"
                );
            }
        }

        // Remove one after the current element
        assert_eq!(model.remove(3), 5, "Last element must be 5");
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(model.row_count(), 3); // --> 2, 3, 4 are left
            assert_eq!(
                inner.instances.len(),
                1,
                "We had totally 4 instances. Above we removed 2 and now another one. The one left is the instance for the current row"
            );
            assert_eq!(
                inner.layout_state.item_index.row, 2,
                "The row must not change because the removed is after"
            );
            assert_eq!(
                inner.layout_state.item_index.instance_index, 0,
                "The instance index must not change because the removed is after"
            );
            assert_eq!(model.row_data(inner.layout_state.item_index.row), Some(4));

            // The current item is not dirty but all after
            assert_ne!(
                inner.instances[inner.layout_state.item_index.instance_index].0,
                RepeatedInstanceState::Dirty
            );

            for (index, (state, item)) in inner.instances.iter().enumerate() {
                if index <= inner.layout_state.item_index.instance_index {
                    assert_ne!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                    assert_eq!(
                        index as i32 + 2 + REMOVE_COUNT as i32,
                        item.as_ref().unwrap().value.get(),
                        "Index: {index}"
                    );
                } else {
                    // All items after the current index are dirty
                    assert_eq!(*state, RepeatedInstanceState::Dirty, "Index: {index}");
                }
            }
        }

        // Remove one before the current element
        // Remove one element of the three left
        assert_eq!(model.remove(1), 3, "Second element must have the internal value 3");
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(model.row_count(), 2); // -> 2, 4 left
            assert_eq!(
                inner.instances.len(),
                1,
                "The removed item was before the current row. So the instance does not change"
            );
            assert_eq!(inner.layout_state.item_index.row, 1);
            assert_eq!(
                inner.layout_state.item_index.instance_index, 0,
                "The instance did not change because the removed item was out of view"
            );
            assert_eq!(model.row_data(inner.layout_state.item_index.row), Some(4));
            assert_eq!(
                get_instance_inner_value(&inner, inner.layout_state.item_index.instance_index),
                4
            );
        }

        // Remove current item
        assert_eq!(model.remove(1), 4);
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(model.row_count(), 1); // -> item with value 2 left
            assert_eq!(inner.instances.len(), 0, "Current instance is now cleared as well");
            assert_eq!(
                inner.layout_state.item_index.row, 0,
                "row points now to the last remaining item"
            );
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
            assert_eq!(
                model.row_data(inner.layout_state.item_index.row),
                Some(2),
                "row points now to the previous one"
            );
        }

        // Remove last element
        assert_eq!(model.remove(0), 2);
        {
            let inner = repeater.0.inner.borrow();
            assert_eq!(model.row_count(), 0);
            assert_eq!(inner.instances.len(), 0);
            assert_eq!(inner.layout_state.item_index.row, 0);
            assert_eq!(inner.layout_state.item_index.instance_index, 0);
            assert_eq!(model.row_data(inner.layout_state.item_index.row), None, "Should be empty");
        }
    }

    // TODO: Test swapping!

    const ITEM_HEIGHT: Coord = 100 as Coord;

    /// Unlike `VecModel`, inserts and removes several rows with a single notification.
    struct TestModel {
        rows: RefCell<Vec<i32>>,
        notify: ModelNotify,
    }

    impl TestModel {
        /// Rows `0..count`, each holding its own index
        fn new(count: usize) -> Rc<Self> {
            Rc::new(Self {
                rows: RefCell::new((0..count as i32).collect()),
                notify: Default::default(),
            })
        }

        fn insert(&self, index: usize, values: &[i32]) {
            self.rows.borrow_mut().splice(index..index, values.iter().copied());
            self.notify.row_added(index, values.len());
        }

        fn remove(&self, rows: core::ops::Range<usize>) {
            self.rows.borrow_mut().drain(rows.clone());
            self.notify.row_removed(rows.start, rows.len());
        }
    }

    impl Model for TestModel {
        type Data = i32;

        fn row_count(&self) -> usize {
            self.rows.borrow().len()
        }

        fn row_data(&self, row: usize) -> Option<i32> {
            self.rows.borrow().get(row).copied()
        }

        fn model_tracker(&self) -> &dyn ModelTracker {
            &self.notify
        }
    }

    #[derive(Debug, PartialEq)]
    enum Slot {
        Clean(i32),
        /// Marked dirty, still showing the data of its last update
        Dirty,
        /// Marked dirty, without an instance yet
        Empty,
    }
    use Slot::{Clean, Dirty, Empty};

    /// The instance slots and the current item
    fn state(repeater: &Repeater<SimpleItem>) -> (Vec<Slot>, ItemIndexRelationShip) {
        let inner = repeater.0.inner.borrow();
        let slots = inner
            .instances
            .iter()
            .map(|(state, instance)| match (state, instance) {
                (RepeatedInstanceState::Clean, Some(instance)) => Clean(instance.value.get()),
                (RepeatedInstanceState::Dirty, Some(instance)) => Dirty,
                (RepeatedInstanceState::Dirty, None) => Empty,
                (RepeatedInstanceState::Clean, None) => panic!("clean slot without an instance"),
            })
            .collect();
        (slots, inner.layout_state.item_index.clone())
    }

    fn new_item() -> ItemTreeRc<SimpleItem> {
        SimpleItem::new(-1)
    }

    /// A repeater with clean instances for the rows in `window` only, as a ListView leaves it
    fn repeater_with_window(
        model: &Rc<TestModel>,
        window: core::ops::Range<usize>,
        current_row: usize,
    ) -> Pin<Box<Repeater<SimpleItem>>> {
        let repeater = Box::pin(Repeater::default());
        let model_rc = ModelRc::from(model.clone());
        repeater.set_model_binding(move || model_rc.clone());
        repeater.as_ref().model(); // Setup tracker
        let mut inner = repeater.0.inner.borrow_mut();
        inner.instances = window
            .clone()
            .map(|row| {
                let instance = SimpleItem::new(model.row_data(row).unwrap());
                (RepeatedInstanceState::Clean, Some(instance))
            })
            .collect();
        inner.layout_state.item_index =
            ItemIndexRelationShip { row: current_row, instance_index: current_row - window.start };
        drop(inner);
        repeater.0.is_dirty.set(false);
        repeater
    }

    /// A repeater outside a ListView, with instances for all rows
    fn repeater_for(model: &Rc<VecModel<i32>>) -> Pin<Box<Repeater<SimpleItem>>> {
        let repeater = Box::pin(Repeater::default());
        let model = ModelRc::from(model.clone());
        repeater.set_model_binding(move || model.clone());
        repeater.as_ref().ensure_updated(new_item);
        repeater
    }

    fn with_ops(
        repeater: &Repeater<SimpleItem>,
        f: impl FnOnce(&mut RustRepeaterOps<'_, SimpleItem>),
    ) {
        let model = ModelRc::default();
        f(&mut RustRepeaterOps { inner: &repeater.0.inner, init: &new_item, model: &model });
    }

    #[test]
    fn test_row_added_without_rows() {
        let model = TestModel::new(5);
        let repeater = repeater_with_window(&model, 0..5, 2);
        let before = state(&repeater);
        // Before:
        //          0, 1, 2, 3, 4
        //                ^
        // Window: |-------------|
        // After:
        //          0, 1, 2, 3, 4
        //                ^
        // Window: |-------------|
        model.insert(3, &[]);
        assert_eq!(state(&repeater), before);
        assert!(!repeater.as_ref().data().project_ref().is_dirty.get());
    }

    #[test]
    fn test_row_added_before_window() {
        let model = TestModel::new(10);
        let repeater = repeater_with_window(&model, 4..7, 5);
        // Before:
        //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        //                         ^
        // Window:              |------|
        // After:
        //          0, 1, 100, 2, 3, 4, 5, 6, 7, 8, 9
        //                              ^
        // Window:                  |------|
        model.insert(2, &[100]);
        assert_eq!(
            state(&repeater),
            (
                // They must be dirty, because the row changed
                vec![Dirty, Dirty, Dirty],
                ItemIndexRelationShip { row: 6, instance_index: 1 }
            )
        );
    }

    #[test]
    fn test_row_added_at_window_start() {
        let model = TestModel::new(10);
        let repeater = repeater_with_window(&model, 2..6, 4);
        // Before:
        //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        //                      ^
        // Window:       |----------|
        // After:
        //          0, 1, 100, 2, 3, 4, 5, 6, 7, 8, 9
        //                           ^
        // Window:        |--------------|
        model.insert(2, &[100]);
        assert_eq!(
            state(&repeater),
            (
                // The row changed for the item, so they get all dirty
                vec![Empty, Dirty, Dirty, Dirty, Dirty],
                ItemIndexRelationShip { row: 5, instance_index: 3 }
            ),
            "We must prepend one item"
        );
    }

    #[test]
    fn test_row_added_after_window() {
        let model = TestModel::new(10);
        let repeater = repeater_with_window(&model, 0..3, 0);
        let before = state(&repeater);
        // Before:
        //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        //          ^
        // Window:  |------|
        // After:
        //          0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 100
        //          ^
        // Window:  |------|
        model.insert(10, &[100]);
        assert_eq!(
            state(&repeater),
            before,
            "State must not change because it is after the window and that would lead to a splitted window"
        );
    }

    #[test]
    fn test_row_added_after_current_row() {
        let model = Rc::new(VecModel::from(vec![10, 20]));
        let repeater = repeater_for(&model);
        // Before:
        //          10, 20
        //           ^
        // Window: |-----|
        // After:
        //          5, 10, 20
        //             ^
        // Window: |---------|
        model.insert(0, 5);
        repeater.as_ref().ensure_updated(new_item);
        // Before:
        //          5, 10, 20
        //             ^
        // Window: |---------|
        // After:
        //          5, 10, 20, 30
        //             ^
        // Window: |-------------|
        model.push(30);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(5), Clean(10), Clean(20), Empty],
                ItemIndexRelationShip { row: 1, instance_index: 1 }
            )
        );
    }

    #[test]
    fn test_row_added_at_current_row_keeps_earlier_instances_clean() {
        let model = Rc::new(VecModel::from(vec![10, 20]));
        let repeater = repeater_for(&model);
        // Before:
        //          10, 20
        //           ^
        // Window: |-----|
        // After:
        //          5, 10, 20
        //             ^
        // Window: |---------|
        model.insert(0, 5);
        repeater.as_ref().ensure_updated(new_item);
        // Before:
        //          5, 10, 20
        //             ^
        // Window: |---------|
        // After:
        //          5, 7, 10, 20
        //                ^
        // Window: |------------|
        model.insert(1, 7);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(5), Empty, Dirty, Dirty],
                ItemIndexRelationShip { row: 2, instance_index: 2 }
            )
        );
    }

    #[test]
    fn test_rows_added_after_reset() {
        let model = Rc::new(VecModel::from(vec![10, 20]));
        let repeater = repeater_for(&model);
        // Before:
        //          10, 20
        //           ^
        // Window: |-----|
        // After:
        //          1, 2, 3
        //          ^
        // Window: |-------|
        model.set_vec(vec![1, 2, 3]);
        assert_eq!(state(&repeater), (vec![], ItemIndexRelationShip { row: 0, instance_index: 0 }));

        repeater.as_ref().ensure_updated(new_item);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(1), Clean(2), Clean(3)],
                ItemIndexRelationShip { row: 0, instance_index: 0 }
            )
        );
        model.push(4);
        model.push(5);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(1), Clean(2), Clean(3), Empty, Empty],
                ItemIndexRelationShip { row: 0, instance_index: 0 }
            )
        );
        ensure_updated_window(&repeater);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(1), Clean(2), Clean(3), Clean(4), Clean(5)],
                ItemIndexRelationShip { row: 0, instance_index: 0 }
            )
        );
    }

    #[test]
    fn test_row_removed_without_rows() {
        let model = TestModel::new(5);
        let repeater = repeater_with_window(&model, 0..5, 2);
        let before = state(&repeater);
        // Before:
        //           0, 1, 2, 3, 4
        //                 ^
        // Window:  |-------------|
        // After:
        //           0, 1, 2, 3, 4
        //                 ^
        // Window:  |-------------|
        model.remove(1..1);
        assert_eq!(state(&repeater), before);
        assert!(!repeater.as_ref().data().project_ref().is_dirty.get());
    }

    #[test]
    fn test_row_removed_after_current_row() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 3);
        let before = state(&repeater);
        // Before:
        //           0, 1, 2, 3, 4, 5, 6, 7
        //                    ^
        // Window:        |----------|
        // After:
        //           0, 1, 2, 3, 5, 6, 7
        //                    ^
        // Window:        |-------|
        model.remove(4..5);
        // Last one is dirty, because the row of it changed
        assert_eq!(state(&repeater), (vec![Clean(2), Clean(3), Dirty], before.1));
    }

    #[test]
    fn test_rows_removed_across_window_end() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 3);
        let before = state(&repeater);
        // Before:
        //           0, 1, 2, 3, 4, 5, 6, 7
        //                    ^
        // Window:        |----------|
        // After:
        //           0, 1, 2, 3, 4, 7
        //                    ^
        // Window:        |-------|
        model.remove(5..7);
        assert_eq!(state(&repeater), (vec![Clean(2), Clean(3), Clean(4)], before.1));
    }

    #[test]
    fn test_rows_removed_after_window() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 3);
        let before = state(&repeater);
        // Before:
        //           0, 1, 2, 3, 4, 5, 6, 7
        //                    ^
        // Window:        |----------|
        // After:
        //           0, 1, 2, 3, 4, 5
        //                    ^
        // Window:        |----------|
        model.remove(6..8);
        assert_eq!(state(&repeater), before);
    }

    #[test]
    fn test_rows_removed_before_current_row_but_inside_window() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..7, 5);
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                        ^
        // Window:       |------------|
        // After:
        //         0, 1, 2, 5, 6, 7
        //                  ^
        // Window:       |------|
        model.remove(3..5);
        // The second and the third must be dirty because their row changed
        assert_eq!(
            state(&repeater),
            (vec![Clean(2), Dirty, Dirty], ItemIndexRelationShip { row: 3, instance_index: 1 })
        );
        ensure_updated_window(&repeater);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(2), Clean(5), Clean(6)],
                ItemIndexRelationShip { row: 3, instance_index: 1 }
            )
        );
    }

    #[test]
    fn test_rows_removed_across_window_start() {
        let model = TestModel::new(10);
        let repeater = repeater_with_window(&model, 4..8, 6);
        // Before:
        // 0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        //                   ^
        // Window:     |--------|
        // After:
        // 0, 1, 2, 5, 6, 7, 8, 9
        //             ^
        // Window: |------|
        model.remove(3..5);
        assert_eq!(
            state(&repeater),
            (vec![Dirty, Dirty, Dirty], ItemIndexRelationShip { row: 4, instance_index: 1 })
        );
        ensure_updated_window(&repeater);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(5), Clean(6), Clean(7)],
                ItemIndexRelationShip { row: 4, instance_index: 1 }
            )
        );
    }

    #[test]
    fn test_row_removed_current_row() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 3);
        // Before:
        //   0, 1, 2, 3, 4, 5, 6, 7
        //            ^
        // Window: |--------|
        // After:
        //   0, 1, 2, 4, 5, 6, 7
        //            ^
        // Window: |-----|
        model.remove(3..4);
        assert_eq!(
            state(&repeater),
            (vec![Clean(2), Dirty, Dirty], ItemIndexRelationShip { row: 3, instance_index: 1 })
        );
        ensure_updated_window(&repeater);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(2), Clean(4), Clean(5)],
                ItemIndexRelationShip { row: 3, instance_index: 1 }
            )
        );
    }

    #[test]
    fn test_rows_removed_across_window_start_and_current_row() {
        let model = TestModel::new(10);
        let repeater = repeater_with_window(&model, 4..8, 5);
        // Before:
        //   0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        //                  ^
        // Window:       |--------|
        // After:
        //   0, 1, 2, 6, 7, 8, 9
        //            ^
        // Window:   |----|
        model.remove(3..6);
        assert_eq!(
            state(&repeater),
            (vec![Dirty, Dirty], ItemIndexRelationShip { row: 3, instance_index: 0 })
        );
        ensure_updated_window(&repeater);
        assert_eq!(
            state(&repeater),
            (vec![Clean(6), Clean(7)], ItemIndexRelationShip { row: 3, instance_index: 0 })
        );
    }

    #[test]
    fn test_rows_removed_from_current_row_across_window_end() {
        let model = TestModel::new(10);
        let repeater = repeater_with_window(&model, 2..6, 4);
        // Before:
        //   0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        //               ^
        // Window: |-----------|
        // After:
        //   0, 1, 2, 3, 8, 9
        //            ^
        // Window: |---|
        model.remove(4..8);
        assert_eq!(
            state(&repeater),
            (vec![Clean(2), Clean(3)], ItemIndexRelationShip { row: 3, instance_index: 1 })
        );
    }

    #[test]
    fn test_all_rows_removed() {
        // Before:
        //         0, 1, 2, 3, 4
        //               ^
        // Window: |-----------|
        // After: empty
        // Window: empty
        let model = TestModel::new(5);
        let repeater = repeater_with_window(&model, 0..5, 2);
        model.remove(0..5);
        assert_eq!(state(&repeater), (vec![], ItemIndexRelationShip { row: 0, instance_index: 0 }));
    }

    #[test]
    fn test_current_row_removed_at_the_end() {
        // Newest first, capped at two rows
        let model = Rc::new(VecModel::from(vec![2, 1]));
        let repeater = repeater_for(&model);
        {
            let value = 3;
            // Example for value 3
            // Before:
            //         2, 1
            //         ^
            // Window: |--|
            // After:
            //         3, 2, 1
            //            ^
            // Window: |-----|
            model.insert(0, value);
            ensure_updated_window(&repeater);
            assert_eq!(
                state(&repeater),
                (
                    vec![Clean(value), Clean(value - 1), Clean(value - 2)],
                    ItemIndexRelationShip { row: 1, instance_index: 1 }
                )
            );
            // Before:
            //         3, 2, 1
            //            ^
            // Window: |-----|
            // After:
            //         3, 2
            //            ^
            // Window: |---|
            model.remove(2);
            ensure_updated_window(&repeater);
            assert_eq!(
                state(&repeater),
                (
                    vec![Clean(value), Clean(value - 1)],
                    ItemIndexRelationShip { row: 1, instance_index: 1 }
                )
            );
        }
        {
            let value = 4;
            // Example for value 4
            // Before:
            //         3, 2
            //            ^
            // Window: |--|
            // After:
            //         4, 3, 2
            //               ^
            // Window: |-----|
            model.insert(0, value);
            ensure_updated_window(&repeater);
            assert_eq!(
                state(&repeater),
                (
                    vec![Clean(value), Clean(value - 1), Clean(value - 2)],
                    ItemIndexRelationShip { row: 2, instance_index: 2 }
                )
            );
            // Before:
            //         4, 3, 2
            //               ^
            // Window: |-----|
            // After:
            //         4, 3 // behind 2 there are no elements so the direct previous gets the new active
            //            ^
            // Window: |---|
            model.remove(2);
            ensure_updated_window(&repeater);
            assert_eq!(
                state(&repeater),
                (
                    vec![Clean(value), Clean(value - 1)],
                    ItemIndexRelationShip { row: 1, instance_index: 1 }
                )
            );
        }
    }

    /// Remove first instance
    #[test]
    fn test_remove_first_instance() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 4);
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                     ^
        // Window:       |---------|
        // After:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                     ^
        // Window:         |-------|
        with_ops(&repeater, |ops| ops.remove_first(1));
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(3), Clean(4), Clean(5)],
                ItemIndexRelationShip { row: 4, instance_index: 1 }
            )
        );
    }

    #[test]
    fn test_prepend_instances() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 3);
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                  ^
        // Window:       |---------|
        // After:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                  ^
        // Window: |---------------|
        with_ops(&repeater, |ops| ops.prepend(2));
        assert_eq!(
            state(&repeater),
            (
                vec![Empty, Empty, Clean(2), Clean(3), Clean(4), Clean(5)],
                ItemIndexRelationShip { row: 3, instance_index: 3 }
            )
        );
    }

    #[test]
    fn test_push_instances() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..5, 3);
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                  ^
        // Window:       |------|
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                  ^
        // Window:       |------------|
        with_ops(&repeater, |ops| ops.push(2));
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(2), Clean(3), Clean(4), Empty, Empty],
                ItemIndexRelationShip { row: 3, instance_index: 1 }
            )
        );
        ensure_updated_window(&repeater);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(2), Clean(3), Clean(4), Clean(5), Clean(6)],
                ItemIndexRelationShip { row: 3, instance_index: 1 }
            )
        );
    }

    #[test]
    fn test_truncate_after_current_instance() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 3);
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                  ^
        // Window:       |---------|
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                  ^
        // Window:      |----|
        with_ops(&repeater, |ops| ops.truncate(2));
        assert_eq!(
            state(&repeater),
            (vec![Clean(2), Clean(3)], ItemIndexRelationShip { row: 3, instance_index: 1 })
        );
    }

    #[test]
    fn test_truncate_removing_current_instance() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 2..6, 4);
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                  ^
        // Window:       |---------|
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //               ^
        // Window:      |-|
        with_ops(&repeater, |ops| ops.truncate(1));
        assert_eq!(
            state(&repeater),
            (vec![Clean(2)], ItemIndexRelationShip { row: 2, instance_index: 0 }),
            "Moves to the last remaining instance"
        );
    }

    #[test]
    fn test_clear_instances() {
        let model = TestModel::new(5);
        let repeater = repeater_with_window(&model, 0..5, 2);
        with_ops(&repeater, |ops| ops.clear());
        assert_eq!(state(&repeater), (vec![], ItemIndexRelationShip { row: 2, instance_index: 0 }));
    }

    #[test]
    fn test_empty_instance() {
        let model = TestModel::new(50);
        let repeater = repeater_with_window(&model, 10..10, 10);
        with_ops(&repeater, |ops| ops.clear());
        assert_eq!(
            state(&repeater),
            (vec![], ItemIndexRelationShip { row: 10, instance_index: 0 })
        );

        repeater.0.inner.borrow_mut().set_current_row(25);
        assert_eq!(
            state(&repeater),
            (vec![], ItemIndexRelationShip { row: 25, instance_index: 0 })
        );
        ensure_updated_window(&repeater);
        assert_eq!(
            state(&repeater),
            (vec![], ItemIndexRelationShip { row: 25, instance_index: 0 })
        );
    }

    #[test]
    fn test_set_current_row_within_window() {
        let model = TestModel::new(5);
        let repeater = repeater_with_window(&model, 0..5, 4);
        let slots = state(&repeater).0;
        // Before:
        //         0, 1, 2, 3, 4
        //                     ^
        // Window:|-------------|
        // After:
        //         0, 1, 2, 3, 4
        //            ^
        // Window:|-------------|
        repeater.0.inner.borrow_mut().set_current_row(1);
        assert_eq!(state(&repeater), (slots, ItemIndexRelationShip { row: 1, instance_index: 1 }));
        let before = state(&repeater);
        repeater.0.inner.borrow_mut().set_current_row(1);
        assert_eq!(state(&repeater), before);
    }

    #[test]
    fn test_set_current_row_append() {
        let model = TestModel::new(8);
        let repeater = repeater_with_window(&model, 0..5, 4);
        // Before:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                     ^
        // Window:|-------------|
        // After:
        //         0, 1, 2, 3, 4, 5, 6, 7
        //                              ^
        // Window:|----------------------|
        repeater.0.inner.borrow_mut().set_current_row(7);
        assert_eq!(
            state(&repeater),
            (
                vec![Clean(0), Clean(1), Clean(2), Clean(3), Clean(4), Empty, Empty, Empty],
                ItemIndexRelationShip { row: 7, instance_index: 7 }
            )
        );
    }

    #[test]
    fn test_instance_index_of_row() {
        let current = ItemIndexRelationShip { row: 5, instance_index: 2 };
        assert_eq!(current.get_instance_index_opt(7), Some(4));
        assert_eq!(current.get_instance_index_opt(5), Some(2));
        assert_eq!(current.get_instance_index_opt(3), Some(0));
        assert_eq!(current.get_instance_index_opt(2), None);
        assert_eq!(current.get_instance_index(7), 4);
        assert_eq!(
            current.get_instance_index(2),
            usize::MAX,
            "We are wrapping around. In real this should never happen"
        );
    }

    #[test]
    fn test_row_data_changed_after_insert_at_front() {
        let model = Rc::new(VecModel::from(vec![10, 20]));
        let repeater = repeater_for(&model);
        model.insert(0, 5);
        ensure_updated_window(&repeater);
        model.set_row_data(0, 6);
        model.set_row_data(2, 30);
        assert_eq!(state(&repeater).0, [Clean(6), Clean(10), Clean(30)]);
    }

    // #[test]
    // fn test_instance_lookup_after_insert_at_front() {
    //     let model = Rc::new(VecModel::from(vec![10, 20]));
    //     let repeater = repeater_for(&model);
    //     model.insert(0, 5);
    //     ensure_updated_window(&repeater);
    //     assert_eq!(repeater.range(), 0..3);
    //     assert_eq!(repeater.instance_at(0).map(|instance| instance.value.get()), Some(5));
    //     let mut rows = Vec::new();
    //     repeater.as_ref().for_each_instance_z(&mut |row, _| rows.push(row));
    //     assert_eq!(rows, [0, 1, 2]);
    // }
    //
    #[test]
    fn test_listview_scroll() {
        let model = TestModel::new(100000);
        let repeater = Box::pin(Repeater::<SimpleItem>::default());
        let model_rc = ModelRc::from(model.clone());
        repeater.set_model_binding(move || model_rc.clone());
        let content_y = Box::pin(Property::new(LogicalLength::default()));
        let listview_height = Box::pin(Property::new(LogicalLength::new(ITEM_HEIGHT * 3 as Coord)));
        let update_listview = || {
            repeater.as_ref().ensure_updated_listview(
                new_item,
                None,
                None,
                content_y.as_ref(),
                LogicalLength::new(100 as Coord),
                listview_height.as_ref(),
            );
        };

        update_listview();
        assert_eq!(state(&repeater).0, [Clean(0), Clean(1), Clean(2)]);

        content_y.as_ref().set(LogicalLength::new(-ITEM_HEIGHT * 5 as Coord));
        update_listview();
        // Scrolled by 5 rows
        // 3 and 4 are kept
        assert_eq!(state(&repeater).0, [Clean(3), Clean(4), Clean(5), Clean(6), Clean(7)]);

        content_y.as_ref().set(LogicalLength::new(ITEM_HEIGHT * 3 as Coord));

        assert_eq!(
            state(&repeater).0,
            [Clean(2), Clean(3), Clean(4), Clean(5), Clean(6), Clean(7)]
        );
    }

    // #[test]
    // fn test_listview_scrolled_after_insert_at_front() {
    //     let model = TestModel::new(10);
    //     let repeater = Box::pin(Repeater::<SimpleItem>::default());
    //     let model_rc = ModelRc::from(model.clone());
    //     repeater.set_model_binding(move || model_rc.clone());
    //     let content_y = Box::pin(Property::new(LogicalLength::default()));
    //     let listview_height = Box::pin(Property::new(LogicalLength::new(ITEM_HEIGHT * 3 as Coord)));
    //     let update = || {
    //         repeater.as_ref().ensure_updated_listview(
    //             new_item,
    //             None,
    //             None,
    //             content_y.as_ref(),
    //             LogicalLength::new(100 as Coord),
    //             listview_height.as_ref(),
    //         );
    //     };

    //     update();
    //     model.insert(0, &[100]);
    //     update();
    //     // Scrolled by 3.5 rows, so rows 3 to 6 are in view
    //     content_y.as_ref().set(LogicalLength::new(-ITEM_HEIGHT * 7 as Coord / 2 as Coord));
    //     update();
    //     assert_eq!(state(&repeater).0, [Clean(2), Clean(3), Clean(4), Clean(5)]);
    // }

    // Trait implementations
    // ###############################################################################

    impl crate::items::Item for SimpleItem {
        fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

        fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

        fn layout_info(
            self: Pin<&Self>,
            _orientation: Orientation,
            _cross_axis_constraint: Coord,
            _window_adapter: &Rc<dyn WindowAdapter>,
            _self_rc: &ItemRc,
        ) -> LayoutInfo {
            unimplemented!("Not implemented");
            LayoutInfo { stretch: 1., ..LayoutInfo::default() }
        }

        fn input_event_filter_before_children(
            self: Pin<&Self>,
            _: &MouseEvent,
            _window_adapter: &Rc<dyn WindowAdapter>,
            _self_rc: &ItemRc,
            _: &mut MouseCursorInner,
        ) -> InputEventFilterResult {
            unimplemented!("Not implemented");
            InputEventFilterResult::ForwardAndIgnore
        }

        fn input_event(
            self: Pin<&Self>,
            _: &MouseEvent,
            _window_adapter: &Rc<dyn WindowAdapter>,
            _self_rc: &ItemRc,
            _: &mut MouseCursorInner,
        ) -> InputEventResult {
            unimplemented!("Not implemented");
            InputEventResult::EventIgnored
        }

        fn capture_key_event(
            self: Pin<&Self>,
            _: &InternalKeyEvent,
            _window_adapter: &Rc<dyn WindowAdapter>,
            _self_rc: &ItemRc,
        ) -> KeyEventResult {
            unimplemented!("Not implemented");
            KeyEventResult::EventIgnored
        }

        fn key_event(
            self: Pin<&Self>,
            _: &InternalKeyEvent,
            _window_adapter: &Rc<dyn WindowAdapter>,
            _self_rc: &ItemRc,
        ) -> KeyEventResult {
            unimplemented!("Not implemented");
            KeyEventResult::EventIgnored
        }

        fn focus_event(
            self: Pin<&Self>,
            _: &FocusEvent,
            _window_adapter: &Rc<dyn WindowAdapter>,
            _self_rc: &ItemRc,
        ) -> FocusEventResult {
            unimplemented!("Not implemented");
            FocusEventResult::FocusIgnored
        }

        fn render(
            self: Pin<&Self>,
            backend: &mut ItemRendererRef,
            self_rc: &ItemRc,
            size: LogicalSize,
        ) -> RenderingResult {
            unimplemented!("Not implemented");
            RenderingResult::ContinueRenderingChildren
        }

        fn bounding_rect(
            self: core::pin::Pin<&Self>,
            _window_adapter: &Rc<dyn WindowAdapter>,
            _self_rc: &ItemRc,
            geometry: LogicalRect,
        ) -> LogicalRect {
            unimplemented!("Not implemented");
            geometry
        }

        fn clips_children(self: core::pin::Pin<&Self>) -> bool {
            unimplemented!("Not implemented");
            false
        }
    }

    impl ItemTree for SimpleItem {
        fn visit_children_item(
            self: core::pin::Pin<&Self>,
            _1: isize,
            _2: crate::item_tree::TraversalOrder,
            _3: vtable::VRefMut<crate::item_tree::ItemVisitorVTable>,
        ) -> crate::item_tree::VisitChildrenResult {
            unimplemented!("Not needed for this test")
        }

        fn get_item_ref(
            self: core::pin::Pin<&Self>,
            index: u32,
        ) -> core::pin::Pin<vtable::VRef<'_, crate::items::ItemVTable>> {
            unimplemented!("Not needed for this test")
        }

        fn get_item_tree(self: core::pin::Pin<&Self>) -> Slice<'_, ItemTreeNode> {
            unimplemented!("Not implemented");
            Slice::default()
        }

        fn parent_node(self: core::pin::Pin<&Self>, result: &mut ItemWeak) {
            unimplemented!("Not implemented");
            *result = ItemWeak::default();
        }

        fn embed_component(
            self: core::pin::Pin<&Self>,
            _parent_component: &ItemTreeWeak,
            _item_tree_index: u32,
        ) -> bool {
            unimplemented!("Not implemented");
            false
        }

        fn ensure_instantiated(mut self: core::pin::Pin<&Self>) -> bool {
            false
        }

        fn layout_info(self: core::pin::Pin<&Self>, o: Orientation) -> LayoutInfo {
            // return LayoutInfo {
            //     max: wi.width.get_internal().0,
            //     max_percent: 100.,
            //     min: wi.width.get_internal().0,
            //     min_percent: 100.,
            //     preferred: wi.width.get_internal().0,
            //     stretch: 1.,
            // };
            unimplemented!("Not needed for this test")
        }

        fn subtree_index(self: core::pin::Pin<&Self>) -> usize {
            unimplemented!("Not implemented");
            0
        }

        fn get_subtree_range(self: core::pin::Pin<&Self>, subtree_index: u32) -> IndexRange {
            unimplemented!("Not implemented");
            IndexRange { start: 0, end: 0 }
        }

        fn get_subtree(
            self: core::pin::Pin<&Self>,
            subtree_index: u32,
            component_index: usize,
            result: &mut ItemTreeWeak,
        ) {
            unimplemented!("Not implemented");
        }

        fn accessible_role(self: Pin<&Self>, _: u32) -> AccessibleRole {
            unimplemented!("Not needed for this test")
        }

        fn accessible_string_property(
            self: Pin<&Self>,
            _: u32,
            _: AccessibleStringProperty,
            _: &mut crate::SharedString,
        ) -> bool {
            unimplemented!("Not implemented");
            false
        }

        fn item_element_infos(self: Pin<&Self>, _: u32, _: &mut SharedString) -> bool {
            unimplemented!("Not implemented");
            false
        }

        fn window_adapter(
            self: Pin<&Self>,
            _do_create: bool,
            result: &mut Option<WindowAdapterRc>,
        ) {
            unimplemented!("Not implemented");
            *result = None;
        }

        fn item_geometry(self: Pin<&Self>, _: u32) -> crate::lengths::LogicalRect {
            crate::lengths::LogicalRect::new(
                euclid::Point2D::new(0 as Coord, 0 as Coord),
                euclid::Size2D::new(100 as Coord, ITEM_HEIGHT),
            )
        }

        fn accessibility_action(
            self: core::pin::Pin<&Self>,
            _: u32,
            _: &crate::accessibility::AccessibilityAction,
        ) {
            unimplemented!("Not needed for this test")
        }

        fn supported_accessibility_actions(
            self: core::pin::Pin<&Self>,
            _: u32,
        ) -> crate::accessibility::SupportedAccessibilityAction {
            unimplemented!("Not needed for this test")
        }
    }
}
