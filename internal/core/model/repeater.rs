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
use crate::api::LogicalPosition;
use crate::item_tree::{ItemTreeVTable, TraversalOrder};
use crate::items::{Flickable, ItemRc, ScrollBehavior};
use crate::layout::Orientation;
use crate::lengths::{LogicalLength, RectLengths};
use crate::model::repeater::scroll::accelerate_jump_decelerate::AccelerateJumpDecelerate;
use crate::{Coord, Property};
use alloc::vec::Vec;
use core::cell::RefCell;
use core::pin::Pin;
#[allow(unused)]
use euclid::num::Floor;
use pin_project::pin_project;

mod scroll;

type ItemTreeRc<C = vtable::Dyn> = vtable::VRc<crate::item_tree::ItemTreeVTable, C>;

/// `instance` as a `C`.
/// The instances of a `Repeater<C>` or a `Conditional<C>` are stored type-erased,
/// so that the code handling them isn't compiled again for each `C`.
fn as_typed<C: RepeatedItemTree>(instance: &ItemTreeRc) -> Pin<&C> {
    vtable::VRef::downcast_pin(vtable::VRc::borrow_pin(instance)).unwrap()
}

/// `instance` as an `ItemTreeRc<C>`, see [`as_typed`].
fn downcast<C: RepeatedItemTree>(instance: &ItemTreeRc) -> ItemTreeRc<C> {
    vtable::VRc::downcast(instance).unwrap()
}

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

    /// The row of the first instance when updating all `instance_count` instances.
    /// Outside a ListView the instances start at row 0.
    fn start_update_all(&mut self, instance_count: usize) -> usize {
        if instance_count == 0 {
            *self = Self::default();
        }
        self.first_row()
    }

    /// Updates the indexes after removing all instances.
    fn instances_cleared(&mut self) {
        self.instance_index = 0;
    }

    /// Updates the indexes before inserting `count` slots in front of `instance_count` instances.
    fn instances_prepended(&mut self, instance_count: usize, count: usize) {
        if instance_count > 0 {
            self.instance_index += count;
        }
    }

    /// Updates the indexes before removing the first `count` of `instance_count` instances.
    fn first_instances_removed(&mut self, instance_count: usize, count: usize) {
        if count >= instance_count {
            self.instances_cleared();
        } else if count > self.instance_index {
            // During a ListView update `row` lags behind the ListView's own copy of the layout
            // state, hence the saturating_sub
            self.row = self.row.saturating_sub(self.instance_index) + count;
            self.instance_index = 0;
        } else {
            self.instance_index -= count;
        }
    }

    /// Updates the indexes before keeping only the first `len` of `instance_count` instances.
    fn instances_truncated(&mut self, instance_count: usize, len: usize) {
        if len == 0 {
            self.instances_cleared();
        } else if len < instance_count && self.instance_index >= len {
            self.row = self.row.saturating_sub(self.instance_index) + len - 1;
            self.instance_index = len - 1;
        }
    }

    /// Updates the indexes for `count` rows inserted into the model at `index`,
    /// with `instance_count` instances.
    /// Returns where to insert empty slots, or `None` if the instances don't change.
    fn rows_added(
        &mut self,
        instance_count: usize,
        index: usize,
        count: usize,
    ) -> Option<InstanceSlots> {
        if count == 0 {
            return None;
        }
        let first_row = self.first_row();
        if index > first_row + instance_count {
            // A slot for these rows would split the window
            return None;
        }
        if index < first_row {
            self.row += count;
            return Some(InstanceSlots { position: 0, count: 0 });
        }
        if index <= self.row && instance_count > 0 {
            self.row += count;
            self.instance_index += count;
        }
        Some(InstanceSlots { position: index - first_row, count })
    }

    /// Updates the indexes for `count` rows removed from the model at `index`,
    /// with `instance_count` instances and `row_count` rows left.
    /// Returns the instances to remove, or `None` if the instances don't change.
    fn rows_removed(
        &mut self,
        instance_count: usize,
        index: usize,
        count: usize,
        row_count: usize,
    ) -> Option<core::ops::Range<usize>> {
        if count == 0 {
            return None;
        }
        let first_row = self.first_row();
        let end_row = first_row + instance_count;
        let removed_end = index + count;
        if instance_count == 0 {
            if removed_end <= self.row {
                self.row -= count;
            } else if index <= self.row {
                self.row = index.min(row_count.saturating_sub(1));
            }
            return None;
        }
        if index >= end_row {
            return None;
        }
        let start = index.clamp(first_row, end_row) - first_row;
        let end = removed_end.clamp(first_row, end_row) - first_row;
        let remaining = instance_count - (end - start);
        if removed_end <= self.row {
            self.row -= count;
            self.instance_index -= end - start;
        } else if index <= self.row {
            // The current item moves to the next remaining instance, or else to the previous one
            *self = if start < remaining {
                Self { row: index, instance_index: start }
            } else if start > 0 {
                Self { row: first_row + start - 1, instance_index: start - 1 }
            } else {
                Self { row: index.min(row_count.saturating_sub(1)), instance_index: 0 }
            };
        }
        Some(start..end)
    }
}

/// Empty slots to insert for rows added to the model.
/// The instances from `position` on are dirty.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct InstanceSlots {
    /// The index of the first new slot.
    pub position: usize,
    /// The number of new slots.
    pub count: usize,
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
#[derive(Default)]
struct RepeaterInner {
    instances: Vec<(RepeatedInstanceState, Option<ItemTreeRc>)>,
    /// ListView-specific layout state (offset, cached heights, scroll position).
    layout_state: RepeaterLayoutState,
    scroll_to_row: Option<scroll::ScrollToRowRequest>,
}

