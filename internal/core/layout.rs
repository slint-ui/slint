// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runtime support for layouts.

// cspell:ignore coord

use crate::items::{
    DialogButtonRole, FlexAlignContent, FlexAlignItems, FlexDirection, LayoutAlignment,
};
use crate::{Coord, SharedVector, slice::Slice};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use num_traits::Float;

pub use crate::items::Orientation;

/// The constraint that applies to an item
// Also, the field needs to be in alphabetical order because how the generated code sort fields for struct
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutInfo {
    /// The maximum size for the item.
    pub max: Coord,
    /// The maximum size in percentage of the parent (value between 0 and 100).
    pub max_percent: Coord,
    /// The minimum size for this item.
    pub min: Coord,
    /// The minimum size in percentage of the parent (value between 0 and 100).
    pub min_percent: Coord,
    /// the preferred size
    pub preferred: Coord,
    /// the  stretch factor
    pub stretch: f32,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        LayoutInfo {
            min: 0 as _,
            max: Coord::MAX,
            min_percent: 0 as _,
            max_percent: 100 as _,
            preferred: 0 as _,
            stretch: 0 as _,
        }
    }
}

impl LayoutInfo {
    // Note: This "logic" is duplicated in the cpp generator's generated code for merging layout infos.
    #[must_use]
    pub fn merge(&self, other: &LayoutInfo) -> Self {
        Self {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
            min_percent: self.min_percent.max(other.min_percent),
            max_percent: self.max_percent.min(other.max_percent),
            preferred: self.preferred.max(other.preferred),
            stretch: self.stretch.min(other.stretch),
        }
    }

    /// Helper function to return a preferred size which is within the min/max constraints
    #[must_use]
    pub fn preferred_bounded(&self) -> Coord {
        self.preferred.min(self.max).max(self.min)
    }
}

impl core::ops::Add for LayoutInfo {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.merge(&rhs)
    }
}

/// Returns the logical min and max sizes given the provided layout constraints.
pub fn min_max_size_for_layout_constraints(
    constraints_horizontal: LayoutInfo,
    constraints_vertical: LayoutInfo,
) -> (Option<crate::api::LogicalSize>, Option<crate::api::LogicalSize>) {
    let min_width = constraints_horizontal.min.min(constraints_horizontal.max) as f32;
    let min_height = constraints_vertical.min.min(constraints_vertical.max) as f32;
    let max_width = constraints_horizontal.max.max(constraints_horizontal.min) as f32;
    let max_height = constraints_vertical.max.max(constraints_vertical.min) as f32;

    //cfg!(target_arch = "wasm32") is there because wasm32 winit don't like when max size is None:
    // panicked at 'Property is read only: JsValue(NoModificationAllowedError: CSSStyleDeclaration.removeProperty: Can't remove property 'max-width' from computed style

    let min_size = if min_width > 0. || min_height > 0. || cfg!(target_arch = "wasm32") {
        Some(crate::api::LogicalSize::new(min_width, min_height))
    } else {
        None
    };

    let max_size = if (max_width > 0.
        && max_height > 0.
        && (max_width < i32::MAX as f32 || max_height < i32::MAX as f32))
        || cfg!(target_arch = "wasm32")
    {
        // maximum widget size for Qt and a workaround for the winit api not allowing partial constraints
        let window_size_max = 16_777_215.;
        Some(crate::api::LogicalSize::new(
            max_width.min(window_size_max),
            max_height.min(window_size_max),
        ))
    } else {
        None
    };

    (min_size, max_size)
}

/// Implement a saturating_add version for both possible value of Coord.
/// So that adding the max value does not overflow
trait Saturating {
    fn add(_: Self, _: Self) -> Self;
}
impl Saturating for i32 {
    #[inline]
    fn add(a: Self, b: Self) -> Self {
        a.saturating_add(b)
    }
}
impl Saturating for f32 {
    #[inline]
    fn add(a: Self, b: Self) -> Self {
        a + b
    }
}

mod grid_internal {
    use super::*;

    fn order_coord<T: PartialOrd>(a: &T, b: &T) -> core::cmp::Ordering {
        a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal)
    }

    #[derive(Debug, Clone)]
    pub struct LayoutData {
        // inputs
        pub min: Coord,
        pub max: Coord,
        pub pref: Coord,
        pub stretch: f32,

        // outputs
        pub pos: Coord,
        pub size: Coord,
    }

    impl Default for LayoutData {
        fn default() -> Self {
            LayoutData {
                min: 0 as _,
                max: Coord::MAX,
                pref: 0 as _,
                stretch: f32::MAX,
                pos: 0 as _,
                size: 0 as _,
            }
        }
    }

    trait Adjust {
        fn can_grow(_: &LayoutData) -> Coord;
        fn to_distribute(expected_size: Coord, current_size: Coord) -> Coord;
        fn distribute(_: &mut LayoutData, val: Coord);
    }

    struct Grow;
    impl Adjust for Grow {
        fn can_grow(it: &LayoutData) -> Coord {
            it.max - it.size
        }

        fn to_distribute(expected_size: Coord, current_size: Coord) -> Coord {
            expected_size - current_size
        }

        fn distribute(it: &mut LayoutData, val: Coord) {
            it.size += val;
        }
    }

    struct Shrink;
    impl Adjust for Shrink {
        fn can_grow(it: &LayoutData) -> Coord {
            it.size - it.min
        }

        fn to_distribute(expected_size: Coord, current_size: Coord) -> Coord {
            current_size - expected_size
        }

        fn distribute(it: &mut LayoutData, val: Coord) {
            it.size -= val;
        }
    }

    #[allow(clippy::unnecessary_cast)] // Coord
    fn adjust_items<A: Adjust>(data: &mut [LayoutData], size_without_spacing: Coord) -> Option<()> {
        loop {
            let size_cannot_grow: Coord = data
                .iter()
                .filter(|it| A::can_grow(it) <= 0 as _)
                .map(|it| it.size)
                .fold(0 as Coord, Saturating::add);

            let total_stretch: f32 =
                data.iter().filter(|it| A::can_grow(it) > 0 as _).map(|it| it.stretch).sum();

            let actual_stretch = |s: f32| if total_stretch <= 0. { 1. } else { s };

            let max_grow = data
                .iter()
                .filter(|it| A::can_grow(it) > 0 as _)
                .map(|it| A::can_grow(it) as f32 / actual_stretch(it.stretch))
                .min_by(order_coord)?;

            let current_size: Coord = data
                .iter()
                .filter(|it| A::can_grow(it) > 0 as _)
                .map(|it| it.size)
                .fold(0 as _, Saturating::add);

            //let to_distribute = size_without_spacing - (size_cannot_grow + current_size);
            let to_distribute =
                A::to_distribute(size_without_spacing, size_cannot_grow + current_size) as f32;
            if to_distribute <= 0. || max_grow <= 0. {
                return Some(());
            }

            let grow = if total_stretch <= 0. {
                to_distribute
                    / (data.iter().filter(|it| A::can_grow(it) > 0 as _).count() as Coord) as f32
            } else {
                to_distribute / total_stretch
            }
            .min(max_grow);

            let mut distributed = 0 as Coord;
            for it in data.iter_mut().filter(|it| A::can_grow(it) > 0 as Coord) {
                let val = (grow * actual_stretch(it.stretch)) as Coord;
                A::distribute(it, val);
                distributed += val;
            }

            if distributed <= 0 as Coord {
                // This can happen when Coord is integer and there is less then a pixel to add to each elements
                // just give the pixel to the one with the bigger stretch
                if let Some(it) = data
                    .iter_mut()
                    .filter(|it| A::can_grow(it) > 0 as _)
                    .max_by(|a, b| actual_stretch(a.stretch).total_cmp(&b.stretch))
                {
                    A::distribute(it, to_distribute as Coord);
                }
                return Some(());
            }
        }
    }

    pub fn layout_items(data: &mut [LayoutData], start_pos: Coord, size: Coord, spacing: Coord) {
        let size_without_spacing = size - spacing * (data.len() - 1) as Coord;

        let mut pref = 0 as Coord;
        for it in data.iter_mut() {
            it.size = it.pref;
            pref += it.pref;
        }
        if size_without_spacing >= pref {
            adjust_items::<Grow>(data, size_without_spacing);
        } else if size_without_spacing < pref {
            adjust_items::<Shrink>(data, size_without_spacing);
        }

        let mut pos = start_pos;
        for it in data.iter_mut() {
            it.pos = pos;
            pos = Saturating::add(pos, Saturating::add(it.size, spacing));
        }
    }

    #[test]
    #[allow(clippy::float_cmp)] // We want bit-wise equality here
    fn test_layout_items() {
        let my_items = &mut [
            LayoutData { min: 100., max: 200., pref: 100., stretch: 1., ..Default::default() },
            LayoutData { min: 50., max: 300., pref: 100., stretch: 1., ..Default::default() },
            LayoutData { min: 50., max: 150., pref: 100., stretch: 1., ..Default::default() },
        ];

        layout_items(my_items, 100., 650., 0.);
        assert_eq!(my_items[0].size, 200.);
        assert_eq!(my_items[1].size, 300.);
        assert_eq!(my_items[2].size, 150.);

        layout_items(my_items, 100., 200., 0.);
        assert_eq!(my_items[0].size, 100.);
        assert_eq!(my_items[1].size, 50.);
        assert_eq!(my_items[2].size, 50.);

        layout_items(my_items, 100., 300., 0.);
        assert_eq!(my_items[0].size, 100.);
        assert_eq!(my_items[1].size, 100.);
        assert_eq!(my_items[2].size, 100.);
    }

    /// Create a vector of LayoutData (e.g. one per row if Vertical) based on the constraints and organized data
    /// Used by both solve_grid_layout() and grid_layout_info()
    pub fn to_layout_data(
        organized_data: &GridLayoutOrganizedData,
        constraints: Slice<LayoutItemInfo>,
        orientation: Orientation,
        repeater_indices: Slice<u32>,
        repeater_steps: Slice<u32>,
        spacing: Coord,
        size: Option<Coord>,
    ) -> Vec<LayoutData> {
        assert!(organized_data.len().is_multiple_of(4));
        let num = organized_data.max_value(
            constraints.len(),
            orientation,
            &repeater_indices,
            &repeater_steps,
        ) as usize;
        if num < 1 {
            return Default::default();
        }
        let marker_for_empty = -1.;
        let mut layout_data = alloc::vec![grid_internal::LayoutData { max: 0 as Coord, stretch: marker_for_empty, ..Default::default() }; num];
        let mut has_spans = false;
        for (idx, cell_data) in constraints.iter().enumerate() {
            let constraint = &cell_data.constraint;
            let mut max = constraint.max;
            if let Some(size) = size {
                max = max.min(size * constraint.max_percent / 100 as Coord);
            }
            let (col_or_row, span) = organized_data.col_or_row_and_span(
                idx,
                orientation,
                &repeater_indices,
                &repeater_steps,
            );
            for c in 0..(span as usize) {
                let cdata = &mut layout_data[col_or_row as usize + c];
                // Initialize max/stretch to proper defaults on first item in this row/col
                // so that empty rows/columns don't stretch.
                if cdata.stretch == marker_for_empty {
                    cdata.max = Coord::MAX;
                    cdata.stretch = 1.;
                }
                cdata.max = cdata.max.min(max);
            }
            if span == 1 {
                let mut min = constraint.min;
                if let Some(size) = size {
                    min = min.max(size * constraint.min_percent / 100 as Coord);
                }
                let pref = constraint.preferred.min(max).max(min);
                let cdata = &mut layout_data[col_or_row as usize];
                cdata.min = cdata.min.max(min);
                cdata.pref = cdata.pref.max(pref);
                cdata.stretch = cdata.stretch.min(constraint.stretch);
            } else {
                has_spans = true;
            }
        }
        if has_spans {
            for (idx, cell_data) in constraints.iter().enumerate() {
                let constraint = &cell_data.constraint;
                let (col_or_row, span) = organized_data.col_or_row_and_span(
                    idx,
                    orientation,
                    &repeater_indices,
                    &repeater_steps,
                );
                if span > 1 {
                    let span_data =
                        &mut layout_data[(col_or_row as usize)..(col_or_row + span) as usize];

                    // Adjust minimum sizes
                    let mut min = constraint.min;
                    if let Some(size) = size {
                        min = min.max(size * constraint.min_percent / 100 as Coord);
                    }
                    grid_internal::layout_items(span_data, 0 as _, min, spacing);
                    for cdata in span_data.iter_mut() {
                        if cdata.min < cdata.size {
                            cdata.min = cdata.size;
                        }
                    }

                    // Adjust maximum sizes
                    let mut max = constraint.max;
                    if let Some(size) = size {
                        max = max.min(size * constraint.max_percent / 100 as Coord);
                    }
                    grid_internal::layout_items(span_data, 0 as _, max, spacing);
                    for cdata in span_data.iter_mut() {
                        if cdata.max > cdata.size {
                            cdata.max = cdata.size;
                        }
                    }

                    // Adjust preferred sizes
                    grid_internal::layout_items(span_data, 0 as _, constraint.preferred, spacing);
                    for cdata in span_data.iter_mut() {
                        cdata.pref = cdata.pref.max(cdata.size).min(cdata.max).max(cdata.min);
                    }

                    // Adjust stretches
                    let total_stretch: f32 = span_data.iter().map(|c| c.stretch).sum();
                    if total_stretch > constraint.stretch {
                        for cdata in span_data.iter_mut() {
                            cdata.stretch *= constraint.stretch / total_stretch;
                        }
                    }
                }
            }
        }
        for cdata in layout_data.iter_mut() {
            if cdata.stretch == marker_for_empty {
                cdata.stretch = 0.;
            }
        }
        layout_data
    }
}

