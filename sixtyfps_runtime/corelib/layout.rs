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

use crate::{slice::Slice, Property};

type Coord = f32;

/// The constraint that applies to an item
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutInfo {
    /// The minimum width for the item.
    pub min_width: f32,
    /// The maximum width for the item.
    pub max_width: f32,
    /// The minimum height for the item.
    pub min_height: f32,
    /// The maximum height for the item.
    pub max_height: f32,

    /// The minimum width in percentage of the parent (value between 0 and 100).
    pub min_width_percent: f32,
    /// The maximum width in percentage of the parent (value between 0 and 100).
    pub max_width_percent: f32,
    /// The minimum height in percentage of the parent (value between 0 and 100).
    pub min_height_percent: f32,
    /// The maximum height in percentage of the parent (value between 0 and 100).
    pub max_height_percent: f32,

    /// the horizontal stretch factor
    pub horizontal_stretch: f32,
    /// the vertical stretch factor
    pub vertical_stretch: f32,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        LayoutInfo {
            min_width: 0.,
            max_width: f32::MAX,
            min_height: 0.,
            max_height: f32::MAX,
            min_width_percent: 0.,
            max_width_percent: 100.,
            min_height_percent: 0.,
            max_height_percent: 100.,
            horizontal_stretch: 0.,
            vertical_stretch: 0.,
        }
    }
}

impl LayoutInfo {
    // Note: This "logic" is duplicated in the cpp generator's generated code for merging layout infos.
    pub fn merge(&self, other: &LayoutInfo) -> Self {
        Self {
            min_width: self.min_width.max(other.min_width),
            max_width: self.max_width.min(other.max_width),
            min_height: self.min_height.max(other.min_height),
            max_height: self.max_height.min(other.max_height),
            min_width_percent: self.min_width_percent.max(other.min_width_percent),
            max_width_percent: self.max_width_percent.min(other.max_width_percent),
            min_height_percent: self.min_height_percent.max(other.min_height_percent),
            max_height_percent: self.max_height_percent.min(other.max_height_percent),
            horizontal_stretch: self.horizontal_stretch.min(other.horizontal_stretch),
            vertical_stretch: self.vertical_stretch.min(other.vertical_stretch),
        }
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
    pub left: Coord,
    pub right: Coord,
    pub top: Coord,
    pub bottom: Coord,
}

#[repr(C)]
#[derive(Debug)]
pub struct GridLayoutData<'a> {
    pub width: Coord,
    pub height: Coord,
    pub x: Coord,
    pub y: Coord,
    pub spacing: Coord,
    pub padding: &'a Padding,
    pub cells: Slice<'a, GridLayoutCellData<'a>>,
}

#[repr(C)]
#[derive(Default, Debug)]
pub struct GridLayoutCellData<'a> {
    pub col: u16,
    pub row: u16,
    pub colspan: u16,
    pub rowspan: u16,
    pub constraint: LayoutInfo,
    pub x: Option<&'a Property<Coord>>,
    pub y: Option<&'a Property<Coord>>,
    pub width: Option<&'a Property<Coord>>,
    pub height: Option<&'a Property<Coord>>,
}

