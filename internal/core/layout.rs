// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runtime support for layouts.

// cspell:ignore coord

use crate::items::{DialogButtonRole, LayoutAlignment};
use crate::{Coord, SharedVector, slice::Slice};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

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
                let jump_pos = (ri_start_cell - cell_nr_adj) as usize * 4;
                final_idx = self[jump_pos] as usize + (cell_in_rep * 4) as usize;
                break;
            }
            // -1 is correct for an empty repeater (e.g. if false), which takes one position, for 0 real cells
            // With step > 1, we have 'step' jump entries but cells_in_repeater actual cells
            cell_nr_adj += cells_in_repeater - step;
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

struct OrganizedDataGenerator<'a> {
    // Input
    repeater_indices: &'a [u32],
    repeater_steps: &'a [u32],
    // An always increasing counter, the index of the cell being added
    counter: usize,
    // The index/4 in result in which we should add the next repeated item
    repeat_offset: usize,
    // The index/4 in repeater_indices
    next_rep: usize,
    // The index/4 in result in which we should add the next non-repeated item
    current_offset: usize,
    // Output
    result: &'a mut GridLayoutOrganizedData,
}

impl<'a> OrganizedDataGenerator<'a> {
    fn new(
        repeater_indices: &'a [u32],
        repeater_steps: &'a [u32],
        result: &'a mut GridLayoutOrganizedData,
    ) -> Self {
        // Calculate total repeated cells (count * step for each repeater)
        let total_repeated_cells: usize = repeater_indices
            .chunks(2)
            .enumerate()
            .map(|(i, chunk)| {
                let count = chunk.get(1).copied().unwrap_or(0) as usize;
                let step = repeater_steps.get(i).copied().unwrap_or(1) as usize;
                count * step
            })
            .sum();
        assert!(result.len() >= total_repeated_cells * 4);
        let repeat_offset = result.len() / 4 - total_repeated_cells;
        Self {
            repeater_indices,
            repeater_steps,
            counter: 0,
            repeat_offset,
            next_rep: 0,
            current_offset: 0,
            result,
        }
    }
    fn add(&mut self, col: u16, colspan: u16, row: u16, rowspan: u16) {
        let res = self.result.make_mut_slice();
        let o = loop {
            if let Some(nr) = self.repeater_indices.get(self.next_rep * 2) {
                let nr = *nr as usize;
                let step = self.repeater_steps.get(self.next_rep).copied().unwrap_or(1) as usize;
                if nr == self.counter {
                    // Write jump entries for each element in the step
                    for s in 0..step {
                        for o in 0..4 {
                            res[(self.current_offset + s) * 4 + o] =
                                ((self.repeat_offset + s) * 4 + o) as _;
                        }
                    }
                    self.current_offset += step;
                }
                if self.counter >= nr {
                    let rep_count = self.repeater_indices[self.next_rep * 2 + 1] as usize;
                    let cells_in_repeater = rep_count * step;
                    if self.counter - nr == cells_in_repeater {
                        // Advance repeat_offset past this repeater's data before moving to next
                        self.repeat_offset += cells_in_repeater;
                        self.next_rep += 1;
                        continue;
                    }
                    // Calculate offset using row-major ordering: step entries per row
                    let cell_in_rep = self.counter - nr;
                    let row_in_rep = cell_in_rep / step;
                    let col_in_rep = cell_in_rep % step;
                    let offset = self.repeat_offset + col_in_rep + row_in_rep * step;
                    break offset;
                }
            }
            self.current_offset += 1;
            break self.current_offset - 1;
        };
        res[o * 4] = col;
        res[o * 4 + 1] = colspan;
        res[o * 4 + 2] = row;
        res[o * 4 + 3] = rowspan;
        self.counter += 1;
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
    // Calculate extra space needed for jump entries when step > 1
    // Each repeater with step > 1 needs (step - 1) extra entries for the additional jump slots
    let extra_jump_entries: usize =
        repeater_steps.iter().map(|&s| (s as usize).saturating_sub(1)).sum();
    organized_data
        .resize(input_data.len() * 4 + repeater_indices.len() * 2 + extra_jump_entries * 4, 0 as _);
    let mut generator = OrganizedDataGenerator::new(
        repeater_indices.as_slice(),
        repeater_steps.as_slice(),
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

/// The layout cache generator inserts the pos and size into the result array (which becomes the layout cache property),
/// including the indirections for repeated items (so that the x,y,width,height properties for repeated items
/// can point to indices known at compile time, those that contain the indirections)
/// Example: for repeater_indices=[1,4] (meaning that item at index 1 is repeated 4 times),
/// result=[0.0, 80.0, 4.0, 5.0, 80.0, 80.0, 160.0, 80.0, 240.0, 80.0, 320.0, 80.0]
///  i.e. pos1, width1, jump to idx 4, jump to idx 5, pos2, width2, pos3, width3, pos4, width4, pos5, width5
struct LayoutCacheGenerator<'a> {
    // Input
    repeater_indices: &'a [u32],
    repeater_steps: &'a [u32],
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
    fn new(
        repeater_indices: &'a [u32],
        repeater_steps: &'a [u32],
        result: &'a mut SharedVector<Coord>,
    ) -> Self {
        // Calculate total repeated cells (count * step for each repeater)
        let total_repeated_cells: usize = repeater_indices
            .chunks(2)
            .enumerate()
            .map(|(i, chunk)| {
                let count = chunk.get(1).copied().unwrap_or(0) as usize;
                let step = repeater_steps.get(i).copied().unwrap_or(1) as usize;
                count * step
            })
            .sum();
        assert!(result.len() >= total_repeated_cells * 2);
        let repeat_offset = result.len() / 2 - total_repeated_cells;
        Self {
            repeater_indices,
            repeater_steps,
            counter: 0,
            repeat_offset,
            next_rep: 0,
            current_offset: 0,
            result,
        }
    }
    fn add(&mut self, pos: Coord, size: Coord) {
        let res = self.result.make_mut_slice();
        let o = loop {
            if let Some(nr) = self.repeater_indices.get(self.next_rep * 2) {
                let nr = *nr as usize;
                let step = self.repeater_steps.get(self.next_rep).copied().unwrap_or(1) as usize;
                if nr == self.counter {
                    // Write jump entries for each element in the step
                    for s in 0..step {
                        for o in 0..2 {
                            res[(self.current_offset + s) * 2 + o] =
                                ((self.repeat_offset + s) * 2 + o) as _;
                        }
                    }
                    self.current_offset += step;
                }
                if self.counter >= nr {
                    let rep_count = self.repeater_indices[self.next_rep * 2 + 1] as usize;
                    let cells_in_repeater = rep_count * step;
                    if self.counter - nr == cells_in_repeater {
                        // Advance repeat_offset past this repeater's data before moving to next
                        self.repeat_offset += cells_in_repeater;
                        self.next_rep += 1;
                        continue;
                    }
                    // Calculate offset using row-major ordering: step entries per row
                    let cell_in_rep = self.counter - nr;
                    let row_in_rep = cell_in_rep / step;
                    let col_in_rep = cell_in_rep % step;
                    let offset = self.repeat_offset + col_in_rep + row_in_rep * step;
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
    // Calculate extra space for jump entries when step > 1
    // Each repeater with step > 1 needs (step - 1) extra entries for additional jump slots
    let extra_jump_entries: usize =
        repeater_steps.iter().map(|&s| (s as usize).saturating_sub(1)).sum();
    result.resize(2 * constraints.len() + repeater_indices.len() + extra_jump_entries * 2, 0 as _);
    let mut generator = LayoutCacheGenerator::new(&repeater_indices, &repeater_steps, &mut result);

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

    let mut generator = LayoutCacheGenerator::new(&repeater_indices, &[], &mut result);
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
        result.resize(num_cells * 4, 0 as _);
        let mut generator = OrganizedDataGenerator::new(&[], &[], &mut result);
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
        result.resize(num_cells * 4 + 2 * repeater_indices.len(), 0 as _);
        let mut generator = OrganizedDataGenerator::new(repeater_indices, &[], &mut result);
        generator.add(0, 1, 0, 2); // fixed
        generator.add(1, 2, 1, 3); // repeated
        generator.add(1, 1, 2, 4);
        generator.add(2, 2, 3, 5);
        assert_eq!(
            result.as_slice(),
            &[
                0, 1, 0, 2, // fixed
                8, 9, 10, 11, // jump to repeater data
                1, 2, 1, 3, 1, 1, 2, 4, 2, 2, 3, 5 // repeater data
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
                28, 29, 30, 31, // jump to first (empty) repeater (not used)
                0, 1, 0, 1, // first row, first column
                28, 29, 30, 31, // jump to first repeater data
                5, 1, 0, 1, // fixed
                44, 45, 46, 47, // jump to second repeater data
                52, 53, 54, 55, // slot for jumping to 3rd repeater (out of bounds, not used)
                8, 1, 0, 1, // final fixed element
                1, 1, 0, 1, // first repeater data
                2, 1, 0, 1, 3, 1, 0, 1, 4, 1, 0, 1, // end of first repeater
                6, 1, 0, 1, // second repeater data
                7, 1, 0, 1 // end of second repeater
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
                8, 9, 10, 11, // jump to repeater data for column 0 (offset 2)
                12, 13, 14, 15, // jump to repeater data for column 1 (offset 3)
                0, 1, 0, 1, // row 0, col 0 (offset 2)
                1, 1, 0, 1, // row 0, col 1 (offset 3)
                0, 1, 1, 1, // row 1, col 0 (offset 4)
                1, 1, 1, 1, // row 1, col 1 (offset 5)
                0, 1, 2, 1, // row 2, col 0 (offset 6)
                1, 1, 2, 1, // row 2, col 1 (offset 7)
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

        // Now test LayoutCacheGenerator
        let mut layout_cache_v = SharedVector::<Coord>::default();
        // 2 jump entries (one per column) + 6 data entries (3 rows × 2 columns)
        layout_cache_v.resize((num_columns * 2 + num_columns * num_rows * 2) as usize, 0 as _);
        let mut generator = LayoutCacheGenerator::new(
            repeater_indices.as_slice(),
            repeater_steps.as_slice(),
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
                4., 5., 6., 7., // jump to repeater data
                0., 50., 0., 50., // row 0
                50., 50., 50., 50., // row 1
                100., 50., 100., 50., // row 2
            ]
        );

        let layout_cache_v_access = |index: usize, repeater_index: usize, step: usize| -> Coord {
            let entries_per_item = 2usize;
            let offset = repeater_index;
            // same as the code generated for LayoutCacheAccess in rust.rs
            *layout_cache_v
                .get((layout_cache_v[index] as usize) + offset as usize * entries_per_item * step)
                .unwrap()
        };
        // Y values for A
        assert_eq!(layout_cache_v_access(0, 0, 2), 0.);
        assert_eq!(layout_cache_v_access(0, 1, 2), 50.);
        assert_eq!(layout_cache_v_access(0, 2, 2), 100.);
        // Y values for B
        assert_eq!(layout_cache_v_access(1 * 2, 0, 2), 0.);
        assert_eq!(layout_cache_v_access(1 * 2, 1, 2), 50.);
        assert_eq!(layout_cache_v_access(1 * 2, 2, 2), 100.);
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
                20, 21, 22, 23, // repeater 0: jump to repeater data for column 0 (offset 5)
                24, 25, 26, 27, // repeater 0: jump to repeater data for column 1 (offset 6)
                44, 45, 46, 47, // repeater 1: jump to repeater data for column 0 (offset 11)
                48, 49, 50, 51, // repeater 1: jump to repeater data for column 1 (offset 12)
                52, 53, 54, 55, // repeater 1: jump to repeater data for column 2 (offset 13)
                // Repeater 0
                0, 1, 0, 1, // row 0, col 0 (offset 5)
                1, 1, 0, 1, // row 0, col 1 (offset 6)
                0, 1, 1, 1, // row 1, col 0 (offset 7)
                1, 1, 1, 1, // row 1, col 1 (offset 8)
                0, 1, 2, 1, // row 2, col 0 (offset 9)
                1, 1, 2, 1, // row 2, col 1 (offset 10)
                // Repeater 1
                0, 1, 3, 1, // row 3, col 0 (offset 11)
                1, 1, 3, 1, // row 3, col 1 (offset 12)
                2, 1, 3, 1, // row 3, col 2 (offset 13)
                0, 1, 4, 1, // row 4, col 0 (offset 14)
                1, 1, 4, 1, // row 4, col 1 (offset 15)
                2, 1, 4, 1, // row 4, col 2 (offset 16)
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

        // Now test LayoutCacheGenerator
        let mut layout_cache_v = SharedVector::<Coord>::default();
        // 5 jump entries, just like above + 12 data entries (3*2+2*3) - where each entry has 2 values (pos, size)
        layout_cache_v.resize((5 * 2 + (3 * 2 + 2 * 3) * 2) as usize, 0 as _);
        let mut generator = LayoutCacheGenerator::new(
            repeater_indices.as_slice(),
            repeater_steps.as_slice(),
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
                10., 11., 12., 13., // repeater 0: jump to repeater data
                22., 23., 24., 25., 26., 27., // repeater 1: jump to repeater data
                0., 50., 0., 50., // row 0
                50., 50., 50., 50., // row 1
                100., 50., 100., 50., // row 2
                150., 50., 150., 50., 150., 50., // row 3
                200., 50., 200., 50., 200., 50., // row 4
            ]
        );

        let layout_cache_v_access = |index: usize, repeater_index: usize, step: usize| -> Coord {
            let entries_per_item = 2usize;
            let offset = repeater_index;
            // same as the code generated for LayoutCacheAccess in rust.rs
            *layout_cache_v
                .get((layout_cache_v[index] as usize) + offset as usize * entries_per_item * step)
                .unwrap()
        };
        // Y values for A
        assert_eq!(layout_cache_v_access(0, 0, 2), 0.);
        assert_eq!(layout_cache_v_access(0, 1, 2), 50.);
        assert_eq!(layout_cache_v_access(0, 2, 2), 100.);
        // Y values for B
        assert_eq!(layout_cache_v_access(1 * 2, 0, 2), 0.);
        assert_eq!(layout_cache_v_access(1 * 2, 1, 2), 50.);
        assert_eq!(layout_cache_v_access(1 * 2, 2, 2), 100.);
    }

    #[test]
    fn test_layout_cache_generator_2_fixed_cells() {
        // 2 fixed cells
        let mut result = SharedVector::<Coord>::default();
        result.resize(2 * 2, 0 as _);
        let mut generator = LayoutCacheGenerator::new(&[], &[], &mut result);
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
        let mut generator = LayoutCacheGenerator::new(repeater_indices, &[], &mut result);
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
        let mut generator = LayoutCacheGenerator::new(repeater_indices, &[], &mut result);
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