#[repr(C)]
pub struct Constraint {
    pub min: Coord,
    pub max: Coord,
}

impl Default for Constraint {
    fn default() -> Self {
        Constraint { min: 0 as Coord, max: Coord::MAX }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Padding {
    pub begin: Coord,
    pub end: Coord,
}

#[repr(C)]
#[derive(Debug)]
/// The horizontal or vertical data for all cells of a GridLayout, used as input to solve_grid_layout()
pub struct GridLayoutData {
    pub size: Coord,
    pub spacing: Coord,
    pub padding: Padding,
    pub organized_data: GridLayoutOrganizedData,
}

/// The input data for a cell of a GridLayout, before row/col determination and before H/V split
/// Used as input to organize_grid_layout()
#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct GridLayoutInputData {
    /// whether this cell is the first one in a Row element
    pub new_row: bool,
    /// col and row number.
    /// Only ROW_COL_AUTO and the u16 range are valid, values outside of
    /// that will be clamped with a warning at runtime
    pub col: f32,
    pub row: f32,
    /// colspan and rowspan
    /// Only the u16 range is valid, values outside of that will be clamped with a warning at runtime
    pub colspan: f32,
    pub rowspan: f32,
}

/// The organized layout data for a GridLayout, after row/col determination:
/// For each cell, stores col, colspan, row, rowspan
pub type GridLayoutOrganizedData = SharedVector<u16>;

impl GridLayoutOrganizedData {
    fn push_cell(&mut self, col: u16, colspan: u16, row: u16, rowspan: u16) {
        self.push(col);
        self.push(colspan);
        self.push(row);
        self.push(rowspan);
    }

    fn col_or_row_and_span(
        &self,
        cell_number: usize,
        orientation: Orientation,
        repeater_indices: &Slice<u32>,
        repeater_steps: &Slice<u32>,
    ) -> (u16, u16) {
        // For every cell, we have 4 entries, each at their own index
        // But we also need to take into account indirections for repeated items

        // Two-level indirection for repeated items:
        //   jump_pos = (ri_start_cell - cell_nr_adj) * 4
        //   data_base = self[jump_pos]        (base of this repeater's data)
        //   stride    = step * 4              (computed from repeater_steps)
        //   data_idx = data_base + row_in_rep * stride + col_in_rep * 4
        let mut final_idx = 0;
        let mut cell_nr_adj = 0i32; // needs to be signed in case we start with an empty repeater
        let cell_number = cell_number as i32;
        // repeater_indices is a list of (start_cell, count) pairs
        for rep_idx in 0..(repeater_indices.len() / 2) {
            let ri_start_cell = repeater_indices[rep_idx * 2] as i32;
            if cell_number < ri_start_cell {
                break;
            }
            let ri_cell_count = repeater_indices[rep_idx * 2 + 1] as i32;
            let step = repeater_steps.get(rep_idx).copied().unwrap_or(1) as i32;
            let cells_in_repeater = ri_cell_count * step;
            if cells_in_repeater > 0
                && cell_number >= ri_start_cell
                && cell_number < ri_start_cell + cells_in_repeater
            {
                let cell_in_rep = cell_number - ri_start_cell;
                let row_in_rep = cell_in_rep / step;
                let col_in_rep = cell_in_rep % step;
                let jump_pos = (ri_start_cell - cell_nr_adj) as usize * 4;
                let data_base = self[jump_pos] as usize;
                let stride = step as usize * 4;
                final_idx = data_base + row_in_rep as usize * stride + col_in_rep as usize * 4;
                break;
            }
            // Each repeater occupies 1 jump cell in the static area but cells_in_repeater cells logically
            // Note: -1 is correct for an empty repeater (e.g. if false), which occupies 1 jump cell, for 0 real cells
            cell_nr_adj += cells_in_repeater - 1;
        }
        if final_idx == 0 {
            final_idx = ((cell_number - cell_nr_adj) * 4) as usize;
        }
        let offset = if orientation == Orientation::Horizontal { 0 } else { 2 };
        (self[final_idx + offset], self[final_idx + offset + 1])
    }

    fn max_value(
        &self,
        num_cells: usize,
        orientation: Orientation,
        repeater_indices: &Slice<u32>,
        repeater_steps: &Slice<u32>,
    ) -> u16 {
        let mut max = 0;
        // This could be rewritten more efficiently to avoid a loop calling a loop, by keeping track of the repeaters we saw until now
        // Not sure it's worth the complexity though
        for idx in 0..num_cells {
            let (col_or_row, span) =
                self.col_or_row_and_span(idx, orientation, repeater_indices, repeater_steps);
            max = max.max(col_or_row + span.max(1));
        }
        max
    }
}

/// Two-level indirection organized data generator for grid layouts with repeaters.
/// Uses 2-level indirection: cache[cache[jump_pos] + ri * stride + col * 4]
/// Each jump cell stores [data_base, 0, 0, 0] where stride is computed as step * 4.
///
/// Layout: [static_cells (4 u16 each)] [jump_cells (4 u16 each, 1 per repeater)]
///         [row_data (rep_count * step * 4 u16)] ... (repeated for each repeater)
struct OrganizedDataGenerator<'a> {
    // Input
    repeater_indices: &'a [u32],
    repeater_steps: &'a [u32],
    // An always increasing counter, the index of the cell being added
    counter: usize,
    // The u16 position in result for the next repeater's data section
    repeat_u16_offset: usize,
    // The index/2 in repeater_indices (i.e. which repeater we're looking at next)
    next_rep: usize,
    // The cell index in result for the next non-repeated item (each cell = 4 u16)
    current_offset: usize,
    // Output
    result: &'a mut GridLayoutOrganizedData,
}

impl<'a> OrganizedDataGenerator<'a> {
    fn new(
        repeater_indices: &'a [u32],
        repeater_steps: &'a [u32],
        static_cells: usize,
        num_repeaters: usize,
        total_repeated_cells_count: usize,
        result: &'a mut GridLayoutOrganizedData,
    ) -> Self {
        result.resize((static_cells + num_repeaters + total_repeated_cells_count) * 4, 0 as _);
        let repeat_u16_offset = (static_cells + num_repeaters) * 4;
        Self {
            repeater_indices,
            repeater_steps,
            counter: 0,
            repeat_u16_offset,
            next_rep: 0,
            current_offset: 0,
            result,
        }
    }
    fn add(&mut self, col: u16, colspan: u16, row: u16, rowspan: u16) {
        let res = self.result.make_mut_slice();
        loop {
            if let Some(nr) = self.repeater_indices.get(self.next_rep * 2) {
                let nr = *nr as usize;
                let step = self.repeater_steps.get(self.next_rep).copied().unwrap_or(1) as usize;
                let rep_count = self.repeater_indices[self.next_rep * 2 + 1] as usize;

                if nr == self.counter {
                    // First cell of this repeater
                    let data_u16_start = self.repeat_u16_offset;

                    // Write jump cell: [data_base, 0, 0, 0]
                    res[self.current_offset * 4] = data_u16_start as _;
                    self.current_offset += 1;
                }
                if self.counter >= nr {
                    let cells_in_repeater = rep_count * step;
                    if self.counter - nr == cells_in_repeater {
                        // Past the end of this repeater — advance past data
                        self.repeat_u16_offset += cells_in_repeater * 4;
                        self.next_rep += 1;
                        continue;
                    }
                    // Write data at the position determined by row/col within repeater
                    let cell_in_rep = self.counter - nr;
                    let row_in_rep = cell_in_rep / step;
                    let col_in_rep = cell_in_rep % step;
                    let data_u16_start = self.repeat_u16_offset;
                    let u16_pos = data_u16_start + row_in_rep * step * 4 + col_in_rep * 4;
                    res[u16_pos] = col;
                    res[u16_pos + 1] = colspan;
                    res[u16_pos + 2] = row;
                    res[u16_pos + 3] = rowspan;
                    self.counter += 1;
                    return;
                }
            }
            // Non-repeated cell
            res[self.current_offset * 4] = col;
            res[self.current_offset * 4 + 1] = colspan;
            res[self.current_offset * 4 + 2] = row;
            res[self.current_offset * 4 + 3] = rowspan;
            self.current_offset += 1;
            self.counter += 1;
            return;
        }
    }
}