#[cfg(test)]
impl RepeaterInner {
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
///
/// With an ongoing scroll request, a layout pass also:
/// - plans the smooth scroll, including whether to jump and where the jump lands,
/// - advances the request to its next phase.
fn update_visible_instances(
    ops: &mut impl RepeaterInstanceOps,
    state: &mut RepeaterLayoutState,
    scroll_to_row: &mut Option<scroll::ScrollToRowRequest>,
    row_count: usize,
    props: &dyn ListViewProperties,
    listview_width: LogicalLength,
    listview_height: LogicalLength,
) -> bool {
    let zero = LogicalLength::default();
    let mut content_width_value = listview_width.get();
    let listview_height = listview_height.get();

    if row_count == 0 {
        *scroll_to_row = None;
        ops.clear();
        state.item_index.instances_cleared();
        props.content_height_set(zero);
        props.content_y_set(zero);
        props.content_width_set(listview_width);
        return false;
    }
    // A height of zero or less, like before the window has a size, leaves no viewport to fill.
    // Unlike with an empty model, the content geometry stays, so that the scroll position does too.
    if listview_height <= 0 as Coord {
        ops.clear();
        state.item_index.instances_cleared();
        return false;
    }
    if scroll_to_row
        .as_ref()
        .is_some_and(|request| request.row >= row_count || request.is_stopped())
    {
        *scroll_to_row = None;
    }

    let mut content_y_value = props.content_y_get().get();
    if !props.content_y_has_binding() {
        content_y_value = content_y_value.min(0 as Coord);
    }

    let scroll::SmoothScrollLayout { jump, extra_above, layout_end } = scroll::smooth_scroll_layout(
        scroll_to_row.as_ref(),
        state,
        ops,
        content_y_value,
        listview_height,
    );

    let mut changed = false;

    if state.item_index.row >= row_count {
        ops.clear();
        state.item_index = ItemIndexRelationShip { row: row_count - 1, instance_index: 0 };
    }
    // `anchor_y` is the top of the item at `anchor_row`, the instance at `anchor_instance`.
    let ItemIndexRelationShip { row: anchor_row, instance_index: mut anchor_instance } =
        state.item_index;
    let mut first_row = anchor_row - anchor_instance;

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
            ops.clear();
            ops.push(1);
            first_row = anchor_row;
            anchor_instance = 0;
            changed |= ops.ensure_updated(0, anchor_row);
            ops.height(0).unwrap_or(0 as Coord)
        }
    };

    let one_and_a_half_screen = listview_height * 3 as Coord / 2 as Coord;
    let first_item_y = state.anchor_y - element_height * anchor_instance as Coord;
    let last_item_bottom = first_item_y + element_height * ops.len() as Coord;

    // The row to seek to, its edge, and its distance beyond that edge of the viewport.
    let seek = scroll_to_row.as_ref().and_then(|request| match (&request.animation, jump) {
        (scroll::ScrollAnimation::Instant(alignment), _) => {
            Some((request.row, *alignment, 0 as Coord))
        }
        (_, Some((alignment, beyond_edge))) => Some((request.row, alignment, beyond_edge)),
        _ => None,
    });
    let (mut new_row, mut new_offset_y) = if let Some((row, alignment, beyond_edge)) = seek {
        // Check if the row is around the already existing instances
        let first = first_row;
        let end = first + ops.len();
        let added = if (first..end).contains(&row) {
            // The row exists in the instantiated instances
            0..0
        } else if row >= end && row - end < ops.len() {
            // The row is within the next ops.len() (about one page) instances
            // so we just append
            ops.push(row + 1 - end);
            end - first..ops.len()
        } else if row < first && first - row <= ops.len() {
            // The row is within the previous ops.len() (about one page) instances
            ops.prepend(first - row);
            first_row = row;
            0..first - row
        } else {
            // Random seek, so we clear all instances and create new ones
            ops.clear();
            ops.push(1);
            first_row = row;
            0..1
        };
        // The loop measures the rows between the old instances and `row` without creating them.
        for i in added {
            changed |= ops.ensure_updated(i, first_row + i);
        }
        let instance = row - first_row;
        changed |= ops.ensure_updated(instance, row);
        let row_height = ops.height(instance).unwrap_or(element_height);
        let row_top = match alignment {
            scroll::RowAlignment::Top => -beyond_edge,
            scroll::RowAlignment::Bottom => {
                // If the row is taller than listview_height, then we ignore the
                // alignment and align to top
                (listview_height - row_height).max(0. as Coord) + beyond_edge
            }
        };
        (row, row_top)
    } else if first_item_y > -content_y_value + one_and_a_half_screen
        || (props.computes_content_height() && last_item_bottom + element_height < -content_y_value)
    {
        // Jumping more than 1.5 screens: random seek.
        ops.clear();
        first_row = ((-content_y_value / element_height).floor() as usize).min(row_count - 1);
        (first_row, content_y_value + element_height * first_row as Coord)
    } else if content_y_value < state.previous_content_y {
        // Scrolled down: find the new offset by walking existing instances.
        let mut it_y = state.anchor_y + content_y_value;
        let mut new_row = anchor_row;
        for i in anchor_instance..ops.len() {
            changed |= ops.ensure_updated(i, new_row);
            let h = ops.height(i).unwrap_or(0 as Coord);
            if it_y + h > 0 as Coord || new_row + 1 >= row_count {
                break;
            }
            it_y += h;
            new_row += 1;
        }
        (new_row, it_y)
    } else {
        // Scrolled up: will instantiate items before offset in the loop below.
        (anchor_row, state.anchor_y + content_y_value)
    };

    let target_row = scroll_to_row.as_ref().map(|request| request.row);
    let mut target_span: Option<(Coord, Coord)>;
    let mut loop_count = 0;
    loop {
        target_span = None;
        // Fill gap before new_row using already-instantiated items.
        while new_row > first_row && new_offset_y > -extra_above {
            new_row -= 1;
            changed |= ops.ensure_updated(new_row - first_row, new_row);
            new_offset_y -= ops.height(new_row - first_row).unwrap_or(0 as Coord);
        }
        // If there is still a gap, create new instances before the current ones.
        let mut prepend_count = 0;
        while new_row > 0 && new_offset_y > -extra_above {
            new_row -= 1;
            ops.prepend(1);
            changed |= ops.ensure_updated(0, new_row);
            new_offset_y -= ops.height(0).unwrap_or(0 as Coord);
            prepend_count += 1;
        }
        if prepend_count > 0 {
            first_row = new_row;
        }
        if seek.is_some() && new_row == 0 {
            new_offset_y = new_offset_y.min(0 as Coord);
        }
        debug_assert!(new_row >= first_row && new_row <= first_row + ops.len());

        // Layout items until we fill the view, starting with already-instantiated ones.
        let mut y = new_offset_y;
        let mut idx = new_row;
        let instances_begin = new_row - first_row;
        for i in instances_begin..ops.len() {
            if idx >= row_count {
                break;
            }
            changed |= ops.ensure_updated(i, idx);
            let top = y;
            content_width_value = content_width_value.max(ops.listview_layout(i, &mut y));
            if target_row == Some(idx) {
                target_span = Some((top, y));
            }
            idx += 1;
            if y >= layout_end {
                break;
            }
        }

        // Create more items until there is no more room.
        while y < layout_end && idx < row_count {
            let i = ops.len();
            ops.push(1);
            changed |= ops.ensure_updated(i, idx);
            let top = y;
            content_width_value = content_width_value.max(ops.listview_layout(i, &mut y));
            if target_row == Some(idx) {
                target_span = Some((top, y));
            }
            idx += 1;
        }

        // Skip the leading rows that end above the kept area, so cleanup drops them.
        while new_row + 1 < idx {
            let bottom = new_offset_y + ops.height(new_row - first_row).unwrap_or(0 as Coord);
            // A row with zero height at the edge isn't above it.
            if bottom > -extra_above || new_offset_y >= -extra_above {
                break;
            }
            new_row += 1;
            new_offset_y = bottom;
        }

        if seek.is_some()
            && y < listview_height
            && (new_row > 0 || new_offset_y < 0 as Coord)
            && loop_count < 3
        {
            // A seek that leaves room below the last row moves all rows down to close the gap.
            new_offset_y += listview_height - y;
            loop_count += 1;
            continue;
        }

        if y < listview_height && content_y_value < 0 as Coord && loop_count < 3 {
            debug_assert!(idx >= row_count);
            // Reached end of model with room to spare. Scroll up.
            content_y_value += listview_height - y;
            loop_count += 1;
            continue;
        }

        // Clean up instances that are not shown.
        if new_row != first_row {
            ops.remove_first(new_row - first_row);
            first_row = new_row;
        }
        let keep = idx - new_row;
        ops.truncate(keep);
        state.item_index = ItemIndexRelationShip { row: first_row, instance_index: 0 };

        if ops.len() == 0 {
            break;
        }

        // Recompute coordinates for the scrollbar.
        state.cached_item_height = (y - new_offset_y) / ops.len() as Coord;
        state.anchor_y = state.cached_item_height * first_row as Coord;
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

        if let Some(request) = scroll_to_row.take() {
            let (top, height) =
                target_span.map(|(top, bottom)| (top, bottom - top)).unwrap_or_else(|| {
                    let heights =
                        (0..ops.len()).map(|i| ops.height(i).unwrap_or(state.cached_item_height));
                    let (top, height) = scroll::row_geometry(state, heights, request.row);
                    (top + new_content_y, height)
                });
            *scroll_to_row = scroll::advance_scroll_to_row(
                request,
                top,
                height,
                listview_height,
                jump.is_some(),
            );
        }

        break;
    }

    changed
}

