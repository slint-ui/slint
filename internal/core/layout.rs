// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Runtime support for layouts.

// cspell:ignore coord

use crate::items::{DialogButtonRole, LayoutAlignment};
use crate::{slice::Slice, Coord, SharedVector};
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

    /// Create a vector of LayoutData for an array of GridLayoutCellData
    pub fn to_layout_data(
        data: &[GridLayoutCellData],
        spacing: Coord,
        size: Option<Coord>,
    ) -> Vec<LayoutData> {
        let mut num = 0;
        for cell in data {
            num = num.max(cell.col_or_row + cell.span);
        }
        if num < 1 {
            return Default::default();
        }
        let mut layout_data = alloc::vec![grid_internal::LayoutData { stretch: 1., ..Default::default() }; num as usize];
        let mut has_spans = false;
        for cell in data {
            let constraint = &cell.constraint;
            let mut max = constraint.max;
            if let Some(size) = size {
                max = max.min(size * constraint.max_percent / 100 as Coord);
            }
            for c in 0..(cell.span as usize) {
                let cdata = &mut layout_data[cell.col_or_row as usize + c];
                cdata.max = cdata.max.min(max);
            }
            if cell.span == 1 {
                let mut min = constraint.min;
                if let Some(size) = size {
                    min = min.max(size * constraint.min_percent / 100 as Coord);
                }
                let pref = constraint.preferred.min(max).max(min);
                let cdata = &mut layout_data[cell.col_or_row as usize];
                cdata.min = cdata.min.max(min);
                cdata.pref = cdata.pref.max(pref);
                cdata.stretch = cdata.stretch.min(constraint.stretch);
            } else {
                has_spans = true;
            }
        }
        if has_spans {
            // Adjust minimum sizes
            for cell in data.iter().filter(|cell| cell.span > 1) {
                let span_data = &mut layout_data
                    [(cell.col_or_row as usize)..(cell.col_or_row + cell.span) as usize];
                let mut min = cell.constraint.min;
                if let Some(size) = size {
                    min = min.max(size * cell.constraint.min_percent / 100 as Coord);
                }
                grid_internal::layout_items(span_data, 0 as _, min, spacing);
                for cdata in span_data {
                    if cdata.min < cdata.size {
                        cdata.min = cdata.size;
                    }
                }
            }
            // Adjust maximum sizes
            for cell in data.iter().filter(|cell| cell.span > 1) {
                let span_data = &mut layout_data
                    [(cell.col_or_row as usize)..(cell.col_or_row + cell.span) as usize];
                let mut max = cell.constraint.max;
                if let Some(size) = size {
                    max = max.min(size * cell.constraint.max_percent / 100 as Coord);
                }
                grid_internal::layout_items(span_data, 0 as _, max, spacing);
                for cdata in span_data {
                    if cdata.max > cdata.size {
                        cdata.max = cdata.size;
                    }
                }
            }
            // Adjust preferred sizes
            for cell in data.iter().filter(|cell| cell.span > 1) {
                let span_data = &mut layout_data
                    [(cell.col_or_row as usize)..(cell.col_or_row + cell.span) as usize];
                grid_internal::layout_items(span_data, 0 as _, cell.constraint.preferred, spacing);
                for cdata in span_data {
                    cdata.pref = cdata.pref.max(cdata.size).min(cdata.max).max(cdata.min);
                }
            }
            // Adjust stretches
            for cell in data.iter().filter(|cell| cell.span > 1) {
                let span_data = &mut layout_data
                    [(cell.col_or_row as usize)..(cell.col_or_row + cell.span) as usize];
                let total_stretch: f32 = span_data.iter().map(|c| c.stretch).sum();
                if total_stretch > cell.constraint.stretch {
                    for cdata in span_data {
                        cdata.stretch *= cell.constraint.stretch / total_stretch;
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
pub struct GridLayoutData<'a> {
    pub size: Coord,
    pub spacing: Coord,
    pub padding: Padding,
    pub cells: Slice<'a, GridLayoutCellData>,
}

#[repr(C)]
#[derive(Default, Debug)]
pub struct GridLayoutCellData {
    /// col, or row.
    pub col_or_row: u16,
    /// colspan or rowspan
    pub span: u16,
    pub constraint: LayoutInfo,
}

/// return, an array which is of size `data.cells.len() * 2` which for each cell we give the pos, size
pub fn solve_grid_layout(data: &GridLayoutData) -> SharedVector<Coord> {
    let mut layout_data =
        grid_internal::to_layout_data(data.cells.as_slice(), data.spacing, Some(data.size));

    if layout_data.is_empty() {
        return Default::default();
    }

    grid_internal::layout_items(
        &mut layout_data,
        data.padding.begin,
        data.size - (data.padding.begin + data.padding.end),
        data.spacing,
    );

    let mut result = SharedVector::with_capacity(4 * data.cells.len());
    for cell in data.cells.iter() {
        let cdata = &layout_data[cell.col_or_row as usize];
        result.push(cdata.pos);
        result.push({
            let first_cell = &layout_data[cell.col_or_row as usize];
            let last_cell = &layout_data[cell.col_or_row as usize + cell.span as usize - 1];
            last_cell.pos + last_cell.size - first_cell.pos
        });
    }
    result
}

pub fn grid_layout_info(
    cells: Slice<GridLayoutCellData>,
    spacing: Coord,
    padding: &Padding,
) -> LayoutInfo {
    let layout_data = grid_internal::to_layout_data(cells.as_slice(), spacing, None);
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
pub fn solve_box_layout(data: &BoxLayoutData, repeater_indexes: Slice<u32>) -> SharedVector<Coord> {
    let mut result = SharedVector::<Coord>::default();
    result.resize(data.cells.len() * 2 + repeater_indexes.len(), 0 as _);

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
    };
    if let Some((mut pos, spacing)) = align {
        for it in &mut layout_data {
            it.pos = pos;
            it.size = it.pref;
            pos += spacing + it.size;
        }
    }

    let res = result.make_mut_slice();

    // The index/2 in result in which we should add the next repeated item
    let mut repeat_offset =
        res.len() / 2 - repeater_indexes.iter().skip(1).step_by(2).sum::<u32>() as usize;
    // The index/2  in repeater_indexes
    let mut next_rep = 0;
    // The index/2 in result in which we should add the next non-repeated item
    let mut current_offset = 0;
    for (idx, layout) in layout_data.iter().enumerate() {
        let o = loop {
            if let Some(nr) = repeater_indexes.get(next_rep * 2) {
                let nr = *nr as usize;
                if nr == idx {
                    for o in 0..2 {
                        res[current_offset * 2 + o] = (repeat_offset * 2 + o) as _;
                    }
                    current_offset += 1;
                }
                if idx >= nr {
                    if idx - nr == repeater_indexes[next_rep * 2 + 1] as usize {
                        next_rep += 1;
                        continue;
                    }
                    repeat_offset += 1;
                    break repeat_offset - 1;
                }
            }
            current_offset += 1;
            break current_offset - 1;
        };
        res[o * 2] = layout.pos;
        res[o * 2 + 1] = layout.size;
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
    if count < 1 {
        return LayoutInfo { max: 0 as _, ..LayoutInfo::default() };
    };
    let is_stretch = alignment == LayoutAlignment::Stretch;
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
    let count = cells.len();
    if count < 1 {
        return LayoutInfo { max: 0 as _, ..LayoutInfo::default() };
    };
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

/// Given the cells of a layout of a Dialog, re-order the button according to the platform
///
/// This function assume that the `roles` contains the roles of the button which are the first `cells`
/// It will simply change the column field of the cell
pub fn reorder_dialog_button_layout(cells: &mut [GridLayoutCellData], roles: &[DialogButtonRole]) {
    fn add_buttons(
        cells: &mut [GridLayoutCellData],
        roles: &[DialogButtonRole],
        idx: &mut u16,
        role: DialogButtonRole,
    ) {
        for (cell, r) in cells.iter_mut().zip(roles.iter()) {
            if *r == role {
                cell.col_or_row = *idx;
                *idx += 1;
            }
        }
    }

    #[cfg(feature = "std")]
    fn is_kde() -> bool {
        // assume some unix check if XDG_CURRENT_DESKTOP stats with K
        std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .and_then(|v| v.as_bytes().first().copied())
            .map_or(false, |x| x.to_ascii_uppercase() == b'K')
    }
    #[cfg(not(feature = "std"))]
    let is_kde = || true;

    let mut idx = 0;

    if cfg!(windows) {
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reset);
        idx += 1;
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Accept);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Action);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reject);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Apply);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Help);
    } else if cfg!(target_os = "macos") {
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Help);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reset);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Apply);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Action);
        idx += 1;
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reject);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Accept);
    } else if is_kde() {
        // KDE variant
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Help);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reset);
        idx += 1;
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Action);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Accept);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Apply);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reject);
    } else {
        // GNOME variant and fallback for WASM build
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Help);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reset);
        idx += 1;
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Action);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Apply);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Reject);
        add_buttons(cells, roles, &mut idx, DialogButtonRole::Accept);
    }
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[no_mangle]
    pub extern "C" fn slint_solve_grid_layout(
        data: &GridLayoutData,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_grid_layout(data)
    }

    #[no_mangle]
    pub extern "C" fn slint_grid_layout_info(
        cells: Slice<GridLayoutCellData>,
        spacing: Coord,
        padding: &Padding,
    ) -> LayoutInfo {
        super::grid_layout_info(cells, spacing, padding)
    }

    #[no_mangle]
    pub extern "C" fn slint_solve_box_layout(
        data: &BoxLayoutData,
        repeater_indexes: Slice<u32>,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_box_layout(data, repeater_indexes)
    }

    #[no_mangle]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn slint_box_layout_info(
        cells: Slice<BoxLayoutCellData>,
        spacing: Coord,
        padding: &Padding,
        alignment: LayoutAlignment,
    ) -> LayoutInfo {
        super::box_layout_info(cells, spacing, padding, alignment)
    }

    #[no_mangle]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn slint_box_layout_info_ortho(
        cells: Slice<BoxLayoutCellData>,
        padding: &Padding,
    ) -> LayoutInfo {
        super::box_layout_info_ortho(cells, padding)
    }

    /// Calls [`reorder_dialog_button_layout`].
    ///
    /// Safety: `cells` must be a pointer to a mutable array of cell data, the array must have at
    /// least `roles.len()` elements.
    #[no_mangle]
    pub unsafe extern "C" fn slint_reorder_dialog_button_layout(
        cells: *mut GridLayoutCellData,
        roles: Slice<DialogButtonRole>,
    ) {
        reorder_dialog_button_layout(core::slice::from_raw_parts_mut(cells, roles.len()), &roles);
    }
}