/// Given the cells of a layout of a Dialog, re-order the buttons according to the platform
/// This function assume that the `roles` contains the roles of the button which are the first cells in `input_data`
pub fn organize_dialog_button_layout(
    input_data: Slice<GridLayoutInputData>,
    dialog_button_roles: Slice<DialogButtonRole>,
) -> GridLayoutOrganizedData {
    let mut organized_data = GridLayoutOrganizedData::default();
    organized_data.reserve(input_data.len() * 4);

    #[cfg(feature = "std")]
    fn is_kde() -> bool {
        // assume some Unix, check if XDG_CURRENT_DESKTOP starts with K
        std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .and_then(|v| v.as_bytes().first().copied())
            .is_some_and(|x| x.eq_ignore_ascii_case(&b'K'))
    }
    #[cfg(not(feature = "std"))]
    let is_kde = || true;

    let expected_order: &[DialogButtonRole] = match crate::detect_operating_system() {
        crate::items::OperatingSystemType::Windows => {
            &[
                DialogButtonRole::Reset,
                DialogButtonRole::None, // spacer
                DialogButtonRole::Accept,
                DialogButtonRole::Action,
                DialogButtonRole::Reject,
                DialogButtonRole::Apply,
                DialogButtonRole::Help,
            ]
        }
        crate::items::OperatingSystemType::Macos | crate::items::OperatingSystemType::Ios => {
            &[
                DialogButtonRole::Help,
                DialogButtonRole::Reset,
                DialogButtonRole::Apply,
                DialogButtonRole::Action,
                DialogButtonRole::None, // spacer
                DialogButtonRole::Reject,
                DialogButtonRole::Accept,
            ]
        }
        _ if is_kde() => {
            // KDE variant
            &[
                DialogButtonRole::Help,
                DialogButtonRole::Reset,
                DialogButtonRole::None, // spacer
                DialogButtonRole::Action,
                DialogButtonRole::Accept,
                DialogButtonRole::Apply,
                DialogButtonRole::Reject,
            ]
        }
        _ => {
            // GNOME variant and fallback for WASM build
            &[
                DialogButtonRole::Help,
                DialogButtonRole::Reset,
                DialogButtonRole::None, // spacer
                DialogButtonRole::Action,
                DialogButtonRole::Accept,
                DialogButtonRole::Apply,
                DialogButtonRole::Reject,
            ]
        }
    };

    // Reorder the actual buttons according to expected_order
    let mut column_for_input: Vec<usize> = Vec::with_capacity(dialog_button_roles.len());
    for role in expected_order.iter() {
        if role == &DialogButtonRole::None {
            column_for_input.push(usize::MAX); // empty column, ensure nothing will match
            continue;
        }
        for (idx, r) in dialog_button_roles.as_slice().iter().enumerate() {
            if *r == *role {
                column_for_input.push(idx);
            }
        }
    }

    for (input_index, cell) in input_data.as_slice().iter().enumerate() {
        let col = column_for_input.iter().position(|&x| x == input_index);
        if let Some(col) = col {
            organized_data.push_cell(col as _, cell.colspan as _, cell.row as _, cell.rowspan as _);
        } else {
            // This is used for the main window (which is the only cell which isn't a button)
            // Given lower_dialog_layout(), this will always be a single cell at 0,0 with a colspan of number_of_buttons
            organized_data.push_cell(
                cell.col as _,
                cell.colspan as _,
                cell.row as _,
                cell.rowspan as _,
            );
        }
    }
    organized_data
}

// GridLayout-specific
fn total_repeated_cells<'a>(repeater_indices: &'a [u32], repeater_steps: &'a [u32]) -> usize {
    repeater_indices
        .chunks(2)
        .enumerate()
        .map(|(i, chunk)| {
            let count = chunk.get(1).copied().unwrap_or(0) as usize;
            let step = repeater_steps.get(i).copied().unwrap_or(1) as usize;
            count * step
        })
        .sum()
}

type Errors = Vec<String>;

pub fn organize_grid_layout(
    input_data: Slice<GridLayoutInputData>,
    repeater_indices: Slice<u32>,
    repeater_steps: Slice<u32>,
) -> GridLayoutOrganizedData {
    let (organized_data, errors) =
        organize_grid_layout_impl(input_data, repeater_indices, repeater_steps);
    for error in errors {
        crate::debug_log!("Slint layout error: {}", error);
    }
    organized_data
}

// Implement "auto" behavior for row/col numbers (unless specified in the slint file).
fn organize_grid_layout_impl(
    input_data: Slice<GridLayoutInputData>,
    repeater_indices: Slice<u32>,
    repeater_steps: Slice<u32>,
) -> (GridLayoutOrganizedData, Errors) {
    let mut organized_data = GridLayoutOrganizedData::default();
    // Cache size: static_cells * 4 + num_repeaters * 4 (jump cells)
    //              + per repeater: rep_count * step * 4 (data)
    let num_repeaters = repeater_indices.len() / 2;
    let total_repeated_cells =
        total_repeated_cells(repeater_indices.as_slice(), repeater_steps.as_slice());
    let static_cells = input_data.len() - total_repeated_cells;
    let mut generator = OrganizedDataGenerator::new(
        repeater_indices.as_slice(),
        repeater_steps.as_slice(),
        static_cells,
        num_repeaters,
        total_repeated_cells,
        &mut organized_data,
    );
    let mut errors = Vec::new();

    fn clamp_to_u16(value: f32, field_name: &str, errors: &mut Vec<String>) -> u16 {
        if value < 0.0 {
            errors.push(format!("cell {field_name} {value} is negative, clamping to 0"));
            0
        } else if value > u16::MAX as f32 {
            errors
                .push(format!("cell {field_name} {value} is too large, clamping to {}", u16::MAX));
            u16::MAX
        } else {
            value as u16
        }
    }

    let mut row = 0;
    let mut col = 0;
    let mut first = true;
    for cell in input_data.as_slice().iter() {
        if cell.new_row && !first {
            row += 1;
            col = 0;
        }
        first = false;

        if cell.row != i_slint_common::ROW_COL_AUTO {
            let cell_row = clamp_to_u16(cell.row, "row", &mut errors);
            if row != cell_row {
                row = cell_row;
                col = 0;
            }
        }
        if cell.col != i_slint_common::ROW_COL_AUTO {
            col = clamp_to_u16(cell.col, "col", &mut errors);
        }

        let colspan = clamp_to_u16(cell.colspan, "colspan", &mut errors);
        let rowspan = clamp_to_u16(cell.rowspan, "rowspan", &mut errors);
        col = col.min(u16::MAX - colspan); // ensure col + colspan doesn't overflow
        generator.add(col, colspan, row, rowspan);
        col += colspan;
    }
    (organized_data, errors)
}

/// Layout cache generator for box layouts.
/// The layout cache generator inserts the pos and size into the result array (which becomes the layout cache property),
/// including the indirections for repeated items (so that the x,y,width,height properties for repeated items
/// can point to indices known at compile time, those that contain the indirections)
/// Example: for repeater_indices=[1,4] (meaning that item at index 1 is repeated 4 times),
/// result=[0.0, 80.0, 4.0, 5.0, 80.0, 80.0, 160.0, 80.0, 240.0, 80.0, 320.0, 80.0]
///  i.e. pos1, width1, jump to idx 4, jump to idx 5, pos2, width2, pos3, width3, pos4, width4, pos5, width5
struct LayoutCacheGenerator<'a> {
    // Input
    repeater_indices: &'a [u32],
    // An always increasing counter, the index of the cell being added
    counter: usize,
    // The index/2 in result in which we should add the next repeated item
    repeat_offset: usize,
    // The index/2 in repeater_indices
    next_rep: usize,
    // The index/2 in result in which we should add the next non-repeated item
    current_offset: usize,
    // Output
    result: &'a mut SharedVector<Coord>,
}

impl<'a> LayoutCacheGenerator<'a> {
    fn new(repeater_indices: &'a [u32], result: &'a mut SharedVector<Coord>) -> Self {
        let total_repeated_cells: usize = repeater_indices
            .chunks(2)
            .map(|chunk| chunk.get(1).copied().unwrap_or(0) as usize)
            .sum();
        assert!(result.len() >= total_repeated_cells * 2);
        let repeat_offset = result.len() / 2 - total_repeated_cells;
        Self { repeater_indices, counter: 0, repeat_offset, next_rep: 0, current_offset: 0, result }
    }
    fn add(&mut self, pos: Coord, size: Coord) {
        let res = self.result.make_mut_slice();
        let o = loop {
            if let Some(nr) = self.repeater_indices.get(self.next_rep * 2) {
                let nr = *nr as usize;
                if nr == self.counter {
                    // Write jump entry
                    for o in 0..2 {
                        res[self.current_offset * 2 + o] = (self.repeat_offset * 2 + o) as _;
                    }
                    self.current_offset += 1;
                }
                if self.counter >= nr {
                    let rep_count = self.repeater_indices[self.next_rep * 2 + 1] as usize;
                    if self.counter - nr == rep_count {
                        self.repeat_offset += rep_count;
                        self.next_rep += 1;
                        continue;
                    }
                    let offset = self.repeat_offset + (self.counter - nr);
                    break offset;
                }
            }
            self.current_offset += 1;
            break self.current_offset - 1;
        };
        res[o * 2] = pos;
        res[o * 2 + 1] = size;
        self.counter += 1;
    }
}

/// Two-level indirection layout cache generator for grid layouts with repeaters.
/// Uses 2-level indirection: cache[cache[jump] + ri * stride + child_offset]
/// Each jump cell stores [data_base, stride] where stride = step * 2.
struct GridLayoutCacheGenerator<'a> {
    // Input
    repeater_indices: &'a [u32],
    repeater_steps: &'a [u32],
    // An always increasing counter, the index of the cell being added
    counter: usize,
    // The f32 position in result for the next repeater's dynamic data section
    repeat_f32_offset: usize,
    // The index/2 in repeater_indices
    next_rep: usize,
    // The cell index (index/2) in result for the next non-repeated item
    current_offset: usize,
    // Output
    result: &'a mut SharedVector<Coord>,
}