fn empty_slots(count: usize) -> impl Iterator<Item = (RepeatedInstanceState, Option<ItemTreeRc>)> {
    core::iter::repeat_with(|| (RepeatedInstanceState::Dirty, None)).take(count)
}

/// The operations of [`RustRepeaterOps`] that depend on the type of the instances.
trait TypedInstanceOps {
    fn create(&self) -> ItemTreeRc;
    fn update(&self, instance: &ItemTreeRc, row: usize);
    fn init(&self, instance: &ItemTreeRc);
    fn listview_layout(&self, instance: &ItemTreeRc, offset_y: &mut LogicalLength)
    -> LogicalLength;
}

struct RepeaterTypedOps<'a, C: RepeatedItemTree> {
    init: &'a dyn Fn() -> ItemTreeRc<C>,
    model: &'a ModelRc<C::Data>,
}

impl<C: RepeatedItemTree> TypedInstanceOps for RepeaterTypedOps<'_, C> {
    fn create(&self) -> ItemTreeRc {
        vtable::VRc::into_dyn((self.init)())
    }
    fn update(&self, instance: &ItemTreeRc, row: usize) {
        as_typed::<C>(instance).update(row, self.model.row_data(row).unwrap_or_default());
    }
    fn init(&self, instance: &ItemTreeRc) {
        as_typed::<C>(instance).init();
    }
    fn listview_layout(
        &self,
        instance: &ItemTreeRc,
        offset_y: &mut LogicalLength,
    ) -> LogicalLength {
        as_typed::<C>(instance).listview_layout(offset_y)
    }
}