pub fn solve_grid_layout(data: &GridLayoutData) {
    let (mut num_col, mut num_row) = (0, 0);
    for cell in data.cells.iter() {
        num_row = num_row.max(cell.row + cell.rowspan);
        num_col = num_col.max(cell.col + cell.colspan);
    }

    if num_col < 1 || num_row < 1 {
        return;
    }

    let mut row_layout_data = vec![grid_internal::LayoutData::default(); num_row as usize];
    let mut col_layout_data = vec![grid_internal::LayoutData::default(); num_col as usize];

    for cell in data.cells.iter() {
        let cnstr = &cell.constraint;
        let row_max = cnstr.max_height.min(data.height * cnstr.max_height_percent / 100.);
        let row_min = cnstr.min_height.max(data.height * cnstr.min_height_percent / 100.)
            / (cell.rowspan as f32);
        let row_pref = row_min;

        for r in 0..(cell.rowspan as usize) {
            let rdata = &mut row_layout_data[cell.row as usize + r];
            rdata.max = rdata.max.min(row_max);
            rdata.min = rdata.min.max(row_min);
            rdata.pref = rdata.pref.max(row_pref);
            rdata.stretch = rdata.stretch.min(cnstr.vertical_stretch);
        }

        let col_max = cnstr.max_width.min(data.width * cnstr.max_width_percent / 100.);
        let col_min = cnstr.min_width.max(data.width * cnstr.min_width_percent / 100.)
            / (cell.colspan as f32);
        let col_pref = col_min;

        for c in 0..(cell.colspan as usize) {
            let cdata = &mut col_layout_data[cell.col as usize + c];
            cdata.max = cdata.max.min(col_max);
            cdata.min = cdata.min.max(col_min);
            cdata.pref = cdata.pref.max(col_pref);
            cdata.stretch = cdata.stretch.min(cnstr.horizontal_stretch);
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
    normalize_stretch(&mut row_layout_data);
    normalize_stretch(&mut col_layout_data);

    grid_internal::layout_items(
        &mut row_layout_data,
        data.y + data.padding.top,
        data.height - (data.padding.top + data.padding.bottom),
        data.spacing,
    );
    grid_internal::layout_items(
        &mut col_layout_data,
        data.x + data.padding.left,
        data.width - (data.padding.left + data.padding.right),
        data.spacing,
    );
    for cell in data.cells.iter() {
        let rdata = &row_layout_data[cell.row as usize];
        let cdata = &col_layout_data[cell.col as usize];
        cell.x.map(|p| p.set(cdata.pos));
        cell.width.map(|p| {
            p.set({
                let first_cell = &col_layout_data[cell.col as usize];
                let last_cell = &col_layout_data[cell.col as usize + cell.colspan as usize - 1];
                last_cell.pos + last_cell.size - first_cell.pos
            })
        });
        cell.y.map(|p| p.set(rdata.pos));
        cell.height.map(|p| {
            p.set({
                let first_cell = &row_layout_data[cell.row as usize];
                let last_cell = &row_layout_data[cell.row as usize + cell.rowspan as usize - 1];
                last_cell.pos + last_cell.size - first_cell.pos
            })
        });
    }
}

pub fn grid_layout_info<'a>(
    cells: &Slice<'a, GridLayoutCellData<'a>>,
    spacing: Coord,
    padding: &Padding,
) -> LayoutInfo {
    let (mut num_col, mut num_row) = (0, 0);
    for cell in cells.iter() {
        num_row = num_row.max(cell.row + cell.rowspan);
        num_col = num_col.max(cell.col + cell.colspan);
    }

    if num_col < 1 || num_row < 1 {
        return LayoutInfo { max_width: 0., max_height: 0., ..LayoutInfo::default() };
    };

    let mut row_layout_data = vec![grid_internal::LayoutData::default(); num_row as usize];
    let mut col_layout_data = vec![grid_internal::LayoutData::default(); num_col as usize];
    for cell in cells.iter() {
        let rdata = &mut row_layout_data[cell.row as usize];
        let cdata = &mut col_layout_data[cell.col as usize];
        rdata.max = rdata.max.min(cell.constraint.max_height);
        cdata.max = cdata.max.min(cell.constraint.max_width);
        rdata.min = rdata.min.max(cell.constraint.min_height);
        cdata.min = cdata.min.max(cell.constraint.min_width);
        rdata.pref = rdata.pref.max(cell.constraint.min_height);
        cdata.pref = cdata.pref.max(cell.constraint.min_width);
        rdata.stretch = rdata.stretch.min(cell.constraint.vertical_stretch);
        cdata.stretch = cdata.stretch.min(cell.constraint.horizontal_stretch);
    }

    let spacing_h = spacing * (num_row - 1) as Coord;
    let spacing_w = spacing * (num_col - 1) as Coord;

    let min_height = row_layout_data.iter().map(|data| data.min).sum::<Coord>()
        + spacing_h
        + padding.top
        + padding.bottom;
    let max_height = row_layout_data.iter().map(|data| data.max).sum::<Coord>()
        + spacing_h
        + padding.top
        + padding.bottom;
    let min_width = col_layout_data.iter().map(|data| data.min).sum::<Coord>()
        + spacing_w
        + padding.left
        + padding.right;
    let max_width = col_layout_data.iter().map(|data| data.max).sum::<Coord>()
        + spacing_w
        + padding.left
        + padding.right;

    let horizontal_stretch = col_layout_data.iter().map(|data| data.stretch).sum::<Coord>();
    let vertical_stretch = row_layout_data.iter().map(|data| data.stretch).sum::<Coord>();

    LayoutInfo {
        min_width,
        max_width,
        min_height,
        max_height,
        min_width_percent: 0.,
        max_width_percent: 100.,
        min_height_percent: 0.,
        max_height_percent: 100.,
        horizontal_stretch,
        vertical_stretch,
    }
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
    pub width: Coord,
    pub height: Coord,
    pub x: Coord,
    pub y: Coord,
    pub spacing: Coord,
    pub padding: &'a Padding,
    pub alignment: LayoutAlignment,
    pub cells: Slice<'a, BoxLayoutCellData<'a>>,
}

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct BoxLayoutCellData<'a> {
    pub constraint: LayoutInfo,
    pub x: Option<&'a Property<Coord>>,
    pub y: Option<&'a Property<Coord>>,
    pub width: Option<&'a Property<Coord>>,
    pub height: Option<&'a Property<Coord>>,
}