impl<'a> GridLayoutCacheGenerator<'a> {
    fn new(
        repeater_indices: &'a [u32],
        repeater_steps: &'a [u32],
        static_cells: usize,
        num_repeaters: usize,
        total_repeated_cells_count: usize,
        result: &'a mut SharedVector<Coord>,
    ) -> Self {
        result.resize((static_cells + num_repeaters + total_repeated_cells_count) * 2, 0 as _);
        let repeat_f32_offset = (static_cells + num_repeaters) * 2;
        Self {
            repeater_indices,
            repeater_steps,
            counter: 0,
            repeat_f32_offset,
            next_rep: 0,
            current_offset: 0,
            result,
        }
    }
    fn add(&mut self, pos: Coord, size: Coord) {
        let res = self.result.make_mut_slice();
        loop {
            if let Some(nr) = self.repeater_indices.get(self.next_rep * 2) {
                let nr = *nr as usize;
                let step = self.repeater_steps.get(self.next_rep).copied().unwrap_or(1) as usize;
                let rep_count = self.repeater_indices[self.next_rep * 2 + 1] as usize;

                if nr == self.counter {
                    // First cell of this repeater
                    let data_f32_start = self.repeat_f32_offset;

                    // Write jump cell: [data_base, 0]
                    res[self.current_offset * 2] = data_f32_start as _;
                    self.current_offset += 1;
                }
                if self.counter >= nr {
                    let cells_in_repeater = rep_count * step;
                    if self.counter - nr == cells_in_repeater {
                        // Past the end of this repeater — advance past data
                        self.repeat_f32_offset += cells_in_repeater * 2;
                        self.next_rep += 1;
                        continue;
                    }
                    // Write data at the position determined by row/col within repeater
                    let cell_in_rep = self.counter - nr;
                    let row_in_rep = cell_in_rep / step;
                    let col_in_rep = cell_in_rep % step;
                    let data_f32_start = self.repeat_f32_offset;
                    let f32_pos = data_f32_start + row_in_rep * step * 2 + col_in_rep * 2;
                    res[f32_pos] = pos;
                    res[f32_pos + 1] = size;
                    self.counter += 1;
                    return;
                }
            }
            // Non-repeated cell
            res[self.current_offset * 2] = pos;
            res[self.current_offset * 2 + 1] = size;
            self.current_offset += 1;
            self.counter += 1;
            return;
        }
    }
}

/// return, an array which is of size `data.cells.len() * 2` which for each cell stores:
/// pos (x or y), size (width or height)
pub fn solve_grid_layout(
    data: &GridLayoutData,
    constraints: Slice<LayoutItemInfo>,
    orientation: Orientation,
    repeater_indices: Slice<u32>,
    repeater_steps: Slice<u32>,
) -> SharedVector<Coord> {
    let mut layout_data = grid_internal::to_layout_data(
        &data.organized_data,
        constraints,
        orientation,
        repeater_indices,
        repeater_steps,
        data.spacing,
        Some(data.size),
    );

    if layout_data.is_empty() {
        return Default::default();
    }

    grid_internal::layout_items(
        &mut layout_data,
        data.padding.begin,
        data.size - (data.padding.begin + data.padding.end),
        data.spacing,
    );

    let mut result = SharedVector::<Coord>::default();
    let num_repeaters = repeater_indices.len() / 2;
    let total_repeated_cells =
        total_repeated_cells(repeater_indices.as_slice(), repeater_steps.as_slice());
    let static_cells = constraints.len() - total_repeated_cells;
    let mut generator = GridLayoutCacheGenerator::new(
        repeater_indices.as_slice(),
        repeater_steps.as_slice(),
        static_cells,
        num_repeaters,
        total_repeated_cells,
        &mut result,
    );

    for idx in 0..constraints.len() {
        let (col_or_row, span) = data.organized_data.col_or_row_and_span(
            idx,
            orientation,
            &repeater_indices,
            &repeater_steps,
        );
        let cdata = &layout_data[col_or_row as usize];
        let size = if span > 0 {
            let last_cell = &layout_data[col_or_row as usize + span as usize - 1];
            last_cell.pos + last_cell.size - cdata.pos
        } else {
            0 as Coord
        };
        generator.add(cdata.pos, size);
    }
    result
}

pub fn grid_layout_info(
    organized_data: GridLayoutOrganizedData, // not & because the code generator doesn't support it in ExtraBuiltinFunctionCall
    constraints: Slice<LayoutItemInfo>,
    repeater_indices: Slice<u32>,
    repeater_steps: Slice<u32>,
    spacing: Coord,
    padding: &Padding,
    orientation: Orientation,
) -> LayoutInfo {
    let layout_data = grid_internal::to_layout_data(
        &organized_data,
        constraints,
        orientation,
        repeater_indices,
        repeater_steps,
        spacing,
        None,
    );
    if layout_data.is_empty() {
        let mut info = LayoutInfo::default();
        info.min = padding.begin + padding.end;
        info.preferred = info.min;
        info.max = info.min;
        return info;
    }
    let spacing_w = spacing * (layout_data.len() - 1) as Coord + padding.begin + padding.end;
    let min = layout_data.iter().map(|data| data.min).sum::<Coord>() + spacing_w;
    let max = layout_data.iter().map(|data| data.max).fold(spacing_w, Saturating::add);
    let preferred = layout_data.iter().map(|data| data.pref).sum::<Coord>() + spacing_w;
    let stretch = layout_data.iter().map(|data| data.stretch).sum::<f32>();
    LayoutInfo { min, max, min_percent: 0 as _, max_percent: 100 as _, preferred, stretch }
}

#[repr(C)]
#[derive(Debug)]
/// The BoxLayoutData is used to represent both a Horizontal and Vertical layout.
/// The width/height x/y correspond to that of a horizontal layout.
/// For vertical layout, they are inverted
pub struct BoxLayoutData<'a> {
    pub size: Coord,
    pub spacing: Coord,
    pub padding: Padding,
    pub alignment: LayoutAlignment,
    pub cells: Slice<'a, LayoutItemInfo>,
}

#[repr(C)]
#[derive(Debug)]
/// The FlexBoxLayoutData is used for a flex layout with wrapping.
pub struct FlexBoxLayoutData<'a> {
    pub width: Coord,
    pub height: Coord,
    pub spacing_h: Coord,
    pub spacing_v: Coord,
    pub padding_h: Padding,
    pub padding_v: Padding,
    pub alignment: LayoutAlignment,
    pub direction: FlexDirection,
    pub align_content: FlexAlignContent,
    pub align_items: FlexAlignItems,
    /// Horizontal constraints (width) for each cell
    pub cells_h: Slice<'a, LayoutItemInfo>,
    /// Vertical constraints (height) for each cell
    pub cells_v: Slice<'a, LayoutItemInfo>,
}

#[repr(C)]
#[derive(Default, Debug, Clone)]
/// The information about a single item in a layout
/// For now this only contains the LayoutInfo constraints, but could be extended in the future
pub struct LayoutItemInfo {
    pub constraint: LayoutInfo,
}

/// Solve a BoxLayout
pub fn solve_box_layout(data: &BoxLayoutData, repeater_indices: Slice<u32>) -> SharedVector<Coord> {
    let mut result = SharedVector::<Coord>::default();
    result.resize(data.cells.len() * 2 + repeater_indices.len(), 0 as _);

    if data.cells.is_empty() {
        return result;
    }

    let mut layout_data: Vec<_> = data
        .cells
        .iter()
        .map(|c| {
            let min = c.constraint.min.max(c.constraint.min_percent * data.size / 100 as Coord);
            let max = c.constraint.max.min(c.constraint.max_percent * data.size / 100 as Coord);
            grid_internal::LayoutData {
                min,
                max,
                pref: c.constraint.preferred.min(max).max(min),
                stretch: c.constraint.stretch,
                ..Default::default()
            }
        })
        .collect();

    let size_without_padding = data.size - data.padding.begin - data.padding.end;
    let pref_size: Coord = layout_data.iter().map(|it| it.pref).sum();
    let num_spacings = (layout_data.len() - 1) as Coord;
    let spacings = data.spacing * num_spacings;

    let align = match data.alignment {
        LayoutAlignment::Stretch => {
            grid_internal::layout_items(
                &mut layout_data,
                data.padding.begin,
                size_without_padding,
                data.spacing,
            );
            None
        }
        _ if size_without_padding <= pref_size + spacings => {
            grid_internal::layout_items(
                &mut layout_data,
                data.padding.begin,
                size_without_padding,
                data.spacing,
            );
            None
        }
        LayoutAlignment::Center => Some((
            data.padding.begin + (size_without_padding - pref_size - spacings) / 2 as Coord,
            data.spacing,
        )),
        LayoutAlignment::Start => Some((data.padding.begin, data.spacing)),
        LayoutAlignment::End => {
            Some((data.padding.begin + (size_without_padding - pref_size - spacings), data.spacing))
        }
        LayoutAlignment::SpaceBetween => {
            Some((data.padding.begin, (size_without_padding - pref_size) / num_spacings))
        }
        LayoutAlignment::SpaceAround => {
            let spacing = (size_without_padding - pref_size) / (num_spacings + 1 as Coord);
            Some((data.padding.begin + spacing / 2 as Coord, spacing))
        }
        LayoutAlignment::SpaceEvenly => {
            let spacing = (size_without_padding - pref_size) / (num_spacings + 2 as Coord);
            Some((data.padding.begin + spacing, spacing))
        }
    };
    if let Some((mut pos, spacing)) = align {
        for it in &mut layout_data {
            it.pos = pos;
            it.size = it.pref;
            pos += spacing + it.size;
        }
    }

    let mut generator = LayoutCacheGenerator::new(&repeater_indices, &mut result);
    for layout in layout_data.iter() {
        generator.add(layout.pos, layout.size);
    }
    result
}

/// Return the LayoutInfo for a BoxLayout with the given cells.
pub fn box_layout_info(
    cells: Slice<LayoutItemInfo>,
    spacing: Coord,
    padding: &Padding,
    alignment: LayoutAlignment,
) -> LayoutInfo {
    let count = cells.len();
    let is_stretch = alignment == LayoutAlignment::Stretch;
    if count < 1 {
        let mut info = LayoutInfo::default();
        info.min = padding.begin + padding.end;
        info.preferred = info.min;
        if is_stretch {
            info.max = info.min;
        }
        return info;
    };
    let extra_w = padding.begin + padding.end + spacing * (count - 1) as Coord;
    let min = cells.iter().map(|c| c.constraint.min).sum::<Coord>() + extra_w;
    let max = if is_stretch {
        (cells.iter().map(|c| c.constraint.max).fold(extra_w, Saturating::add)).max(min)
    } else {
        Coord::MAX
    };
    let preferred = cells.iter().map(|c| c.constraint.preferred_bounded()).sum::<Coord>() + extra_w;
    let stretch = cells.iter().map(|c| c.constraint.stretch).sum::<f32>();
    LayoutInfo { min, max, min_percent: 0 as _, max_percent: 100 as _, preferred, stretch }
}

pub fn box_layout_info_ortho(cells: Slice<LayoutItemInfo>, padding: &Padding) -> LayoutInfo {
    let extra_w = padding.begin + padding.end;
    let mut fold =
        cells.iter().fold(LayoutInfo { stretch: f32::MAX, ..Default::default() }, |a, b| {
            a.merge(&b.constraint)
        });
    fold.max = fold.max.max(fold.min);
    fold.preferred = fold.preferred.clamp(fold.min, fold.max);
    fold.min += extra_w;
    fold.max = Saturating::add(fold.max, extra_w);
    fold.preferred += extra_w;
    fold
}