fn instance_height(instance: &ItemTreeRc) -> Coord {
    vtable::VRc::borrow_pin(instance).as_ref().item_geometry(0).height_length().get()
}

/// Adapter implementing [`RepeaterInstanceOps`] for the native Rust repeater.
struct RustRepeaterOps<'a> {
    inner: &'a RefCell<RepeaterInner>,
    typed: &'a dyn TypedInstanceOps,
    instance_generation: Pin<&'a Property<()>>,
}

impl RepeaterInstanceOps for RustRepeaterOps<'_> {
    fn len(&self) -> usize {
        self.inner.borrow().instances.len()
    }

    fn clear(&mut self) {
        let mut inner = self.inner.borrow_mut();
        inner.instances.clear();
        inner.layout_state.item_index.instances_cleared();
    }

    fn push(&mut self, count: usize) {
        self.inner.borrow_mut().instances.extend(empty_slots(count));
    }

    fn prepend(&mut self, count: usize) {
        let mut inner = self.inner.borrow_mut();
        let instance_count = inner.instances.len();
        inner.layout_state.item_index.instances_prepended(instance_count, count);
        inner.instances.splice(0..0, empty_slots(count));
    }

    fn remove_first(&mut self, count: usize) {
        let mut inner = self.inner.borrow_mut();
        let instance_count = inner.instances.len();
        inner.layout_state.item_index.first_instances_removed(instance_count, count);
        inner.instances.drain(..count.min(instance_count));
    }

    fn truncate(&mut self, len: usize) {
        let mut inner = self.inner.borrow_mut();
        let instance_count = inner.instances.len();
        inner.layout_state.item_index.instances_truncated(instance_count, len);
        inner.instances.truncate(len);
    }

    fn ensure_updated(&mut self, instance_idx: usize, row: usize) -> bool {
        let (created, instance) = {
            let mut inner = self.inner.borrow_mut();
            let c = &mut inner.instances[instance_idx];
            if c.0 != RepeatedInstanceState::Dirty {
                return false;
            }
            let created = c.1.is_none();
            let instance = c.1.get_or_insert_with(|| self.typed.create());
            self.typed.update(instance, row);
            c.0 = RepeatedInstanceState::Clean;
            (created, instance.clone())
        };
        if created {
            // #13723
            self.instance_generation.mark_dirty();
            crate::properties::evaluate_no_tracking(|| self.typed.init(&instance));
        }
        crate::item_tree::ensure_item_tree_instantiated(&instance);
        created
    }

    fn height(&self, instance_idx: usize) -> Option<Coord> {
        self.inner.borrow().instances[instance_idx].1.as_ref().map(instance_height)
    }

    fn listview_layout(&self, instance_idx: usize, y: &mut Coord) -> Coord {
        let inner = self.inner.borrow();
        let mut y_len = LogicalLength::new(*y);
        let w = self
            .typed
            .listview_layout(inner.instances[instance_idx].1.as_ref().unwrap(), &mut y_len);
        *y = y_len.get();
        w.get()
    }
}

impl RepeaterInner {
    /// Implements [`ModelChangeListener::row_added`].
    /// Returns whether the instances changed.
    fn row_added(&mut self, index: usize, count: usize) -> bool {
        let Some(slots) =
            self.layout_state.item_index.rows_added(self.instances.len(), index, count)
        else {
            return false;
        };
        self.instances.splice(slots.position..slots.position, empty_slots(slots.count));
        for c in self.instances[slots.position..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedInstanceState::Dirty;
        }
        true
    }

    /// Implements [`ModelChangeListener::row_removed`].
    /// `row_count` is the number of rows after the removal.
    /// Returns whether the instances changed.
    fn row_removed(&mut self, index: usize, count: usize, row_count: usize) -> bool {
        let Some(removed) = self.layout_state.item_index.rows_removed(
            self.instances.len(),
            index,
            count,
            row_count,
        ) else {
            return false;
        };
        let start = removed.start;
        self.instances.drain(removed);
        for c in self.instances[start..].iter_mut() {
            // Because all the indexes are dirty
            c.0 = RepeatedInstanceState::Dirty;
        }
        true
    }
}

/// This struct is put in a component when using the `for` syntax
/// It helps instantiating the ItemTree `T`
#[pin_project]
pub struct RepeaterTracker<T: RepeatedItemTree> {
    inner: RefCell<RepeaterInner>,
    #[pin]
    model: Property<ModelRc<T::Data>>,
    #[pin]
    /// Set to true when the model becomes dirty.
    is_dirty: Property<bool>,
    #[pin]
    /// Marked dirty whenever `instances` changes, including when `ensure_updated` creates one.
    /// A row's data change alone doesn't mark it.
    instance_generation: Property<()>,
    /// Only used for the list view to track if the scrollbar has changed and item needs to be laid out again.
    #[pin]
    listview_geometry_tracker: crate::properties::PropertyTracker,
}

