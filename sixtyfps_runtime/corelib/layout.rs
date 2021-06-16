/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Runtime support for layouting.
//!
//! Currently this is a very basic implementation

use crate::{slice::Slice, SharedVector};

/// Vertical or Orizontal orientation
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(u8)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

type Coord = f32;

/// The constraint that applies to an item
// NOTE: when adding new fields, the C++ operator== also need updates
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutInfo {
    /// The minimum size for this item.
    pub min: f32,
    /// The maximum size for the item.
    pub max: f32,

    /// The minimum size in percentage of the parent (value between 0 and 100).
    pub min_percent: f32,
    /// The maximum size in percentage of the parent (value between 0 and 100).
    pub max_percent: f32,

    /// the preferred size
    pub preferred: f32,

    /// the  stretch factor
    pub stretch: f32,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        LayoutInfo {
            min: 0.,
            max: f32::MAX,
            min_percent: 0.,
            max_percent: 100.,
            preferred: 0.,
            stretch: 0.,
        }
    }
}

impl LayoutInfo {
    // Note: This "logic" is duplicated in the cpp generator's generated code for merging layout infos.
    pub fn merge(&self, other: &LayoutInfo) -> Self {
        let merge_preferred_size = |left_stretch, left_size, right_stretch, right_size| {
            if left_stretch < right_stretch {
                left_size
            } else if left_stretch > right_stretch {
                right_size
            } else {
                (left_size + right_size) / 2.
            }
        };

        Self {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
            min_percent: self.min_percent.max(other.min_percent),
            max_percent: self.max_percent.min(other.max_percent),
            preferred: merge_preferred_size(
                self.stretch,
                self.preferred,
                other.stretch,
                other.preferred,
            ),
            stretch: self.stretch.min(other.stretch),
        }
    }

    /// Helper function to return a preferred size which is within the min/max constraints
    pub fn preferred_bounded(&self) -> f32 {
        self.preferred.min(self.max).max(self.min)
    }
}

impl core::ops::Add for LayoutInfo {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.merge(&rhs)
    }
}

mod grid_internal {
    use super::*;

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
            LayoutData { min: 0., max: Coord::MAX, pref: 0., stretch: f32::MAX, pos: 0., size: 0. }
        }
    }

    pub fn layout_items(data: &mut [LayoutData], start_pos: Coord, size: Coord, spacing: Coord) {
        use stretch::geometry::*;
        use stretch::number::*;
        use stretch::style::*;

        let mut stretch = stretch::Stretch::new();

        let box_style = stretch::style::Style {
            size: Size { width: Dimension::Percent(1.), height: Dimension::Percent(1.) },
            flex_grow: 1.,
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_basis: Dimension::Percent(1.),
            ..Default::default()
        };

        let flex_box = stretch.new_node(box_style, vec![]).unwrap();

        data.iter().enumerate().for_each(|(index, cell)| {
            let min =
                if cell.min == 0.0 { Dimension::Undefined } else { Dimension::Points(cell.min) };
            let max = if cell.max == f32::MAX {
                Dimension::Undefined
            } else {
                Dimension::Points(cell.max)
            };
            let pref =
                if cell.pref == 0.0 { Dimension::Undefined } else { Dimension::Points(cell.pref) };

            let mut margin = Rect::default();

            if index != 0 {
                margin.start = Dimension::Points(spacing / 2.);
            }
            if index != data.len() - 1 {
                margin.end = Dimension::Points(spacing / 2.);
            }

            let cell_style = Style {
                min_size: Size { width: min, height: Dimension::Auto },
                max_size: Size { width: max, height: Dimension::Auto },
                size: Size { width: pref, height: Dimension::Auto },
                flex_grow: cell.stretch,
                flex_shrink: cell.stretch,
                margin,
                ..Default::default()
            };

            let cell_item = stretch.new_node(cell_style, vec![]).unwrap();
            stretch.add_child(flex_box, cell_item).unwrap();
        });

        stretch
            .compute_layout(
                flex_box,
                Size { width: Number::Defined(size), height: Number::Undefined },
            )
            .unwrap();

        data.iter_mut()
            .zip(
                stretch
                    .children(flex_box)
                    .unwrap()
                    .iter()
                    .map(|child| stretch.layout(*child).unwrap()),
            )
            .for_each(|(cell, layout)| {
                cell.pos = start_pos + layout.location.x;
                cell.size = layout.size.width;
            });
    }

    #[test]
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
}