/// Helper module for taffy-based flexbox layout
mod flexbox_taffy {
    use super::{
        Coord, FlexAlignContent, FlexAlignItems, LayoutAlignment, LayoutItemInfo, Padding, Slice,
    };
    use alloc::vec::Vec;
    pub use taffy::prelude::FlexDirection as TaffyFlexDirection;
    use taffy::prelude::{
        AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexWrap, LengthPercentage,
        NodeId, Rect, Size, Style, TaffyTree,
    };

    /// Parameters for FlexboxTaffyBuilder::new
    pub struct FlexBoxLayoutParams<'a> {
        pub cells_h: &'a Slice<'a, LayoutItemInfo>,
        pub cells_v: &'a Slice<'a, LayoutItemInfo>,
        pub spacing_h: Coord,
        pub spacing_v: Coord,
        pub padding_h: &'a Padding,
        pub padding_v: &'a Padding,
        pub alignment: LayoutAlignment,
        pub align_content: FlexAlignContent,
        pub align_items: FlexAlignItems,
        pub flex_direction: TaffyFlexDirection,
        pub container_width: Option<Coord>,
        pub container_height: Option<Coord>,
    }

    /// Build a taffy tree from Slint layout constraints
    pub struct FlexboxTaffyBuilder {
        pub taffy: TaffyTree<()>,
        pub children: Vec<NodeId>,
        pub container: NodeId,
    }

    impl FlexboxTaffyBuilder {
        /// Create a new flexbox layout tree from item constraints
        pub fn new(params: FlexBoxLayoutParams) -> Self {
            let mut taffy = TaffyTree::<()>::new();

            // Create child nodes from Slint constraints
            let children: Vec<NodeId> = params
                .cells_h
                .iter()
                .enumerate()
                .map(|(idx, cell_h)| {
                    let cell_v = params.cells_v.get(idx);
                    let h_constraint = &cell_h.constraint;
                    let v_constraint = cell_v.map(|c| &c.constraint);

                    // Use preferred_bounded() which clamps preferred to min/max bounds
                    let preferred_width = h_constraint.preferred_bounded();
                    let preferred_height =
                        v_constraint.map(|vc| vc.preferred_bounded()).unwrap_or(0 as Coord);

                    // flex_basis depends on direction
                    let flex_basis = match params.flex_direction {
                        TaffyFlexDirection::Row | TaffyFlexDirection::RowReverse => {
                            Dimension::Length(preferred_width as _)
                        }
                        TaffyFlexDirection::Column | TaffyFlexDirection::ColumnReverse => {
                            Dimension::Length(preferred_height as _)
                        }
                    };

                    taffy
                        .new_leaf(Style {
                            flex_basis,
                            size: Size {
                                width: match params.flex_direction {
                                    TaffyFlexDirection::Column
                                    | TaffyFlexDirection::ColumnReverse => {
                                        if preferred_width > 0 as Coord {
                                            Dimension::Length(preferred_width as _)
                                        } else {
                                            Dimension::Auto
                                        }
                                    }
                                    _ => Dimension::Auto,
                                },
                                height: match params.flex_direction {
                                    TaffyFlexDirection::Row | TaffyFlexDirection::RowReverse => {
                                        if preferred_height > 0 as Coord {
                                            Dimension::Length(preferred_height as _)
                                        } else {
                                            Dimension::Auto
                                        }
                                    }
                                    _ => Dimension::Auto,
                                },
                            },
                            min_size: Size {
                                width: Dimension::Length(h_constraint.min as _),
                                height: Dimension::Length(
                                    v_constraint.map(|vc| vc.min as f32).unwrap_or(0.0),
                                ),
                            },
                            max_size: Size {
                                width: if h_constraint.max < Coord::MAX {
                                    Dimension::Length(h_constraint.max as _)
                                } else {
                                    Dimension::Auto
                                },
                                height: if let Some(vc) = v_constraint {
                                    if vc.max < Coord::MAX {
                                        Dimension::Length(vc.max as _)
                                    } else {
                                        Dimension::Auto
                                    }
                                } else {
                                    Dimension::Auto
                                },
                            },
                            flex_grow: 0.0,
                            flex_shrink: 0.0,
                            ..Default::default()
                        })
                        .unwrap() // cannot fail
                })
                .collect();

            // Create container node
            let container = taffy
                .new_with_children(
                    Style {
                        display: Display::Flex,
                        flex_direction: params.flex_direction,
                        flex_wrap: FlexWrap::Wrap,
                        justify_content: Some(match params.alignment {
                            // Start/End map to FlexStart/FlexEnd to respect flex direction (including reverse)
                            // AlignContent::Start/End would ignore direction and always use writing mode
                            LayoutAlignment::Start => AlignContent::FlexStart,
                            LayoutAlignment::End => AlignContent::FlexEnd,
                            LayoutAlignment::Center => AlignContent::Center,
                            LayoutAlignment::Stretch => AlignContent::Stretch,
                            LayoutAlignment::SpaceBetween => AlignContent::SpaceBetween,
                            LayoutAlignment::SpaceAround => AlignContent::SpaceAround,
                            LayoutAlignment::SpaceEvenly => AlignContent::SpaceEvenly,
                        }),
                        align_items: Some(match params.align_items {
                            FlexAlignItems::Stretch => AlignItems::Stretch,
                            FlexAlignItems::Start => AlignItems::FlexStart,
                            FlexAlignItems::End => AlignItems::FlexEnd,
                            FlexAlignItems::Center => AlignItems::Center,
                        }),
                        align_content: Some(match params.align_content {
                            FlexAlignContent::Stretch => AlignContent::Stretch,
                            FlexAlignContent::Start => AlignContent::FlexStart,
                            FlexAlignContent::End => AlignContent::FlexEnd,
                            FlexAlignContent::Center => AlignContent::Center,
                        }),
                        gap: Size {
                            width: LengthPercentage::Length(params.spacing_h as _),
                            height: LengthPercentage::Length(params.spacing_v as _),
                        },
                        padding: Rect {
                            left: LengthPercentage::Length(params.padding_h.begin as _),
                            right: LengthPercentage::Length(params.padding_h.end as _),
                            top: LengthPercentage::Length(params.padding_v.begin as _),
                            bottom: LengthPercentage::Length(params.padding_v.end as _),
                        },
                        size: Size {
                            width: params
                                .container_width
                                .map(|w| Dimension::Length(w as _))
                                .unwrap_or(Dimension::Auto),
                            height: params
                                .container_height
                                .map(|h| Dimension::Length(h as _))
                                .unwrap_or(Dimension::Auto),
                        },
                        ..Default::default()
                    },
                    &children,
                )
                .unwrap(); // cannot fail

            Self { taffy, children, container }
        }

        /// Compute the layout with the given available space
        pub fn compute_layout(&mut self, available_width: Coord, available_height: Coord) {
            self.taffy
                .compute_layout(
                    self.container,
                    taffy::prelude::Size {
                        width: if available_width < Coord::MAX {
                            AvailableSpace::Definite(available_width as _)
                        } else {
                            AvailableSpace::MaxContent
                        },
                        height: if available_height < Coord::MAX {
                            AvailableSpace::Definite(available_height as _)
                        } else {
                            AvailableSpace::MaxContent
                        },
                    },
                )
                .unwrap_or_else(|e| {
                    crate::debug_log!("FlexBox layout computation error: {}", e);
                });
        }

        /// Get the computed container size
        pub fn container_size(&self) -> (Coord, Coord) {
            let layout = self.taffy.layout(self.container).unwrap();
            (layout.size.width as Coord, layout.size.height as Coord)
        }

        /// Get the geometry for a specific child
        pub fn child_geometry(&self, idx: usize) -> (Coord, Coord, Coord, Coord) {
            let layout = self.taffy.layout(self.children[idx]).unwrap();
            (
                layout.location.x as Coord,
                layout.location.y as Coord,
                layout.size.width as Coord,
                layout.size.height as Coord,
            )
        }
    }
}

/// A cache generator for FlexBoxLayout that handles 4 values per item (x, y, width, height)
struct FlexBoxLayoutCacheGenerator<'a> {
    // Input
    repeater_indices: &'a [u32],
    // An always increasing counter, the index of the cell being added
    counter: usize,
    // The index/4 in result in which we should add the next repeated item
    repeat_offset: usize,
    // The index/4 in repeater_indices
    next_rep: usize,
    // The index/4 in result in which we should add the next non-repeated item
    current_offset: usize,
    // Output
    result: &'a mut SharedVector<Coord>,
}

impl<'a> FlexBoxLayoutCacheGenerator<'a> {
    fn new(repeater_indices: &'a [u32], result: &'a mut SharedVector<Coord>) -> Self {
        // Calculate total repeated cells (count for each repeater)
        let total_repeated_cells: usize = repeater_indices
            .chunks(2)
            .map(|chunk| chunk.get(1).copied().unwrap_or(0) as usize)
            .sum();
        assert!(result.len() >= total_repeated_cells * 4);
        let repeat_offset = result.len() / 4 - total_repeated_cells;
        Self { repeater_indices, counter: 0, repeat_offset, next_rep: 0, current_offset: 0, result }
    }

    fn add(&mut self, x: Coord, y: Coord, w: Coord, h: Coord) {
        let res = self.result.make_mut_slice();
        let o = loop {
            if let Some(nr) = self.repeater_indices.get(self.next_rep * 2) {
                let nr = *nr as usize;
                if nr == self.counter {
                    // Write jump entries for repeater start
                    // Store the base offset (index into the repeated data region)
                    res[self.current_offset * 4] = (self.repeat_offset * 4) as Coord;
                    res[self.current_offset * 4 + 1] = (self.repeat_offset * 4 + 1) as Coord;
                    res[self.current_offset * 4 + 2] = (self.repeat_offset * 4 + 2) as Coord;
                    res[self.current_offset * 4 + 3] = (self.repeat_offset * 4 + 3) as Coord;
                    self.current_offset += 1;
                }
                if self.counter >= nr {
                    let rep_count = self.repeater_indices[self.next_rep * 2 + 1] as usize;
                    if self.counter - nr == rep_count {
                        // Advance repeat_offset past this repeater's data before moving to next
                        self.repeat_offset += rep_count;
                        self.next_rep += 1;
                        continue;
                    }
                    // Calculate offset into repeated data
                    let cell_in_rep = self.counter - nr;
                    let offset = self.repeat_offset + cell_in_rep;
                    break offset;
                }
            }
            self.current_offset += 1;
            break self.current_offset - 1;
        };
        res[o * 4] = x;
        res[o * 4 + 1] = y;
        res[o * 4 + 2] = w;
        res[o * 4 + 3] = h;
        self.counter += 1;
    }
}