impl<T: RepeatedItemTree> RepeaterTracker<T> {
    /// Call whenever `instances` changes outside of `ensure_updated`.
    fn instances_changed(self: Pin<&Self>) {
        self.is_dirty.set(true);
        self.project_ref().instance_generation.mark_dirty();
    }
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
                    as_typed::<T>(comp).update(row, model.row_data(row).unwrap_or_default());
                    c.0 = RepeatedInstanceState::Clean;
                }
            } else {
                c.0 = RepeatedInstanceState::Dirty;
            }
        }
    }
    /// Notify the peers that rows were added
    fn row_added(self: Pin<&Self>, index: usize, count: usize) {
        if self.inner.borrow_mut().row_added(index, count) {
            self.instances_changed();
        }
    }
    /// Notify the peers that rows were removed
    fn row_removed(self: Pin<&Self>, index: usize, count: usize) {
        let row_count = self.model.get_internal().row_count();
        if self.inner.borrow_mut().row_removed(index, count, row_count) {
            self.instances_changed();
        }
    }

    fn reset(self: Pin<&Self>) {
        self.instances_changed();
        let mut inner = self.inner.borrow_mut();
        inner.instances.clear();
        inner.layout_state.item_index.instances_cleared();
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

/// Implements [`Repeater::listview_ensure_row_visible`] for the Rust and the C++ repeater.
/// `heights` are the heights of the instances from the first instance on.
/// Returns whether the ListView needs a layout pass.
fn listview_ensure_row_visible(
    state: &RepeaterLayoutState,
    heights: &[Coord],
    scroll_to_row: &mut Option<scroll::ScrollToRowRequest>,
    row_count: usize,
    row: i32,
    behavior: ScrollBehavior,
    flickable_rc: &ItemRc,
) -> bool {
    let Some(row) = usize::try_from(row).ok().filter(|row| *row < row_count) else {
        return false;
    };
    let Some(flickable) = flickable_rc.downcast::<Flickable>() else {
        return false;
    };
    let behavior =
        if Flickable::reduced_motion(flickable_rc) { ScrollBehavior::Instant } else { behavior };
    let flickable = flickable.as_pin_ref();
    let content_y = flickable.content_y();
    let viewport_height = flickable_rc.geometry().height();
    let row_in_viewport =
        (!heights.is_empty() && state.cached_item_height > 0 as Coord).then(|| {
            let (top_anchor, height) = scroll::row_geometry(state, heights.iter().copied(), row);
            (top_anchor + content_y.get(), height)
        });
    // an instant scroll pulls an overscroll back in bounds.
    let stop_scrolling = || {
        flickable.scroll_to(
            flickable_rc,
            LogicalPosition::new(-flickable.content_x().get() as f32, -content_y.get() as f32),
            ScrollBehavior::Instant,
        )
    };

    let animation = match (row_in_viewport, behavior) {
        (None, ScrollBehavior::Instant) => {
            stop_scrolling();
            scroll::ScrollAnimation::Instant(scroll::RowAlignment::Bottom)
        }
        (None, ScrollBehavior::Smooth) => {
            scroll::ScrollAnimation::AccelerateJumpDecelerate(AccelerateJumpDecelerate::new())
        }
        (Some((top_anchor, row_height)), behavior) => {
            let Some(alignment) =
                scroll::RowAlignment::for_row(top_anchor, row_height, viewport_height)
            else {
                // The row is already in view, so we don't have to scroll. Stop any scrolling
                *scroll_to_row = None;
                stop_scrolling();
                return false;
            };
            let distance = alignment.distance(top_anchor, row_height, viewport_height);
            match behavior {
                ScrollBehavior::Instant => {
                    flickable.scroll_to(
                        flickable_rc,
                        LogicalPosition::new(
                            -flickable.content_x().get() as f32,
                            -(content_y.get() + distance) as f32,
                        ),
                        ScrollBehavior::Instant,
                    );
                    scroll::ScrollAnimation::Instant(alignment)
                }
                ScrollBehavior::Smooth => scroll::ScrollAnimation::AccelerateJumpDecelerate(
                    AccelerateJumpDecelerate::new(),
                ),
            }
        }
    };
    *scroll_to_row =
        Some(scroll::ScrollToRowRequest { row, flickable: flickable_rc.downgrade(), animation });
    true
}

impl<C: RepeatedItemTree + 'static> Repeater<C> {
    /// Scrolls the ListView that the Flickable `flickable_rc` belongs to, so that `row` is fully visible.
    /// With `ScrollBehavior::Smooth`, it animates the scroll.
    ///
    /// A row above the viewport, or one taller than it, ends up at the top edge;
    /// any other row ends up at the bottom edge.
    /// A row that is already fully visible stops a running scroll.
    /// An out-of-range row does nothing.
    pub fn listview_ensure_row_visible(
        self: Pin<&Self>,
        row: i32,
        behavior: ScrollBehavior,
        flickable_rc: &ItemRc,
    ) {
        let row_count = self.model().row_count();
        if listview_ensure_instance_row_visible(
            &self.0.inner,
            row_count,
            row,
            behavior,
            flickable_rc,
        ) {
            self.data().is_dirty.set(true);
        }
    }

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
    /// tracking scope. This is for layout and visit code that reads the
    /// instances through [`Self::len`] and [`Self::instance_at`]: it is
    /// notified when rows are inserted or removed, and again when
    /// `ensure_updated` creates their instances.
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
            // A number model (`for i in n`) is new each time `n` is evaluated, but the
            // existing rows stay the same, so keep their instances.
            let number = |m: &ModelRc<C::Data>| m.as_any().downcast_ref::<usize>().copied();
            if let (Some(old_count), Some(new_count)) = (number(&old_model), number(&m)) {
                if new_count > old_count {
                    self.data().row_added(old_count, new_count - old_count);
                } else if new_count < old_count {
                    self.data().row_removed(new_count, old_count - new_count);
                }
            } else if old_model != m {
                *self.data().inner.borrow_mut() = RepeaterInner::default();
                self.data().instances_changed();
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
                let instance_count = inner.instances.len();
                inner.layout_state.item_index.start_update_all(instance_count)
            };
            let mut ops = RustRepeaterOps {
                inner: &self.0.inner,
                typed: &RepeaterTypedOps::<C> { init: &init, model: &model },
                instance_generation: self.data().project_ref().instance_generation,
            };
            self.data().is_dirty.set(false);
            update_all_instances(&mut ops, offset, count);
            self.data().instance_generation.mark_dirty();
            true
        } else {
            false
        };
        ensure_children_instantiated(&self.0.inner) || changed
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
        let mut layout_state = data.inner.borrow().layout_state.clone();
        let mut scroll_to_row = data.inner.borrow_mut().scroll_to_row.take();
        let mut ops = RustRepeaterOps {
            inner: &data.inner,
            typed: &RepeaterTypedOps::<C> { init: &init, model: &model },
            instance_generation: data.project_ref().instance_generation,
        };
        let changed = update_visible_instances(
            &mut ops,
            &mut layout_state,
            &mut scroll_to_row,
            row_count,
            props,
            listview_width,
            listview_height,
        );
        {
            let mut inner = data.inner.borrow_mut();
            inner.layout_state = layout_state;
            // A request made while the layout ran, from a row's init code, wins.
            inner.scroll_to_row = inner.scroll_to_row.take().or(scroll_to_row);
        }

        if changed {
            self.data().instance_generation.mark_dirty();
        }
        ensure_children_instantiated(&self.0.inner) || changed
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
        visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        self.track_model_changes();
        visit_instances(&self.0.inner, order, visitor)
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
            let z = c.as_ref().and_then(|c| as_typed::<C>(c).z_order()).unwrap_or_default();
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
            .get(inner.layout_state.item_index.get_instance_index_opt(index)?)?
            .1
            .as_ref()
            .map(downcast)
    }

    /// Return true if the Repeater as empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all instances
    pub fn instances_vec(&self) -> Vec<ItemTreeRc<C>> {
        self.0.inner.borrow().instances.iter().filter_map(|x| x.1.as_ref()).map(downcast).collect()
    }
}