/// Solve a BoxLayout
pub fn solve_box_layout(data: &BoxLayoutData, is_horizontal: bool) {
    use stretch::geometry::*;
    use stretch::number::*;
    use stretch::style::*;

    let mut stretch = stretch::Stretch::new();

    let box_style = stretch::style::Style {
        size: Size { width: Dimension::Percent(1.), height: Dimension::Percent(1.) },
        flex_direction: if is_horizontal { FlexDirection::Row } else { FlexDirection::Column },
        flex_basis: Dimension::Percent(1.),
        justify_content: data.alignment.into(),
        align_content: data.alignment.into(),
        ..Default::default()
    };

    let stretch_factor = |cell: &BoxLayoutCellData| {
        if is_horizontal {
            cell.constraint.horizontal_stretch
        } else {
            cell.constraint.vertical_stretch
        }
    };
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
        if is_horizontal {
            if index != 0 {
                margin.start = Dimension::Points(data.spacing / 2.);
            }
            if index != data.cells.len() - 1 {
                margin.end = Dimension::Points(data.spacing / 2.);
            }
        } else {
            if index != 0 {
                margin.top = Dimension::Points(data.spacing / 2.);
            }
            if index != data.cells.len() - 1 {
                margin.bottom = Dimension::Points(data.spacing / 2.);
            }
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
            width: min(constraint.min_width, constraint.min_width_percent, data.width),
            height: min(constraint.min_height, constraint.min_height_percent, data.height),
        };
        let max_size = Size {
            width: max(constraint.max_width, constraint.max_width_percent, data.width),
            height: max(constraint.max_height, constraint.max_height_percent, data.height),
        };
        let cell_style = Style {
            min_size,
            max_size,
            flex_grow: if data.alignment != LayoutAlignment::stretch {
                0.
            } else if let Some(s) = smaller_strecth {
                stretch_factor(cell) / s
            } else {
                1.
            },
            flex_basis: if is_horizontal { min_size.width } else { min_size.height },
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
                width: Number::Defined(data.width - (data.padding.left + data.padding.right)),
                height: Number::Defined(data.height - (data.padding.top + data.padding.bottom)),
            },
        )
        .unwrap();

    let start_pos_x = data.x + data.padding.left;
    let start_pos_y = data.y + data.padding.top;

    for (cell, layout) in data.cells.iter().zip(
        stretch.children(flex_box).unwrap().iter().map(|child| stretch.layout(*child).unwrap()),
    ) {
        cell.x.map(|p| p.set(start_pos_x + layout.location.x));
        cell.y.map(|p| p.set(start_pos_y + layout.location.y));
        cell.width.map(|p| p.set(layout.size.width));
        cell.height.map(|p| p.set(layout.size.height));
    }
}