/// Solve a FlexBoxLayout using Taffy
/// Returns: [x1, y1, w1, h1, x2, y2, w2, h2, ...] for each item
pub fn solve_flexbox_layout(
    data: &FlexBoxLayoutData,
    repeater_indices: Slice<u32>,
) -> SharedVector<Coord> {
    // 4 values per item: x, y, width, height
    let mut result = SharedVector::<Coord>::default();
    result.resize(data.cells_h.len() * 4 + repeater_indices.len() * 2, 0 as _);

    if data.cells_h.is_empty() {
        return result;
    }

    let taffy_direction = match data.direction {
        FlexDirection::Row => flexbox_taffy::TaffyFlexDirection::Row,
        FlexDirection::RowReverse => flexbox_taffy::TaffyFlexDirection::RowReverse,
        FlexDirection::Column => flexbox_taffy::TaffyFlexDirection::Column,
        FlexDirection::ColumnReverse => flexbox_taffy::TaffyFlexDirection::ColumnReverse,
    };

    let (container_width, container_height) = (
        if data.width > 0 as Coord { Some(data.width) } else { None },
        if data.height > 0 as Coord { Some(data.height) } else { None },
    );

    let mut builder = flexbox_taffy::FlexboxTaffyBuilder::new(flexbox_taffy::FlexBoxLayoutParams {
        cells_h: &data.cells_h,
        cells_v: &data.cells_v,
        spacing_h: data.spacing_h,
        spacing_v: data.spacing_v,
        padding_h: &data.padding_h,
        padding_v: &data.padding_v,
        alignment: data.alignment,
        align_content: data.align_content,
        align_items: data.align_items,
        flex_direction: taffy_direction,
        container_width,
        container_height,
    });

    let (available_width, available_height) = match data.direction {
        FlexDirection::Row | FlexDirection::RowReverse => (data.width, Coord::MAX),
        FlexDirection::Column | FlexDirection::ColumnReverse => (Coord::MAX, data.height),
    };

    builder.compute_layout(available_width, available_height);

    // Extract results using the cache generator to handle repeaters
    let mut generator = FlexBoxLayoutCacheGenerator::new(&repeater_indices, &mut result);
    for idx in 0..data.cells_h.len() {
        let (x, y, w, h) = builder.child_geometry(idx);
        generator.add(x, y, w, h);
    }

    result
}