#[repr(C)]
pub struct Constraint {
    pub min: Coord,
    pub max: Coord,
}

impl Default for Constraint {
    fn default() -> Self {
        Constraint { min: 0., max: Coord::MAX }
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Padding {
    pub begin: Coord,
    pub end: Coord,
}

#[repr(C)]
#[derive(Debug)]
pub struct GridLayoutData<'a> {
    pub size: Coord,
    pub spacing: Coord,
    pub padding: &'a Padding,
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

/// return, an array which is of siz `data.cells.len() * 4` which for each cell we give the x, y, width, height
pub fn solve_grid_layout(data: &GridLayoutData) -> SharedVector<Coord> {
    let mut num = 0;
    for cell in data.cells.iter() {
        num = num.max(cell.col_or_row + cell.span);
    }

    if num < 1 {
        return Default::default();
    }

    let mut layout_data = vec![grid_internal::LayoutData::default(); num as usize];

    for cell in data.cells.iter() {
        let cnstr = &cell.constraint;
        let max = cnstr.max.min(data.size * cnstr.max_percent / 100.);
        let min = cnstr.min.max(data.size * cnstr.min_percent / 100.) / (cell.span as f32);
        let pref = cnstr.preferred;

        for c in 0..(cell.span as usize) {
            let cdata = &mut layout_data[cell.col_or_row as usize + c];
            cdata.max = cdata.max.min(max);
            cdata.min = cdata.min.max(min);
            cdata.pref = cdata.pref.max(pref);
            cdata.stretch = cdata.stretch.min(cnstr.stretch);
        }
    }

    // Normalize so that all the values are 1 or more
    let normalize_stretch = |v: &mut Vec<grid_internal::LayoutData>| {
        let mut small: Option<f32> = None;
        v.iter().for_each(|x| {
            if x.stretch > 0. {
                small = Some(small.map(|y| y.min(x.stretch)).unwrap_or(x.stretch))
            }
        });
        if small.unwrap_or(0.) < 1. {
            v.iter_mut()
                .for_each(|x| x.stretch = if let Some(s) = small { x.stretch / s } else { 1. })
        }
    };
    normalize_stretch(&mut layout_data);

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

pub fn grid_layout_info<'a>(
    cells: Slice<'a, GridLayoutCellData>,
    spacing: Coord,
    padding: &Padding,
) -> LayoutInfo {
    let mut num = 0;
    for cell in cells.iter() {
        num = num.max(cell.col_or_row + cell.span);
    }

    if num < 1 {
        return LayoutInfo { max: 0., ..LayoutInfo::default() };
    }

    let mut layout_data = vec![grid_internal::LayoutData::default(); num as usize];
    for cell in cells.iter() {
        let cdata = &mut layout_data[cell.col_or_row as usize];
        cdata.max = cdata.max.min(cell.constraint.max);
        cdata.min = cdata.min.max(cell.constraint.min);
        cdata.pref = cdata.pref.max(cell.constraint.preferred);
        cdata.stretch = cdata.stretch.min(cell.constraint.stretch);
    }

    let spacing_w = spacing * (num - 1) as Coord + padding.begin + padding.end;
    let min = layout_data.iter().map(|data| data.min).sum::<Coord>() + spacing_w;
    let max = layout_data.iter().map(|data| data.max).sum::<Coord>() + spacing_w;
    let stretch = layout_data.iter().map(|data| data.stretch).sum::<Coord>();
    LayoutInfo { min, max, min_percent: 0., max_percent: 100., preferred: min, stretch }
}

/// Enum representing the alignment property of a BoxLayout or HorizontalLayout
#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum LayoutAlignment {
    stretch,
    center,
    start,
    end,
    space_between,
    space_around,
}

impl Default for LayoutAlignment {
    fn default() -> Self {
        Self::stretch
    }
}

impl From<LayoutAlignment> for stretch::style::JustifyContent {
    fn from(a: LayoutAlignment) -> Self {
        match a {
            LayoutAlignment::stretch => Self::FlexStart,
            LayoutAlignment::center => Self::Center,
            LayoutAlignment::start => Self::FlexStart,
            LayoutAlignment::end => Self::FlexEnd,
            LayoutAlignment::space_between => Self::SpaceBetween,
            LayoutAlignment::space_around => Self::SpaceAround,
        }
    }
}

impl From<LayoutAlignment> for stretch::style::AlignContent {
    fn from(a: LayoutAlignment) -> Self {
        match a {
            LayoutAlignment::stretch => Self::Stretch,
            LayoutAlignment::center => Self::Center,
            LayoutAlignment::start => Self::FlexStart,
            LayoutAlignment::end => Self::FlexEnd,
            LayoutAlignment::space_between => Self::SpaceBetween,
            LayoutAlignment::space_around => Self::SpaceAround,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
/// The BoxLayoutData is used to represent both a Horizontal and Vertical layout.
/// The width/height x/y corrspond to that of a horizontal layout.
/// For vertical layout, they are inverted
pub struct BoxLayoutData<'a> {
    pub size: Coord,
    pub spacing: Coord,
    pub padding: &'a Padding,
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
    use stretch::geometry::*;
    use stretch::number::*;
    use stretch::style::*;

    let mut stretch = stretch::Stretch::new();

    let box_style = stretch::style::Style {
        size: Size { width: Dimension::Percent(1.), height: Dimension::Percent(1.) },
        flex_direction: FlexDirection::Row,
        flex_basis: Dimension::Percent(1.),
        justify_content: data.alignment.into(),
        align_content: data.alignment.into(),
        ..Default::default()
    };

    let stretch_factor = |cell: &BoxLayoutCellData| cell.constraint.stretch;
    let mut smaller_strecth: Option<f32> = None;
    data.cells.iter().map(stretch_factor).for_each(|x| {
        if x > 0. {
            smaller_strecth = Some(smaller_strecth.map(|y| y.min(x)).unwrap_or(x))
        }
    });

    //let stretch_factor_sum = data.cells.iter().map(stretch_factor).sum::<Coord>();

    let flex_box = stretch.new_node(box_style, vec![]).unwrap();

    for (index, cell) in data.cells.iter().enumerate() {
        let mut margin = Rect::default();
        if index != 0 {
            margin.start = Dimension::Points(data.spacing / 2.);
        }
        if index != data.cells.len() - 1 {
            margin.end = Dimension::Points(data.spacing / 2.);
        }
        let min = |m, m_p, tot| {
            if m_p <= 0.0 {
                if m <= 0.0 {
                    Dimension::Undefined
                } else {
                    Dimension::Points(m)
                }
            } else {
                if m <= 0.0 {
                    Dimension::Percent(m_p / 100.)
                } else {
                    Dimension::Points(m.min(m_p * tot / 100.))
                }
            }
        };
        let max = |m, m_p, tot| {
            if m_p >= 100. {
                if m == f32::MAX {
                    Dimension::Undefined
                } else {
                    Dimension::Points(m)
                }
            } else {
                if m == f32::MAX {
                    Dimension::Percent(m_p / 100.)
                } else {
                    Dimension::Points(m.max(m_p * tot / 100.))
                }
            }
        };
        let constraint = &cell.constraint;
        let min_size = Size {
            width: min(constraint.min, constraint.min_percent, data.size),
            height: Dimension::Undefined,
        };
        let max_size = Size {
            width: max(constraint.max, constraint.max_percent, data.size),
            height: Dimension::Undefined,
        };

        let flex_grow_shrink = if data.alignment != LayoutAlignment::stretch {
            0.
        } else if let Some(s) = smaller_strecth {
            stretch_factor(cell) / s
        } else {
            1.
        };

        let convert_preferred_size = |size| {
            if size != 0. {
                Dimension::Points(size)
            } else {
                Dimension::Auto
            }
        };

        let cell_style = Style {
            size: Size {
                width: convert_preferred_size(constraint.preferred),
                height: Dimension::Auto,
            },
            min_size,
            max_size,
            flex_grow: flex_grow_shrink,
            flex_shrink: flex_grow_shrink,
            flex_basis: min_size.width,
            margin,
            align_self: AlignSelf::Stretch,
            ..Default::default()
        };

        let cell_item = stretch.new_node(cell_style, vec![]).unwrap();
        stretch.add_child(flex_box, cell_item).unwrap();
    }

    stretch
        .compute_layout(
            flex_box,
            Size {
                width: Number::Defined(data.size - (data.padding.begin + data.padding.end)),
                height: Number::Undefined,
            },
        )
        .unwrap();

    let start_pos = data.padding.begin;

    let mut result = SharedVector::<f32>::default();
    result.resize(data.cells.len() * 2 + repeater_indexes.len(), 0.);
    let res = result.as_slice_mut();

    // The index/2 in result in which we should add the next repeated item
    let mut repeat_ofst =
        res.len() / 2 - repeater_indexes.iter().skip(1).step_by(2).sum::<u32>() as usize;
    // The index/2  in repeater_indexes
    let mut next_rep = 0;
    // The index/2 in result in which we should add the next non-repeated item
    let mut current_ofst = 0;
    for (idx, layout) in stretch
        .children(flex_box)
        .unwrap()
        .iter()
        .map(|child| stretch.layout(*child).unwrap())
        .enumerate()
    {
        let o = loop {
            if let Some(nr) = repeater_indexes.get(next_rep * 2) {
                let nr = *nr as usize;
                if nr == idx {
                    for o in 0..2 {
                        res[current_ofst * 2 + o] = (repeat_ofst * 2 + o) as _;
                    }
                    current_ofst += 1;
                }
                if idx >= nr {
                    if idx - nr == repeater_indexes[next_rep * 2 + 1] as usize {
                        next_rep += 1;
                        continue;
                    }
                    repeat_ofst += 1;
                    break repeat_ofst - 1;
                }
            }
            current_ofst += 1;
            break current_ofst - 1;
        };
        res[o * 2 + 0] = start_pos + layout.location.x;
        res[o * 2 + 1] = layout.size.width;
    }
    result
}

/// Return the LayoutInfo for a BoxLayout with the given cells.
pub fn box_layout_info<'a>(
    cells: Slice<'a, BoxLayoutCellData>,
    spacing: Coord,
    padding: &Padding,
    alignment: LayoutAlignment,
) -> LayoutInfo {
    let count = cells.len();
    if count < 1 {
        return LayoutInfo { max: 0., ..LayoutInfo::default() };
    };
    let is_stretch = alignment == LayoutAlignment::stretch;
    let extra_w = padding.begin + padding.end + spacing * (count - 1) as Coord;
    let min = cells.iter().map(|c| c.constraint.min).sum::<Coord>() + extra_w;
    let max = if is_stretch {
        (cells.iter().map(|c| c.constraint.max).sum::<Coord>() + extra_w).max(min)
    } else {
        f32::MAX
    };
    let stretch = cells.iter().map(|c| c.constraint.stretch).sum::<f32>();
    LayoutInfo { min, max, min_percent: 0., max_percent: 100., preferred: 0., stretch }
}

pub fn box_layout_info_ortho<'a>(
    cells: Slice<'a, BoxLayoutCellData>,
    padding: &Padding,
) -> LayoutInfo {
    let count = cells.len();
    if count < 1 {
        return LayoutInfo { max: 0., ..LayoutInfo::default() };
    };
    let extra_w = padding.begin + padding.end;

    let mut fold =
        cells.iter().fold(LayoutInfo { stretch: f32::MAX, ..Default::default() }, |a, b| {
            a.merge(&b.constraint)
        });
    fold.max = fold.max.max(fold.min);
    fold.min += extra_w;
    fold.max += extra_w;
    fold.preferred += extra_w;
    fold
}

#[repr(C)]
pub struct PathLayoutData<'a> {
    pub elements: &'a crate::graphics::PathData,
    pub item_count: u32,
    pub x: Coord,
    pub y: Coord,
    pub width: Coord,
    pub height: Coord,
    pub offset: f32,
}