/// Implements [`Repeater::listview_ensure_row_visible`] with the instances of `inner`.
/// Returns whether the ListView needs a layout pass.
fn listview_ensure_instance_row_visible(
    inner: &RefCell<RepeaterInner>,
    row_count: usize,
    row: i32,
    behavior: ScrollBehavior,
    flickable_rc: &ItemRc,
) -> bool {
    let (state, heights, mut scroll_to_row) = {
        let mut inner = inner.borrow_mut();
        let state = inner.layout_state.clone();
        let heights = inner
            .instances
            .iter()
            .map(|c| c.1.as_ref().map_or(state.cached_item_height, instance_height))
            .collect::<Vec<_>>();
        (state, heights, inner.scroll_to_row.take())
    };
    let needs_layout = listview_ensure_row_visible(
        &state,
        &heights,
        &mut scroll_to_row,
        row_count,
        row,
        behavior,
        flickable_rc,
    );
    inner.borrow_mut().scroll_to_row = scroll_to_row;
    needs_layout
}

/// Ensures that the children of the instances are instantiated.
/// Returns whether any of them changed.
fn ensure_children_instantiated(inner: &RefCell<RepeaterInner>) -> bool {
    // We can't keep inner borrowed because instantiating a child might modify the model
    let instances: Vec<_> = inner.borrow().instances.iter().flat_map(|x| x.1.clone()).collect();
    let mut changed = false;
    for instance in instances {
        changed |= crate::item_tree::ensure_item_tree_instantiated(&instance);
    }
    changed
}

/// Calls the visitor for the root of `instance`. Returns whether the visit was aborted.
fn visit_instance(
    instance: &ItemTreeRc,
    order: TraversalOrder,
    visitor: crate::item_tree::ItemVisitorRefMut,
) -> bool {
    vtable::VRc::borrow_pin(instance).as_ref().visit_children_item(-1, order, visitor).has_aborted()
}

/// Implements [`Repeater::visit`].
fn visit_instances(
    inner: &RefCell<RepeaterInner>,
    order: TraversalOrder,
    mut visitor: crate::item_tree::ItemVisitorRefMut,
) -> crate::item_tree::VisitChildrenResult {
    // We can't keep inner borrowed because the event might modify the model
    let count = inner.borrow().instances.len() as u32;
    for i in 0..count {
        let i = if order == TraversalOrder::BackToFront { i } else { count - i - 1 };
        let c = inner.borrow().instances.get(i as usize).and_then(|c| c.1.clone());
        if let Some(c) = c
            && visit_instance(&c, order, visitor.borrow_mut())
        {
            return crate::item_tree::VisitChildrenResult::abort(i, 0);
        }
    }
    crate::item_tree::VisitChildrenResult::CONTINUE
}

#[pin_project]
pub struct Conditional<C: RepeatedItemTree> {
    #[pin]
    inner: ConditionalInner,
    phantom: core::marker::PhantomData<ItemTreeRc<C>>,
}

impl<C: RepeatedItemTree> Default for Conditional<C> {
    fn default() -> Self {
        Self { inner: ConditionalInner::default(), phantom: core::marker::PhantomData }
    }
}