/// Return LayoutInfo (i.e. min, preferred, max etc.) for a FlexBoxLayout
/// This handles both main-axis (simple) and cross-axis (wrapping-aware) cases.
/// The constraint_size is the perpendicular dimension to orientation:
/// - For Horizontal orientation: constraint_size is height
/// - For Vertical orientation: constraint_size is width
///
/// The constraint_size is ignored for main-axis calculation.
pub fn flexbox_layout_info(
    cells_h: Slice<LayoutItemInfo>,
    cells_v: Slice<LayoutItemInfo>,
    spacing_h: Coord,
    spacing_v: Coord,
    padding_h: &Padding,
    padding_v: &Padding,
    orientation: Orientation,
    direction: FlexDirection,
    constraint_size: Coord,
) -> LayoutInfo {
    if cells_h.is_empty() {
        assert!(cells_v.is_empty());
        let padding = match orientation {
            Orientation::Horizontal => padding_h,
            Orientation::Vertical => padding_v,
        };
        let pad = padding.begin + padding.end;
        return LayoutInfo { min: pad, preferred: pad, max: pad, ..Default::default() };
    }

    // Min size is the maximum of any single item (since they can wrap) plus padding
    let (cells, padding, spacing) = match (direction, orientation) {
        (FlexDirection::Row | FlexDirection::RowReverse, Orientation::Horizontal)
        | (FlexDirection::Column | FlexDirection::ColumnReverse, Orientation::Vertical) => {
            (&cells_h, padding_h, spacing_h)
        }
        _ => (&cells_v, padding_v, spacing_v),
    };
    let extra_pad = padding.begin + padding.end;
    let min =
        cells.iter().map(|c| c.constraint.min).fold(0.0 as Coord, |a, b| a.max(b)) + extra_pad;

    // Determine if we're asking for main-axis or cross-axis
    let is_main_axis = matches!(
        (direction, orientation),
        (FlexDirection::Row | FlexDirection::RowReverse, Orientation::Horizontal)
            | (FlexDirection::Column | FlexDirection::ColumnReverse, Orientation::Vertical)
    );

    // The main-axis constraint determines how items wrap.
    // For main-axis queries, use sqrt of total item area as an approximation.
    // For cross-axis queries, use the provided constraint_size.
    let main_axis_constraint = if is_main_axis {
        // constraint_size is not used for the main axis
        let total_area = cells_h
            .iter()
            .map(|c| c.constraint.preferred_bounded())
            .zip(cells_v.iter().map(|c| c.constraint.preferred_bounded()))
            .map(|(h, v)| h * v)
            .sum::<Coord>();
        let count = cells.len();
        Float::sqrt(total_area as f32) as Coord + spacing * (count - 1) as Coord + extra_pad
    } else {
        constraint_size
    };

    let taffy_direction = match direction {
        FlexDirection::Row => flexbox_taffy::TaffyFlexDirection::Row,
        FlexDirection::RowReverse => flexbox_taffy::TaffyFlexDirection::RowReverse,
        FlexDirection::Column => flexbox_taffy::TaffyFlexDirection::Column,
        FlexDirection::ColumnReverse => flexbox_taffy::TaffyFlexDirection::ColumnReverse,
    };

    let (container_width, container_height) = match direction {
        FlexDirection::Row | FlexDirection::RowReverse => (Some(main_axis_constraint), None),
        FlexDirection::Column | FlexDirection::ColumnReverse => (None, Some(main_axis_constraint)),
    };

    let mut builder = flexbox_taffy::FlexboxTaffyBuilder::new(flexbox_taffy::FlexBoxLayoutParams {
        cells_h: &cells_h,
        cells_v: &cells_v,
        spacing_h,
        spacing_v,
        padding_h,
        padding_v,
        alignment: LayoutAlignment::Start,
        align_content: FlexAlignContent::Stretch,
        align_items: FlexAlignItems::Stretch,
        flex_direction: taffy_direction,
        container_width,
        container_height,
    });

    let (available_width, available_height) = match direction {
        FlexDirection::Row | FlexDirection::RowReverse => (main_axis_constraint, Coord::MAX),
        FlexDirection::Column | FlexDirection::ColumnReverse => (Coord::MAX, main_axis_constraint),
    };

    builder.compute_layout(available_width, available_height);

    let preferred = if is_main_axis {
        // For main-axis, container_size() returns max(content, available_space) for
        // multi-line layouts, giving back our approximation unchanged. Scan child
        // positions to find the actual extent of the widest row.
        let mut min_pos = Coord::MAX;
        let mut max_end = 0.0 as Coord;
        for i in 0..cells_h.len() {
            let (x, y, w, h) = builder.child_geometry(i);
            let (pos, size) = match orientation {
                Orientation::Horizontal => (x, w),
                Orientation::Vertical => (y, h),
            };
            min_pos = min_pos.min(pos);
            max_end = max_end.max(pos + size);
        }
        (max_end - min_pos) + padding.begin + padding.end
    } else {
        // For cross-axis, the queried dimension is Auto so container_size() returns
        // the content-based size directly.
        let (total_width, total_height) = builder.container_size();
        match orientation {
            Orientation::Horizontal => total_width,
            Orientation::Vertical => total_height,
        }
    };

    let stretch =
        if is_main_axis { cells.iter().map(|c| c.constraint.stretch).sum::<f32>() } else { 0.0 };

    LayoutInfo {
        min,
        max: Coord::MAX, // TODO?
        min_percent: 0 as _,
        max_percent: 100 as _,
        preferred,
        stretch,
    }
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_organize_grid_layout(
        input_data: Slice<GridLayoutInputData>,
        repeater_indices: Slice<u32>,
        repeater_steps: Slice<u32>,
        result: &mut GridLayoutOrganizedData,
    ) {
        *result = super::organize_grid_layout(input_data, repeater_indices, repeater_steps);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_organize_dialog_button_layout(
        input_data: Slice<GridLayoutInputData>,
        dialog_button_roles: Slice<DialogButtonRole>,
        result: &mut GridLayoutOrganizedData,
    ) {
        *result = super::organize_dialog_button_layout(input_data, dialog_button_roles);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_solve_grid_layout(
        data: &GridLayoutData,
        constraints: Slice<LayoutItemInfo>,
        orientation: Orientation,
        repeater_indices: Slice<u32>,
        repeater_steps: Slice<u32>,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_grid_layout(
            data,
            constraints,
            orientation,
            repeater_indices,
            repeater_steps,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_grid_layout_info(
        organized_data: &GridLayoutOrganizedData,
        constraints: Slice<LayoutItemInfo>,
        repeater_indices: Slice<u32>,
        repeater_steps: Slice<u32>,
        spacing: Coord,
        padding: &Padding,
        orientation: Orientation,
    ) -> LayoutInfo {
        super::grid_layout_info(
            organized_data.clone(),
            constraints,
            repeater_indices,
            repeater_steps,
            spacing,
            padding,
            orientation,
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_solve_box_layout(
        data: &BoxLayoutData,
        repeater_indices: Slice<u32>,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_box_layout(data, repeater_indices)
    }

    #[unsafe(no_mangle)]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn slint_box_layout_info(
        cells: Slice<LayoutItemInfo>,
        spacing: Coord,
        padding: &Padding,
        alignment: LayoutAlignment,
    ) -> LayoutInfo {
        super::box_layout_info(cells, spacing, padding, alignment)
    }

    #[unsafe(no_mangle)]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn slint_box_layout_info_ortho(
        cells: Slice<LayoutItemInfo>,
        padding: &Padding,
    ) -> LayoutInfo {
        super::box_layout_info_ortho(cells, padding)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_solve_flexbox_layout(
        data: &FlexBoxLayoutData,
        repeater_indices: Slice<u32>,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_flexbox_layout(data, repeater_indices)
    }

    #[unsafe(no_mangle)]
    /// Return LayoutInfo for a FlexBoxLayout with runtime direction support.
    pub extern "C" fn slint_flexbox_layout_info(
        cells_h: Slice<LayoutItemInfo>,
        cells_v: Slice<LayoutItemInfo>,
        spacing_h: Coord,
        spacing_v: Coord,
        padding_h: &Padding,
        padding_v: &Padding,
        orientation: Orientation,
        direction: FlexDirection,
        constraint_size: Coord,
    ) -> LayoutInfo {
        super::flexbox_layout_info(
            cells_h,
            cells_v,
            spacing_h,
            spacing_v,
            padding_h,
            padding_v,
            orientation,
            direction,
            constraint_size,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_from_organized_data(
        organized_data: &GridLayoutOrganizedData,
        num_cells: usize,
        repeater_indices: Slice<u32>,
        repeater_steps: Slice<u32>,
    ) -> Vec<(u16, u16, u16, u16)> {
        let mut result = Vec::new();
        for i in 0..num_cells {
            let col_and_span = organized_data.col_or_row_and_span(
                i,
                Orientation::Horizontal,
                &repeater_indices,
                &repeater_steps,
            );
            let row_and_span = organized_data.col_or_row_and_span(
                i,
                Orientation::Vertical,
                &repeater_indices,
                &repeater_steps,
            );
            result.push((col_and_span.0, col_and_span.1, row_and_span.0, row_and_span.1));
        }
        result
    }

    #[test]
    fn test_organized_data_generator_2_fixed_cells() {
        // 2 fixed cells
        let mut result = GridLayoutOrganizedData::default();
        let num_cells = 2;
        let mut generator = OrganizedDataGenerator::new(&[], &[], num_cells, 0, 0, &mut result);
        generator.add(0, 1, 0, 1);
        generator.add(1, 2, 0, 3);
        assert_eq!(result.as_slice(), &[0, 1, 0, 1, 1, 2, 0, 3]);

        let repeater_indices = Slice::from_slice(&[]);
        let empty_steps = Slice::from_slice(&[]);
        let collected_data =
            collect_from_organized_data(&result, num_cells, repeater_indices, empty_steps);
        assert_eq!(collected_data.as_slice(), &[(0, 1, 0, 1), (1, 2, 0, 3)]);

        assert_eq!(
            result.max_value(num_cells, Orientation::Horizontal, &repeater_indices, &empty_steps),
            3
        );
        assert_eq!(
            result.max_value(num_cells, Orientation::Vertical, &repeater_indices, &empty_steps),
            3
        );
    }

    #[test]
    fn test_organized_data_generator_1_fixed_cell_1_repeater() {
        // 4 cells: 1 fixed cell, 1 repeater with 3 repeated cells
        let mut result = GridLayoutOrganizedData::default();
        let num_cells = 4;
        let repeater_indices = &[1u32, 3u32];
        let mut generator =
            OrganizedDataGenerator::new(repeater_indices, &[], 1, 1, 3, &mut result);
        generator.add(0, 1, 0, 2); // fixed
        generator.add(1, 2, 1, 3); // repeated
        generator.add(1, 1, 2, 4);
        generator.add(2, 2, 3, 5);
        assert_eq!(
            result.as_slice(),
            &[
                0, 1, 0, 2, // fixed cell
                8, 0, 0, 0, // jump cell: data_base=8, stride=4 (step=1, epi=4)
                1, 2, 1, 3, // repeated cell 1
                1, 1, 2, 4, // repeated cell 2
                2, 2, 3, 5, // repeated cell 3
            ]
        );
        let repeater_indices = Slice::from_slice(repeater_indices);
        let empty_steps = Slice::from_slice(&[]);
        let collected_data =
            collect_from_organized_data(&result, num_cells, repeater_indices, empty_steps);
        assert_eq!(
            collected_data.as_slice(),
            &[(0, 1, 0, 2), (1, 2, 1, 3), (1, 1, 2, 4), (2, 2, 3, 5)]
        );

        assert_eq!(
            result.max_value(num_cells, Orientation::Horizontal, &repeater_indices, &empty_steps),
            4
        );
        assert_eq!(
            result.max_value(num_cells, Orientation::Vertical, &repeater_indices, &empty_steps),
            8
        );
    }

    #[test]

    fn test_organize_data_with_auto_and_spans() {
        let auto = i_slint_common::ROW_COL_AUTO;
        let input = std::vec![
            GridLayoutInputData { new_row: true, col: auto, row: auto, colspan: 2., rowspan: -1. },
            GridLayoutInputData { new_row: false, col: auto, row: auto, colspan: 1., rowspan: 2. },
            GridLayoutInputData { new_row: true, col: auto, row: auto, colspan: 2., rowspan: 1. },
            GridLayoutInputData { new_row: true, col: -2., row: 80000., colspan: 2., rowspan: 1. },
        ];
        let repeater_indices = Slice::from_slice(&[]);
        let (organized_data, errors) = organize_grid_layout_impl(
            Slice::from_slice(&input),
            repeater_indices,
            Slice::from_slice(&[]),
        );
        assert_eq!(
            organized_data.as_slice(),
            &[
                0, 2, 0, 0, // row 0, col 0, rowspan 0 (see below)
                2, 1, 0, 2, // row 0, col 2 (due to colspan of first cell)
                0, 2, 1, 1, // row 1, col 0
                0, 2, 65535, 1, // row 65535, col 0
            ]
        );
        assert_eq!(errors.len(), 3);
        // Note that a rowspan of 0 is valid, it means the cell doesn't occupy any row
        assert_eq!(errors[0], "cell rowspan -1 is negative, clamping to 0");
        assert_eq!(errors[1], "cell row 80000 is too large, clamping to 65535");
        assert_eq!(errors[2], "cell col -2 is negative, clamping to 0");
        let empty_steps = Slice::from_slice(&[]);
        let collected_data = collect_from_organized_data(
            &organized_data,
            input.len(),
            repeater_indices,
            empty_steps,
        );
        assert_eq!(
            collected_data.as_slice(),
            &[(0, 2, 0, 0), (2, 1, 0, 2), (0, 2, 1, 1), (0, 2, 65535, 1)]
        );
        assert_eq!(
            organized_data.max_value(3, Orientation::Horizontal, &repeater_indices, &empty_steps),
            3
        );
        assert_eq!(
            organized_data.max_value(3, Orientation::Vertical, &repeater_indices, &empty_steps),
            2
        );
    }

    #[test]
    fn test_organize_data_1_empty_repeater() {
        // Row { Text {}    if false: Text {} }, this test shows why we need i32 for cell_nr_adj
        let auto = i_slint_common::ROW_COL_AUTO;
        let cell =
            GridLayoutInputData { new_row: true, col: auto, row: auto, colspan: 1., rowspan: 1. };
        let input = std::vec![cell];
        let repeater_indices = Slice::from_slice(&[1u32, 0u32]);
        let (organized_data, errors) = organize_grid_layout_impl(
            Slice::from_slice(&input),
            repeater_indices,
            Slice::from_slice(&[]),
        );
        assert_eq!(
            organized_data.as_slice(),
            &[
                0, 1, 0, 1, // fixed
                0, 0, 0, 0
            ] // jump to repeater data (not used)
        );
        assert_eq!(errors.len(), 0);
        let empty_steps = Slice::from_slice(&[]);
        let collected_data = collect_from_organized_data(
            &organized_data,
            input.len(),
            repeater_indices,
            empty_steps,
        );
        assert_eq!(collected_data.as_slice(), &[(0, 1, 0, 1)]);
        assert_eq!(
            organized_data.max_value(1, Orientation::Horizontal, &repeater_indices, &empty_steps),
            1
        );
    }

    #[test]
    fn test_organize_data_4_repeaters() {
        let auto = i_slint_common::ROW_COL_AUTO;
        let mut cell =
            GridLayoutInputData { new_row: true, col: auto, row: auto, colspan: 1., rowspan: 1. };
        let mut input = std::vec![cell.clone()];
        for _ in 0..8 {
            cell.new_row = false;
            input.push(cell.clone());
        }
        let repeater_indices = Slice::from_slice(&[0u32, 0u32, 1u32, 4u32, 6u32, 2u32, 8u32, 0u32]);
        let (organized_data, errors) = organize_grid_layout_impl(
            Slice::from_slice(&input),
            repeater_indices,
            Slice::from_slice(&[]),
        );
        assert_eq!(
            organized_data.as_slice(),
            &[
                28, 0, 0, 0, // rep0 jump: data at 28, stride=4 (empty)
                0, 1, 0, 1, // fixed cell (col=0)
                28, 0, 0, 0, // rep1 jump: data at 28, stride=4 (4 rows)
                5, 1, 0, 1, // fixed cell (col=5)
                44, 0, 0, 0, // rep2 jump: data at 44, stride=4 (2 rows)
                52, 0, 0, 0, // rep3 jump: data at 52, stride=4 (empty)
                8, 1, 0, 1, // fixed cell (col=8)
                1, 1, 0, 1, // rep1 row 0
                2, 1, 0, 1, // rep1 row 1
                3, 1, 0, 1, // rep1 row 2
                4, 1, 0, 1, // rep1 row 3
                6, 1, 0, 1, // rep2 row 0
                7, 1, 0, 1, // rep2 row 1
            ]
        );
        assert_eq!(errors.len(), 0);
        let empty_steps = Slice::from_slice(&[]);
        let collected_data = collect_from_organized_data(
            &organized_data,
            input.len(),
            repeater_indices,
            empty_steps,
        );
        assert_eq!(
            collected_data.as_slice(),
            &[
                (0, 1, 0, 1),
                (1, 1, 0, 1),
                (2, 1, 0, 1),
                (3, 1, 0, 1),
                (4, 1, 0, 1),
                (5, 1, 0, 1),
                (6, 1, 0, 1),
                (7, 1, 0, 1),
                (8, 1, 0, 1),
            ]
        );
        let empty_steps = Slice::from_slice(&[]);
        assert_eq!(
            organized_data.max_value(
                input.len(),
                Orientation::Horizontal,
                &repeater_indices,
                &empty_steps
            ),
            9
        );
    }

    #[test]
    fn test_organize_data_repeated_rows() {
        let auto = i_slint_common::ROW_COL_AUTO;
        let mut input = Vec::new();
        let num_rows: u32 = 3;
        let num_columns: u32 = 2;
        // 3 rows of 2 columns each
        for _ in 0..num_rows {
            let mut cell = GridLayoutInputData {
                new_row: true,
                col: auto,
                row: auto,
                colspan: 1.,
                rowspan: 1.,
            };
            input.push(cell.clone());
            cell.new_row = false;
            input.push(cell.clone());
        }
        // Repeater 0: starts at index 0, has 3 instances of 2 elements
        let repeater_indices_arr = [0_u32, num_rows];
        let repeater_steps_arr = [num_columns];
        let repeater_steps = Slice::from_slice(&repeater_steps_arr);
        let repeater_indices = Slice::from_slice(&repeater_indices_arr);
        let (organized_data, errors) =
            organize_grid_layout_impl(Slice::from_slice(&input), repeater_indices, repeater_steps);
        assert_eq!(
            organized_data.as_slice(),
            &[
                4, 0, 0, 0, // jump cell: data at u16 idx 4, stride=8 (=step*4=2*4)
                0, 1, 0, 1, 1, 1, 0, 1, // row 0: col 0, col 1
                0, 1, 1, 1, 1, 1, 1, 1, // row 1: col 0, col 1
                0, 1, 2, 1, 1, 1, 2, 1, // row 2: col 0, col 1
            ]
        );
        assert_eq!(errors.len(), 0);
        let collected_data = collect_from_organized_data(
            &organized_data,
            input.len(),
            repeater_indices,
            repeater_steps,
        );
        assert_eq!(
            collected_data.as_slice(),
            // (col, colspan, row, rowspan) for each cell in input order
            &[(0, 1, 0, 1), (1, 1, 0, 1), (0, 1, 1, 1), (1, 1, 1, 1), (0, 1, 2, 1), (1, 1, 2, 1),]
        );
        assert_eq!(
            organized_data.max_value(
                input.len(),
                Orientation::Horizontal,
                &repeater_indices,
                &repeater_steps
            ),
            2
        );
        assert_eq!(
            organized_data.max_value(
                input.len(),
                Orientation::Vertical,
                &repeater_indices,
                &repeater_steps
            ),
            3
        );

        // Now test GridLayoutCacheGenerator
        let mut layout_cache_v = SharedVector::<Coord>::default();
        let mut generator = GridLayoutCacheGenerator::new(
            repeater_indices.as_slice(),
            repeater_steps.as_slice(),
            0, // static_cells
            1, // num_repeaters
            6, // total_repeated_cells (3 rows * 2 columns)
            &mut layout_cache_v,
        );
        // Row 0
        generator.add(0., 50.);
        generator.add(0., 50.);
        // Row 1
        generator.add(50., 50.);
        generator.add(50., 50.);
        // Row 2
        generator.add(100., 50.);
        generator.add(100., 50.);
        assert_eq!(
            layout_cache_v.as_slice(),
            &[
                2., 0., // jump cell: data at pos 2
                0., 50., 0., 50., // row 0
                50., 50., 50., 50., // row 1
                100., 50., 100., 50., // row 2
            ]
        );

        // GridRepeaterCacheAccess: cache[cache[jump_index] + ri * stride + child_offset]
        let layout_cache_v_access = |jump_index: usize,
                                     repeater_index: usize,
                                     stride: usize,
                                     child_offset: usize|
         -> Coord {
            let base = layout_cache_v[jump_index] as usize;
            let data_idx = base + repeater_index * stride + child_offset;
            layout_cache_v[data_idx]
        };
        // stride=4 (step=2, entries_per_item=2)
        // Y pos for child 0 (child_offset=0)
        assert_eq!(layout_cache_v_access(0, 0, 4, 0), 0.);
        assert_eq!(layout_cache_v_access(0, 1, 4, 0), 50.);
        assert_eq!(layout_cache_v_access(0, 2, 4, 0), 100.);
        // Y pos for child 1 (child_offset=2)
        assert_eq!(layout_cache_v_access(0, 0, 4, 2), 0.);
        assert_eq!(layout_cache_v_access(0, 1, 4, 2), 50.);
        assert_eq!(layout_cache_v_access(0, 2, 4, 2), 100.);
    }

    #[test]
    fn test_organize_data_repeated_rows_multiple_repeaters() {
        let auto = i_slint_common::ROW_COL_AUTO;
        let mut input = Vec::new();
        let num_rows: u32 = 5;
        let mut cell =
            GridLayoutInputData { new_row: true, col: auto, row: auto, colspan: 1., rowspan: 1. };
        // 3 rows of 2 columns each
        for _ in 0..3 {
            cell.new_row = true;
            input.push(cell.clone());
            cell.new_row = false;
            input.push(cell.clone());
        }
        // 2 rows of 3 columns each
        for _ in 0..2 {
            cell.new_row = true;
            input.push(cell.clone());
            cell.new_row = false;
            input.push(cell.clone());
            cell.new_row = false;
            input.push(cell.clone());
        }
        // Repeater 0: starts at index 0, has 3 instances of 2 elements
        // Repeater 1: starts at index 6 (after repeater 0's 3*2=6 cells), has 2 instances of 3 elements
        let repeater_indices_arr = [0_u32, 3, 6, 2];
        let repeater_steps_arr = [2, 3];
        let repeater_steps = Slice::from_slice(&repeater_steps_arr);
        let repeater_indices = Slice::from_slice(&repeater_indices_arr);
        let (organized_data, errors) =
            organize_grid_layout_impl(Slice::from_slice(&input), repeater_indices, repeater_steps);
        assert_eq!(
            organized_data.as_slice(),
            &[
                8, 0, 0, 0, // repeater 0 jump: data at 8, stride=8 (=step*4=2*4)
                32, 0, 0, 0, // repeater 1 jump: data at 32, stride=12 (=step*4=3*4)
                // Repeater 0 data
                0, 1, 0, 1, 1, 1, 0, 1, // row 0: col 0, col 1
                0, 1, 1, 1, 1, 1, 1, 1, // row 1: col 0, col 1
                0, 1, 2, 1, 1, 1, 2, 1, // row 2: col 0, col 1
                // Repeater 1 data
                0, 1, 3, 1, 1, 1, 3, 1, 2, 1, 3, 1, // row 0: col 0, col 1, col 2
                0, 1, 4, 1, 1, 1, 4, 1, 2, 1, 4, 1, // row 1: col 0, col 1, col 2
            ]
        );
        assert_eq!(errors.len(), 0);
        let collected_data = collect_from_organized_data(
            &organized_data,
            input.len(),
            repeater_indices,
            repeater_steps,
        );
        assert_eq!(
            collected_data.as_slice(),
            // (col, colspan, row, rowspan) for each cell in input order
            &[
                (0, 1, 0, 1),
                (1, 1, 0, 1),
                (0, 1, 1, 1),
                (1, 1, 1, 1),
                (0, 1, 2, 1),
                (1, 1, 2, 1),
                (0, 1, 3, 1),
                (1, 1, 3, 1),
                (2, 1, 3, 1),
                (0, 1, 4, 1),
                (1, 1, 4, 1),
                (2, 1, 4, 1)
            ]
        );
        assert_eq!(
            organized_data.max_value(
                input.len(),
                Orientation::Horizontal,
                &repeater_indices,
                &repeater_steps
            ),
            3 // max col (2) + colspan (1) = 3
        );
        assert_eq!(
            organized_data.max_value(
                input.len(),
                Orientation::Vertical,
                &repeater_indices,
                &repeater_steps
            ),
            num_rows as u16 // max row (4) + rowspan (1) = 5
        );

        // Now test GridLayoutCacheGenerator
        let mut layout_cache_v = SharedVector::<Coord>::default();
        let mut generator = GridLayoutCacheGenerator::new(
            repeater_indices.as_slice(),
            repeater_steps.as_slice(),
            0,  // static_cells
            2,  // num_repeaters
            12, // total_repeated_cells (3*2 + 2*3)
            &mut layout_cache_v,
        );
        // Row 0
        generator.add(0., 50.);
        generator.add(0., 50.);
        // Row 1
        generator.add(50., 50.);
        generator.add(50., 50.);
        // Row 2
        generator.add(100., 50.);
        generator.add(100., 50.);
        // Row 3
        generator.add(150., 50.);
        generator.add(150., 50.);
        generator.add(150., 50.);
        // Row 4
        generator.add(200., 50.);
        generator.add(200., 50.);
        generator.add(200., 50.);
        assert_eq!(
            layout_cache_v.as_slice(),
            &[
                4., 0., // repeater 0 jump: data at pos 4
                16., 0., // repeater 1 jump: data at pos 16
                0., 50., 0., 50., // repeater 0 row 0 data
                50., 50., 50., 50., // repeater 0 row 1 data
                100., 50., 100., 50., // repeater 0 row 2 data
                150., 50., 150., 50., 150., 50., // repeater 1 row 3 data
                200., 50., 200., 50., 200., 50., // repeater 1 row 4 data
            ]
        );

        // GridRepeaterCacheAccess: cache[cache[jump_index] + ri * stride + child_offset]
        let layout_cache_v_access = |jump_index: usize,
                                     repeater_index: usize,
                                     stride: usize,
                                     child_offset: usize|
         -> Coord {
            let base = layout_cache_v[jump_index] as usize;
            let data_idx = base + repeater_index * stride + child_offset;
            layout_cache_v[data_idx]
        };
        // Repeater 0: Y pos for child 0 (child_offset=0), stride=4
        assert_eq!(layout_cache_v_access(0, 0, 4, 0), 0.);
        assert_eq!(layout_cache_v_access(0, 1, 4, 0), 50.);
        assert_eq!(layout_cache_v_access(0, 2, 4, 0), 100.);
        // Repeater 0: Y pos for child 1 (child_offset=2), stride=4
        assert_eq!(layout_cache_v_access(0, 0, 4, 2), 0.);
        assert_eq!(layout_cache_v_access(0, 1, 4, 2), 50.);
        assert_eq!(layout_cache_v_access(0, 2, 4, 2), 100.);
        // Repeater 1: Y pos for child 0 (child_offset=0), jump at index 2, stride=6
        assert_eq!(layout_cache_v_access(2, 0, 6, 0), 150.);
        assert_eq!(layout_cache_v_access(2, 1, 6, 0), 200.);
        // Repeater 1: Y pos for child 2 (child_offset=4), jump at index 2, stride=6
        assert_eq!(layout_cache_v_access(2, 0, 6, 4), 150.);
        assert_eq!(layout_cache_v_access(2, 1, 6, 4), 200.);
    }

    #[test]
    fn test_layout_cache_generator_2_fixed_cells() {
        // 2 fixed cells
        let mut result = SharedVector::<Coord>::default();
        result.resize(2 * 2, 0 as _);
        let mut generator = LayoutCacheGenerator::new(&[], &mut result);
        generator.add(0., 50.); // fixed
        generator.add(80., 50.); // fixed
        assert_eq!(result.as_slice(), &[0., 50., 80., 50.]);
    }

    #[test]
    fn test_layout_cache_generator_1_fixed_cell_1_repeater() {
        // 4 cells: 1 fixed cell, 1 repeater with 3 repeated cells
        let mut result = SharedVector::<Coord>::default();
        let repeater_indices = &[1, 3];
        result.resize(4 * 2 + repeater_indices.len(), 0 as _);
        let mut generator = LayoutCacheGenerator::new(repeater_indices, &mut result);
        generator.add(0., 50.); // fixed
        generator.add(80., 50.); // repeated
        generator.add(160., 50.);
        generator.add(240., 50.);
        assert_eq!(
            result.as_slice(),
            &[
                0., 50., // fixed
                4., 5., // jump to repeater data
                80., 50., 160., 50., 240., 50. // repeater data
            ]
        );
    }

    #[test]
    fn test_layout_cache_generator_4_repeaters() {
        // 8 cells: 1 fixed cell, 1 empty repeater, 1 repeater with 4 repeated cells, 1 fixed cell, 1 repeater with 2 repeated cells, 1 empty repeater
        let mut result = SharedVector::<Coord>::default();
        let repeater_indices = &[1, 0, 1, 4, 6, 2, 8, 0];
        result.resize(8 * 2 + repeater_indices.len(), 0 as _);
        let mut generator = LayoutCacheGenerator::new(repeater_indices, &mut result);
        generator.add(0., 50.); // fixed
        generator.add(80., 10.); // repeated
        generator.add(160., 10.);
        generator.add(240., 10.);
        generator.add(320., 10.); // end of second repeater
        generator.add(400., 80.); // fixed
        generator.add(500., 20.); // repeated
        generator.add(600., 20.); // end of third repeater
        assert_eq!(
            result.as_slice(),
            &[
                0., 50., // fixed
                12., 13., // jump to first (empty) repeater (not used)
                12., 13., // jump to second repeater data
                400., 80., // fixed
                20., 21., // jump to third repeater data
                0., 0., // slot for jumping to fourth repeater (currently empty)
                80., 10., 160., 10., 240., 10., 320., 10., // first repeater data
                500., 20., 600., 20. // second repeater data
            ]
        );
    }
}