#[repr(C)]
#[derive(Default)]
pub struct PathLayoutItemData {
    pub width: Coord,
    pub height: Coord,
}

pub fn solve_path_layout(data: &PathLayoutData, repeater_indexes: Slice<u32>) -> SharedVector<f32> {
    use lyon_geom::*;
    use lyon_path::iterator::PathIterator;

    // Clone of path elements is cheap because it's a clone of underlying SharedVector
    let mut path_iter = data.elements.clone().iter();
    path_iter.fit(data.width, data.height, None);

    let tolerance: f32 = 0.1; // lyon::tessellation::StrokeOptions::DEFAULT_TOLERANCE

    let segment_lengths: Vec<Coord> = path_iter
        .iter()
        .bezier_segments()
        .map(|segment| match segment {
            BezierSegment::Linear(line_segment) => line_segment.length(),
            BezierSegment::Quadratic(quadratic_segment) => {
                quadratic_segment.approximate_length(tolerance)
            }
            BezierSegment::Cubic(cubic_segment) => cubic_segment.approximate_length(tolerance),
        })
        .collect();

    let path_length: Coord = segment_lengths.iter().sum();
    // the max(2) is there to put the item in the middle when there is a single item
    let item_distance = 1. / ((data.item_count - 1) as f32).max(2.);

    let mut i = 0;
    let mut next_t: f32 = data.offset;
    if data.item_count == 1 {
        next_t += item_distance;
    }

    let mut result = SharedVector::<f32>::default();
    result.resize(data.item_count as usize * 2 + repeater_indexes.len(), 0.);
    let res = result.as_slice_mut();

    // The index/2 in result in which we should add the next repeated item
    let mut repeat_ofst =
        res.len() / 2 - repeater_indexes.iter().skip(1).step_by(2).sum::<u32>() as usize;
    // The index/2  in repeater_indexes
    let mut next_rep = 0;
    // The index/2 in result in which we should add the next non-repeated item
    let mut current_ofst = 0;

    'main_loop: while i < data.item_count {
        let mut current_length: f32 = 0.;
        next_t %= 1.;

        for (seg_idx, segment) in path_iter.iter().bezier_segments().enumerate() {
            let seg_len = segment_lengths[seg_idx];
            let seg_start = current_length;
            current_length += seg_len;

            let seg_end_t = (seg_start + seg_len) / path_length;

            while next_t <= seg_end_t {
                let local_t = ((next_t * path_length) - seg_start) / seg_len;

                let item_pos = segment.sample(local_t);

                let o = loop {
                    if let Some(nr) = repeater_indexes.get(next_rep * 2) {
                        let nr = *nr;
                        if nr == i {
                            for o in 0..4 {
                                res[current_ofst * 4 + o] = (repeat_ofst * 4 + o) as _;
                            }
                            current_ofst += 1;
                        }
                        if i >= nr {
                            if i - nr == repeater_indexes[next_rep * 2 + 1] {
                                next_rep += 1;
                                continue;
                            }
                            repeat_ofst += 1;
                            break repeat_ofst - 1;
                        }
                    }
                    current_ofst += 1;
                    break current_ofst - 1;
                };

                res[o * 2 + 0] = item_pos.x + data.x;
                res[o * 2 + 1] = item_pos.y + data.y;
                i += 1;
                next_t += item_distance;
                if i >= data.item_count {
                    break 'main_loop;
                }
            }

            if next_t > 1. {
                break;
            }
        }
    }

    result
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[no_mangle]
    pub extern "C" fn sixtyfps_solve_grid_layout(
        data: &GridLayoutData,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_grid_layout(data)
    }

    #[no_mangle]
    pub extern "C" fn sixtyfps_grid_layout_info<'a>(
        cells: Slice<'a, GridLayoutCellData>,
        spacing: Coord,
        padding: &Padding,
    ) -> LayoutInfo {
        super::grid_layout_info(cells, spacing, padding)
    }

    #[no_mangle]
    pub extern "C" fn sixtyfps_solve_box_layout(
        data: &BoxLayoutData,
        repeater_indexes: Slice<u32>,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_box_layout(data, repeater_indexes)
    }

    #[no_mangle]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn sixtyfps_box_layout_info<'a>(
        cells: Slice<'a, BoxLayoutCellData>,
        spacing: Coord,
        padding: &Padding,
        alignment: LayoutAlignment,
    ) -> LayoutInfo {
        super::box_layout_info(cells, spacing, padding, alignment)
    }

    #[no_mangle]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn sixtyfps_box_layout_info_ortho<'a>(
        cells: Slice<'a, BoxLayoutCellData>,
        padding: &Padding,
    ) -> LayoutInfo {
        super::box_layout_info_ortho(cells, padding)
    }

    #[no_mangle]
    pub extern "C" fn sixtyfps_solve_path_layout(
        data: &PathLayoutData,
        repeater_indexes: Slice<u32>,
        result: &mut SharedVector<Coord>,
    ) {
        *result = super::solve_path_layout(data, repeater_indexes)
    }
}