#[pin_project]
struct ConditionalInner {
    #[pin]
    model: Property<bool>,
    #[pin]
    instance_generation: Property<()>,
    instance: RefCell<Option<ItemTreeRc>>,
}

impl Default for ConditionalInner {
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

impl ConditionalInner {
    /// Implements [`Conditional::ensure_updated`].
    fn ensure_updated(
        self: Pin<&Self>,
        create: &dyn Fn() -> ItemTreeRc,
        init: &dyn Fn(&ItemTreeRc),
    ) -> bool {
        let model = self.project_ref().model.get();

        let changed = if !model {
            self.instance.take().is_some()
        } else if self.instance.borrow().is_none() {
            let i = create();
            self.instance.replace(Some(i.clone()));
            // #13723
            self.instance_generation.mark_dirty();
            init(&i);
            true
        } else {
            false
        };
        if changed {
            self.instance_generation.mark_dirty();
        }
        if let Some(instance) = self.instance.borrow().as_ref() {
            crate::item_tree::ensure_item_tree_instantiated(instance) || changed
        } else {
            changed
        }
    }

    /// Implements [`Conditional::visit`].
    fn visit(
        self: Pin<&Self>,
        order: TraversalOrder,
        mut visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        // We can't keep self.instance borrowed because the event might modify the model
        let instance = self.instance.borrow().clone();
        if let Some(c) = instance
            && visit_instance(&c, order, visitor.borrow_mut())
        {
            return crate::item_tree::VisitChildrenResult::abort(0, 0);
        }

        crate::item_tree::VisitChildrenResult::CONTINUE
    }
}

impl<C: RepeatedItemTree + 'static> Conditional<C> {
    fn inner(self: Pin<&Self>) -> Pin<&ConditionalInner> {
        self.project_ref().inner
    }

    /// Register the condition as a dependency of the current tracking scope
    /// (e.g. the redraw tracker) so it is notified when the condition changes.
    pub fn track_model_changes(self: Pin<&Self>) {
        self.inner().project_ref().model.register_as_dependency();
    }

    /// Register the instance generation as a dependency of the current
    /// tracking scope. Layout code uses this to re-evaluate when
    /// `ensure_updated` creates or removes the instance.
    pub fn track_instance_changes(self: Pin<&Self>) {
        self.inner().project_ref().instance_generation.register_as_dependency();
    }

    /// Call this function to make sure that the model is updated.
    /// The init function is the function to create a ItemTree.
    /// Returns `true` if the instance was created or removed, or any child changed.
    pub fn ensure_updated(self: Pin<&Self>, init: impl Fn() -> ItemTreeRc<C>) -> bool {
        self.inner().ensure_updated(&|| vtable::VRc::into_dyn(init()), &|instance| {
            as_typed::<C>(instance).init()
        })
    }

    /// Set the model binding
    pub fn set_model_binding(&self, binding: impl Fn() -> bool + 'static) {
        self.inner.model.set_binding(binding);
    }

    /// Call the visitor for the root of each instance.
    /// Also registers model dependencies so the current tracking scope
    /// (e.g. the redraw tracker) is notified when the condition changes.
    pub fn visit(
        self: Pin<&Self>,
        order: TraversalOrder,
        visitor: crate::item_tree::ItemVisitorRefMut,
    ) -> crate::item_tree::VisitChildrenResult {
        self.track_model_changes();
        self.inner().visit(order, visitor)
    }

    /// Call `cb` with the index and the z value of the instance if the condition is
    /// active, when the conditional element has a dynamic z binding.
    /// Also registers the condition as a dependency of the current tracking scope.
    pub fn for_each_instance_z(self: Pin<&Self>, cb: &mut dyn FnMut(u32, f32)) {
        self.track_model_changes();
        let instance = self.inner.instance.borrow().clone();
        if let Some(c) = instance {
            cb(0, as_typed::<C>(&c).z_order().unwrap_or_default());
        }
    }

    /// Return the amount of instances (1 if the conditional is active, 0 otherwise)
    pub fn len(&self) -> usize {
        self.inner.instance.borrow().is_some() as usize
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
        self.inner.instance.borrow().as_ref().map(downcast)
    }

