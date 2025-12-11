// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Runtime support for layouts.

// cspell:ignore coord

use crate::items::{DialogButtonRole, LayoutAlignment};
use crate::{Coord, SharedVector, slice::Slice};
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
        constraints: Slice<LayoutInfo>,
        orientation: Orientation,
        spacing: Coord,
        size: Option<Coord>,
    ) -> Vec<LayoutData> {
        assert!(organized_data.len() % 4 == 0);
        assert!(constraints.len() * 4 == organized_data.len());
        let num = organized_data.max_value(orientation) as usize;
        if num < 1 {
            return Default::default();
        }
        let mut layout_data =
            alloc::vec![grid_internal::LayoutData { stretch: 1., ..Default::default() }; num];
        let mut has_spans = false;
        for (idx, constraint) in constraints.iter().enumerate() {
            let mut max = constraint.max;
            if let Some(size) = size {
                max = max.min(size * constraint.max_percent / 100 as Coord);
            }
            let (col_or_row, span) = organized_data.col_or_row_and_span(idx, orientation);
            for c in 0..(span as usize) {
                let cdata = &mut layout_data[col_or_row as usize + c];
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
            for (idx, constraint) in constraints.iter().enumerate() {
                let (col_or_row, span) = organized_data.col_or_row_and_span(idx, orientation);
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
    /// col and row number (u16::MAX means auto).
    pub col: u16,
    pub row: u16,
    /// colspan and rowspan
    pub colspan: u16,
    pub rowspan: u16,
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

    fn col_or_row_and_span(&self, index: usize, orientation: Orientation) -> (u16, u16) {
        let offset = if orientation == Orientation::Horizontal { 0 } else { 2 };
        (self[index * 4 + offset], self[index * 4 + offset + 1])
    }

    fn max_value(&self, orientation: Orientation) -> u16 {
        let mut max = 0;
        for idx in 0..self.len() / 4 {
            let (col_or_row, span) = self.col_or_row_and_span(idx, orientation);
            max = max.max(col_or_row + span.max(1));
        }
        max
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
            organized_data.push_cell(col as u16, cell.colspan, cell.row, cell.rowspan);
        } else {
            // This is used for the main window (which is the only cell which isn't a button)
            // Given lower_dialog_layout(), this will always be a single cell at 0,0 with a colspan of number_of_buttons
            organized_data.push_cell(cell.col, cell.colspan, cell.row, cell.rowspan);
        }
    }
    organized_data
}

// Implement "auto" behavior for row/col numbers (unless specified in the slint file).
pub fn organize_grid_layout(input_data: Slice<GridLayoutInputData>) -> GridLayoutOrganizedData {
    let mut organized_data = GridLayoutOrganizedData::default();
    organized_data.reserve(input_data.len() * 4);
    let mut row = 0;
    let mut col = 0;
    let mut first = true;
    let auto = u16::MAX;
    for cell in input_data.as_slice().iter() {
        if cell.new_row && !first {
            row += 1;
            col = 0;
        }
        first = false;
        if cell.row != auto && row != cell.row {
            row = cell.row;
            col = 0;
        }
        if cell.col != auto {
            col = cell.col;
        }

        organized_data.push_cell(col, cell.colspan, row, cell.rowspan);
        col += 1;
    }
    organized_data
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
        let repeat_offset =
            result.len() / 2 - repeater_indices.iter().skip(1).step_by(2).sum::<u32>() as usize;
        Self { repeater_indices, counter: 0, repeat_offset, next_rep: 0, current_offset: 0, result }
    }
    fn add(&mut self, pos: Coord, size: Coord) {
        let res = self.result.make_mut_slice();
        let o = loop {
            if let Some(nr) = self.repeater_indices.get(self.next_rep * 2) {
                let nr = *nr as usize;
                if nr == self.counter {
                    for o in 0..2 {
                        res[self.current_offset * 2 + o] = (self.repeat_offset * 2 + o) as _;
                    }
                    self.current_offset += 1;
                }
                if self.counter >= nr {
                    if self.counter - nr == self.repeater_indices[self.next_rep * 2 + 1] as usize {
                        self.next_rep += 1;
                        continue;
                    }
                    self.repeat_offset += 1;
                    break self.repeat_offset - 1;
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
    constraints: Slice<LayoutInfo>,
    orientation: Orientation,
) -> SharedVector<Coord> {
    let mut layout_data = grid_internal::to_layout_data(
        &data.organized_data,
        constraints,
        orientation,
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

    let mut result = SharedVector::with_capacity(2 * constraints.len());
    for idx in 0..constraints.len() {
        let (col_or_row, span) = data.organized_data.col_or_row_and_span(idx, orientation);
        let cdata = &layout_data[col_or_row as usize];
        result.push(cdata.pos);
        result.push(if span > 0 {
            let first_cell = &layout_data[col_or_row as usize];
            let last_cell = &layout_data[col_or_row as usize + span as usize - 1];
            last_cell.pos + last_cell.size - first_cell.pos
        } else {
            0 as Coord
        });
    }
    result
}

pub fn grid_layout_info(
    organized_data: GridLayoutOrganizedData, // not & because the code generator doesn't support it in ExtraBuiltinFunctionCall
    constraints: Slice<LayoutInfo>,
    spacing: Coord,
    padding: &Padding,
    orientation: Orientation,
) -> LayoutInfo {
    let layout_data =
        grid_internal::to_layout_data(&organized_data, constraints, orientation, spacing, None);
    if layout_data.is_empty() {
        return Default::default();
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
    pub cells: Slice<'a, BoxLayoutCellData>,
}

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct BoxLayoutCellData {
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
    cells: Slice<BoxLayoutCellData>,
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

pub fn box_layout_info_ortho(cells: Slice<BoxLayoutCellData>, padding: &Padding) -> LayoutInfo {
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
        result: &mut GridLayoutOrganizedData,
    ) {
        *result = super::organize_grid_layout(input_data);
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
        constraints: Slice<LayoutInfo>,
        orientation: Orientation,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_grid_layout(data, constraints, orientation)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_grid_layout_info(
        organized_data: &GridLayoutOrganizedData,
        constraints: Slice<LayoutInfo>,
        spacing: Coord,
        padding: &Padding,
        orientation: Orientation,
    ) -> LayoutInfo {
        super::grid_layout_info(organized_data.clone(), constraints, spacing, padding, orientation)
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
        cells: Slice<BoxLayoutCellData>,
        spacing: Coord,
        padding: &Padding,
        alignment: LayoutAlignment,
    ) -> LayoutInfo {
        super::box_layout_info(cells, spacing, padding, alignment)
    }

    #[unsafe(no_mangle)]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn slint_box_layout_info_ortho(
        cells: Slice<BoxLayoutCellData>,
        padding: &Padding,
    ) -> LayoutInfo {
        super::box_layout_info_ortho(cells, padding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
