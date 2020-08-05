//! Runtime support for layouting.
//!
//! Currently this is a very basic implementation

use crate::{slice::Slice, Property};

type Coord = f32;

/// The constraint that applies to an item
#[repr(C)]
#[derive(Clone, Debug)]
pub struct LayoutInfo {
    /// The minimum width for the item.
    pub min_width: f32,
    /// The maximum width for the item.
    pub max_width: f32,
    /// The minimum height for the item.
    pub min_height: f32,
    /// The maximum height for the item.
    pub max_height: f32,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        LayoutInfo { min_width: 0., max_width: f32::MAX, min_height: 0., max_height: f32::MAX }
    }
}

mod internal {
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
            LayoutData { min: 0., max: Coord::MAX, pref: 0., stretch: 1., pos: 0., size: 0. }
        }
    }

    /// Layout the items within a specified size
    ///
    /// This is quite a simple implementation for now
    pub fn layout_items(data: &mut [LayoutData], start_pos: Coord, size: Coord, spacing: Coord) {
        let (min, _max, perf, mut s) =
            data.iter().fold((0., 0., 0., 0.), |(min, max, pref, s), it| {
                (min + it.min, max + it.max, pref + it.pref, s + it.stretch)
            });

        //let total_spacing = spacing * (data.len() - 1) as Coord;
        let size_without_spacing = size - spacing * (data.len() - 1) as Coord;

        if size_without_spacing >= perf {
            // bigger than the prefered size

            // distribute each item its prefered size
            let mut pos = start_pos;
            for it in data.iter_mut() {
                it.size = it.pref;
                it.pos = pos;
                pos += it.size;
            }

            // Allocate the space according to the stretch. Until all space is distributed, or all item
            // have reached their maximum size
            let mut extra_space = size_without_spacing - perf;
            while s > 0. && extra_space > 0. {
                let extra_per_stretch = extra_space / s;
                s = 0.;
                let mut pos = start_pos;
                for it in data.iter_mut() {
                    let give = (extra_per_stretch * it.stretch).min(it.max - it.size);
                    it.size += give;
                    extra_space -= give;
                    if give > 0. {
                        s += it.stretch;
                    }
                    it.pos = pos;
                    pos += it.size + spacing;
                }
            }
        } else
        /*if size < min*/
        {
            // We have less than the minimum size
            // distribute the difference proportional to the size (TODO: and stretch)
            let ratio = size_without_spacing / min;
            let mut pos = start_pos;
            for it in data {
                it.size = it.min * ratio;
                it.pos = pos;
                pos += it.size + spacing;
            }
        }
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
#[derive(Debug)]
pub struct GridLayoutData<'a> {
    pub width: Coord,
    pub height: Coord,
    pub x: Coord,
    pub y: Coord,
    pub spacing: Coord,
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

/// FIXME: rename with sixstyfps prefix
#[no_mangle]
pub extern "C" fn solve_grid_layout(data: &GridLayoutData) {
    let (mut num_col, mut num_row) = (0, 0);
    for cell in data.cells.iter() {
        num_row = num_row.max(cell.row + cell.rowspan);
        num_col = num_col.max(cell.col + cell.colspan);
    }

    if num_col < 1 || num_row < 1 {
        return;
    }

    let mut row_layout_data = vec![internal::LayoutData::default(); num_row as usize];
    let mut col_layout_data = vec![internal::LayoutData::default(); num_col as usize];
    for cell in data.cells.iter() {
        let rdata = &mut row_layout_data[cell.row as usize];
        let cdata = &mut col_layout_data[cell.col as usize];
        rdata.max = rdata.max.min(cell.constraint.max_height);
        cdata.max = cdata.max.min(cell.constraint.max_width);
        rdata.min = rdata.min.max(cell.constraint.min_height);
        cdata.min = cdata.min.max(cell.constraint.min_width);
        rdata.pref = rdata.pref.max(cell.constraint.min_height);
        cdata.pref = cdata.pref.max(cell.constraint.min_width);
    }

    internal::layout_items(&mut row_layout_data, data.y, data.height, data.spacing);
    internal::layout_items(&mut col_layout_data, data.x, data.width, data.spacing);
    for cell in data.cells.iter() {
        let rdata = &row_layout_data[cell.row as usize];
        let cdata = &col_layout_data[cell.col as usize];
        cell.x.map(|p| p.set(cdata.pos));
        cell.width.map(|p| p.set(cdata.size));
        cell.y.map(|p| p.set(rdata.pos));
        cell.height.map(|p| p.set(rdata.size));
    }
}

#[no_mangle]
pub extern "C" fn grid_layout_info<'a>(
    cells: &Slice<'a, GridLayoutCellData<'a>>,
    spacing: Coord,
) -> LayoutInfo {
    let (mut num_col, mut num_row) = (0, 0);
    for cell in cells.iter() {
        num_row = num_row.max(cell.row + cell.rowspan);
        num_col = num_col.max(cell.col + cell.colspan);
    }

    if num_col < 1 || num_row < 1 {
        return LayoutInfo { min_width: 0., max_width: 0., min_height: 0., max_height: 0. };
    };

    let mut row_layout_data = vec![internal::LayoutData::default(); num_row as usize];
    let mut col_layout_data = vec![internal::LayoutData::default(); num_col as usize];
    for cell in cells.iter() {
        let rdata = &mut row_layout_data[cell.row as usize];
        let cdata = &mut col_layout_data[cell.col as usize];
        rdata.max = rdata.max.min(cell.constraint.max_height);
        cdata.max = cdata.max.min(cell.constraint.max_width);
        rdata.min = rdata.min.max(cell.constraint.min_height);
        cdata.min = cdata.min.max(cell.constraint.min_width);
        rdata.pref = rdata.pref.max(cell.constraint.min_height);
        cdata.pref = cdata.pref.max(cell.constraint.min_width);
    }

    let spacing_h = spacing * (num_row - 1) as Coord;
    let spacing_w = spacing * (num_col - 1) as Coord;

    let min_height = row_layout_data.iter().map(|data| data.min).sum::<Coord>() * spacing_h;
    let max_height = row_layout_data.iter().map(|data| data.max).sum::<Coord>() * spacing_h;
    let min_width = col_layout_data.iter().map(|data| data.min).sum::<Coord>() * spacing_w;
    let max_width = col_layout_data.iter().map(|data| data.max).sum::<Coord>() * spacing_w;

    LayoutInfo { min_width, max_width, min_height, max_height }
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

/// FIXME: rename with sixstyfps prefix
#[no_mangle]
pub extern "C" fn solve_path_layout(data: &PathLayoutData) {
    use lyon::geom::*;
    use lyon::path::iterator::PathIterator;

    if data.items.is_empty() {
        return;
    }

    let path_iter = data.elements.iter_fitted(data.width, data.height);

    let tolerance = lyon::tessellation::StrokeOptions::DEFAULT_TOLERANCE;

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