    /// Return true if the Repeater as empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a vector containing all instances
    pub fn instances_vec(&self) -> Vec<ItemTreeRc<C>> {
        self.inner.instance.borrow().as_ref().map(downcast).into_iter().collect()
    }
}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use crate::slice::Slice;
    use alloc::boxed::Box;

    /// The C++ repeater's storage of its pending `ensure-row-visible` request.
    #[repr(C)]
    pub struct ScrollToRowRequestOpaque {
        _inner: *const core::ffi::c_void,
    }

    type ScrollToRowRequestStorage = Option<Box<scroll::ScrollToRowRequest>>;

    static_assertions::assert_eq_align!(ScrollToRowRequestOpaque, ScrollToRowRequestStorage);
    static_assertions::assert_eq_size!(ScrollToRowRequestOpaque, ScrollToRowRequestStorage);

    /// Initialize the request storage.
    /// slint_repeater_scroll_to_row_drop must be called.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_repeater_scroll_to_row_construct(
        request: *mut ScrollToRowRequestOpaque,
    ) {
        unsafe { core::ptr::write(request.cast::<ScrollToRowRequestStorage>(), None) };
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_repeater_scroll_to_row_drop(
        request: *mut ScrollToRowRequestOpaque,
    ) {
        unsafe { core::ptr::drop_in_place(request.cast::<ScrollToRowRequestStorage>()) };
    }

    /// C++ callback table for [`RepeaterInstanceOps`], including the opaque
    /// user_data pointer that is passed to each callback.
    #[repr(C)]
    pub struct RepeaterInstanceOpsVTable {
        pub user_data: *mut core::ffi::c_void,
        /// The `item_index` of the repeater's layout state, updated along with the instances.
        pub item_index: *mut ItemIndexRelationShip,
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
            unsafe {
                (*self.item_index).instances_cleared();
                (self.clear)(self.user_data)
            }
        }
        fn push(&mut self, count: usize) {
            unsafe { (self.push)(self.user_data, count) }
        }
        fn prepend(&mut self, count: usize) {
            let instance_count = self.len();
            unsafe {
                (*self.item_index).instances_prepended(instance_count, count);
                (self.prepend)(self.user_data, count)
            }
        }
        fn remove_first(&mut self, count: usize) {
            let instance_count = self.len();
            unsafe {
                (*self.item_index).first_instances_removed(instance_count, count);
                (self.remove_first)(self.user_data, count)
            }
        }
        fn truncate(&mut self, len: usize) {
            let instance_count = self.len();
            unsafe {
                (*self.item_index).instances_truncated(instance_count, len);
                (self.truncate)(self.user_data, len)
            }
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

    /// See [`Repeater::ensure_updated`].
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_repeater_ensure_updated(
        ops: &mut RepeaterInstanceOpsVTable,
        count: usize,
    ) {
        let instance_count = ops.len();
        let offset = unsafe { (*ops.item_index).start_update_all(instance_count) };
        update_all_instances(ops, offset, count);
    }

    /// See [`ModelChangeListener::row_added`].
    /// Returns whether the instances change, and if so, sets `slots`.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_repeater_rows_added(
        item_index: &mut ItemIndexRelationShip,
        instance_count: usize,
        index: usize,
        count: usize,
        slots: &mut InstanceSlots,
    ) -> bool {
        item_index.rows_added(instance_count, index, count).map(|s| *slots = s).is_some()
    }

    /// See [`ModelChangeListener::row_removed`].
    /// `row_count` is the number of rows after the removal.
    /// Returns whether the instances change, and if so, sets `removed` to the instances to remove.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_repeater_rows_removed(
        item_index: &mut ItemIndexRelationShip,
        instance_count: usize,
        index: usize,
        count: usize,
        row_count: usize,
        removed: &mut crate::item_tree::IndexRange,
    ) -> bool {
        item_index
            .rows_removed(instance_count, index, count, row_count)
            .map(|range| *removed = range.into())
            .is_some()
    }

    /// `state` and `scroll_to_row` are pointers, not references, because a row's init code,
    /// which `ops` runs, can call `slint_repeater_listview_ensure_row_visible` on them.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_repeater_ensure_updated_listview(
        ops: &mut RepeaterInstanceOpsVTable,
        state: *mut RepeaterLayoutState,
        scroll_to_row: *mut ScrollToRowRequestOpaque,
        row_count: usize,
        content_width: Option<Pin<&Property<LogicalLength>>>,
        content_height: Option<Pin<&Property<LogicalLength>>>,
        content_y: Pin<&Property<LogicalLength>>,
        listview_width: LogicalLength,
        listview_height: LogicalLength,
    ) -> bool {
        let props = TypedListViewProps { content_width, content_height, content_y };
        let mut layout_state = unsafe { (*state).clone() };
        // When a new item gets created in update_visible_instances and in its init
        // we call ensure-row-visible() we modify scroll_to_row
        // here we make a copy and reset `scroll_to_row`. If below `scroll_to_row`
        // is still not valid we set our new request otherwise we keep the `scroll_to_row`
        // request set by ensure-row-visible
        // Tested in `listview_ensure_row_visible_from_row_init.slint`
        // Safety: `slint_repeater_scroll_to_row_construct` initialized it, with the same layout.
        let storage = scroll_to_row.cast::<ScrollToRowRequestStorage>();
        let mut request = unsafe { (*storage).take() }.map(|request| *request);
        let changed = update_visible_instances(
            ops,
            &mut layout_state,
            &mut request,
            row_count,
            &props,
            listview_width,
            listview_height,
        );
        unsafe {
            *state = layout_state;
            // See `Repeater::ensure_updated_listview_callback`.
            if (*storage).is_none() {
                *storage = request.map(Box::new);
            }
        }
        changed
    }

    /// See [`Repeater::listview_ensure_row_visible`].
    /// `heights` are the heights of the instances, NaN for one that isn't created yet.
    /// Returns whether the ListView needs a layout pass.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_repeater_listview_ensure_row_visible(
        state: &RepeaterLayoutState,
        scroll_to_row: &mut ScrollToRowRequestOpaque,
        heights: Slice<Coord>,
        row_count: usize,
        row: i32,
        behavior: ScrollBehavior,
        flickable_rc: &ItemRc,
    ) -> bool {
        let heights = heights
            .iter()
            .map(|height| if height.is_nan() { state.cached_item_height } else { *height })
            .collect::<Vec<_>>();

        // Safety: `slint_repeater_scroll_to_row_construct` initialized it, with the same layout.
        let storage = unsafe {
            &mut *(scroll_to_row as *mut ScrollToRowRequestOpaque)
                .cast::<ScrollToRowRequestStorage>()
        };
        let mut request = storage.take().map(|request| *request);
        let needs_layout = listview_ensure_row_visible(
            state,
            &heights,
            &mut request,
            row_count,
            row,
            behavior,
            flickable_rc,
        );
        *storage = request.map(Box::new);
        needs_layout
    }
}

#[cfg(test)]
mod tests;