/// Return the LayoutInfo for a BoxLayout with the given cells.
pub fn box_layout_info<'a>(
    cells: &Slice<'a, BoxLayoutCellData<'a>>,
    spacing: Coord,
    padding: &Padding,
    alignment: LayoutAlignment,
    is_horizontal: bool,
) -> LayoutInfo {
    let count = cells.len();
    if count < 1 {
        return LayoutInfo { max_width: 0., max_height: 0., ..LayoutInfo::default() };
    };
    let order_float = |a: &Coord, b: &Coord| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal);
    let is_stretch = alignment == LayoutAlignment::stretch;

    if is_horizontal {
        let extra_w = padding.left + padding.right + spacing * (count - 1) as Coord;

        let min_height = cells.iter().map(|c| c.constraint.min_height).max_by(order_float).unwrap()
            + padding.top
            + padding.bottom;
        let max_height =
            (cells.iter().map(|c| c.constraint.max_height).min_by(order_float).unwrap()
                + padding.top
                + padding.bottom)
                .max(min_height);
        let min_width = cells.iter().map(|c| c.constraint.min_width).sum::<Coord>() + extra_w;
        let max_width = if is_stretch {
            (cells.iter().map(|c| c.constraint.max_width).sum::<Coord>() + extra_w).max(min_width)
        } else {
            f32::MAX
        };
        let horizontal_stretch = cells.iter().map(|c| c.constraint.horizontal_stretch).sum::<f32>();
        let vertical_stretch =
            cells.iter().map(|c| c.constraint.vertical_stretch).min_by(order_float).unwrap();
        LayoutInfo {
            min_width,
            max_width,
            min_height,
            max_height,
            min_width_percent: 0.,
            max_width_percent: 100.,
            min_height_percent: 0.,
            max_height_percent: 100.,
            horizontal_stretch,
            vertical_stretch,
        }
    } else {
        let extra_h = padding.top + padding.bottom + spacing * (count - 1) as Coord;

        let min_width = cells.iter().map(|c| c.constraint.min_width).max_by(order_float).unwrap()
            + padding.left
            + padding.right;
        let max_width = (cells.iter().map(|c| c.constraint.max_width).min_by(order_float).unwrap()
            + padding.left
            + padding.right)
            .max(min_width);
        let min_height = cells.iter().map(|c| c.constraint.min_height).sum::<Coord>() + extra_h;
        let max_height = if is_stretch {
            (cells.iter().map(|c| c.constraint.max_height).sum::<Coord>() + extra_h).max(min_height)
        } else {
            f32::MAX
        };
        let horizontal_stretch =
            cells.iter().map(|c| c.constraint.horizontal_stretch).min_by(order_float).unwrap();
        let vertical_stretch = cells.iter().map(|c| c.constraint.vertical_stretch).sum::<f32>();
        LayoutInfo {
            min_width,
            max_width,
            min_height,
            max_height,
            min_width_percent: 0.,
            max_width_percent: 100.,
            min_height_percent: 0.,
            max_height_percent: 100.,
            horizontal_stretch,
            vertical_stretch,
        }
    }
}

#[repr(C)]
pub struct PathLayoutData<'a> {
    pub elements: &'a crate::graphics::PathData,
    pub items: Slice<'a, PathLayoutItemData<'a>>,
    pub x: Coord,
    pub y: Coord,
    pub width: Coord,
    pub height: Coord,
    pub offset: f32,
}

#[repr(C)]
#[derive(Default)]
pub struct PathLayoutItemData<'a> {
    pub x: Option<&'a Property<Coord>>,
    pub y: Option<&'a Property<Coord>>,
    pub width: Coord,
    pub height: Coord,
}

pub fn solve_path_layout(data: &PathLayoutData) {
    use lyon_geom::*;
    use lyon_path::iterator::PathIterator;

    if data.items.is_empty() {
        return;
    }

    // Clone of path elements is cheap because it's a clone of underlying SharedVector
    let path_iter = data.elements.clone().iter_fitted(data.width, data.height);

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
    let item_distance = 1. / ((data.items.len() - 1) as f32).max(2.);

    let mut i = 0;
    let mut next_t: f32 = data.offset;
    if data.items.len() == 1 {
        next_t += item_distance;
    }
    'main_loop: while i < data.items.len() {
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
                let center_x_offset = data.items[i].width / 2.;
                let center_y_offset = data.items[i].height / 2.;
                data.items[i].x.map(|prop| prop.set(item_pos.x - center_x_offset + data.x));
                data.items[i].y.map(|prop| prop.set(item_pos.y - center_y_offset + data.y));

                i += 1;
                next_t += item_distance;
                if i >= data.items.len() {
                    break 'main_loop;
                }
            }

            if next_t > 1. {
                break;
            }
        }
    }
}

pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[no_mangle]
    pub extern "C" fn sixtyfps_solve_grid_layout(data: &GridLayoutData) {
        super::solve_grid_layout(data)
    }

    #[no_mangle]
    pub extern "C" fn sixtyfps_grid_layout_info<'a>(
        cells: &Slice<'a, GridLayoutCellData<'a>>,
        spacing: Coord,
        padding: &Padding,
    ) -> LayoutInfo {
        super::grid_layout_info(cells, spacing, padding)
    }

    #[no_mangle]
    pub extern "C" fn sixtyfps_solve_box_layout(data: &BoxLayoutData, is_horizontal: bool) {
        super::solve_box_layout(data, is_horizontal)
    }

    #[no_mangle]
    /// Return the LayoutInfo for a BoxLayout with the given cells.
    pub extern "C" fn sixtyfps_box_layout_info<'a>(
        cells: &Slice<'a, BoxLayoutCellData<'a>>,
        spacing: Coord,
        padding: &Padding,
        alignment: LayoutAlignment,
        is_horizontal: bool,
    ) -> LayoutInfo {
        super::box_layout_info(cells, spacing, padding, alignment, is_horizontal)
    }

    #[no_mangle]
    pub extern "C" fn sixtyfps_solve_path_layout(data: &PathLayoutData) {
        super::solve_path_layout(data)
    }
}
