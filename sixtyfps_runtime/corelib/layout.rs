//! Runtime support for layouting.
//!
//! Currently this is a very basic implementation

use crate::{abi::slice::Slice, Property};

type Coord = f32;

mod internal {
    use super::*;

    #[derive(Debug, Default)]
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

    /// Layout the items within a specified size
    ///
    /// This is quite a simple implementation for now
    pub fn layout_items(data: &mut [LayoutData], start_pos: Coord, size: Coord) {
        let (min, _max, perf, mut s) =
            data.iter().fold((0., 0., 0., 0.), |(min, max, pref, s), it| {
                (min + it.min, max + it.max, pref + it.pref, s + it.stretch)
            });
        if size >= perf {
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
            let mut extra_space = size - perf;
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
                    pos += it.size;
                }
            }
        } else
        /*if size < min*/
        {
            // We have less than the minimum size
            // distribute the difference proportional to the size (TODO: and stretch)
            let ratio = size / min;
            let mut pos = start_pos;
            for it in data {
                it.size = it.min * ratio;
                it.pos = pos;
                pos += it.size;
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

        layout_items(my_items, 100., 650.);
        assert_eq!(my_items[0].size, 200.);
        assert_eq!(my_items[1].size, 300.);
        assert_eq!(my_items[2].size, 150.);

        layout_items(my_items, 100., 200.);
        assert_eq!(my_items[0].size, 100.);
        assert_eq!(my_items[1].size, 50.);
        assert_eq!(my_items[2].size, 50.);

        layout_items(my_items, 100., 300.);
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
pub struct GridLayoutData<'a> {
    pub row_constraint: Slice<'a, Constraint>,
    pub col_constraint: Slice<'a, Constraint>,
    pub width: Coord,
    pub height: Coord,
    pub x: Coord,
    pub y: Coord,
    pub cells: Slice<'a, Slice<'a, GridLayoutCellData<'a>>>,
}

#[repr(C)]
#[derive(Default)]
pub struct GridLayoutCellData<'a> {
    pub x: Option<&'a Property<Coord>>,
    pub y: Option<&'a Property<Coord>>,
    pub width: Option<&'a Property<Coord>>,
    pub height: Option<&'a Property<Coord>>,
}

/// FIXME: rename with sixstyfps prefix
#[no_mangle]
pub extern "C" fn solve_grid_layout(data: &GridLayoutData) {
    let map = |c: &Constraint| internal::LayoutData {
        min: c.min,
        max: c.max,
        pref: c.min,
        stretch: 1.,
        pos: 0.,
        size: 0.,
    };

    let mut row_layout_data = data.row_constraint.iter().map(map).collect::<Vec<_>>();
    let mut col_layout_data = data.col_constraint.iter().map(map).collect::<Vec<_>>();
    internal::layout_items(&mut row_layout_data, data.y, data.height);
    internal::layout_items(&mut col_layout_data, data.x, data.width);
    for (row_data, row) in row_layout_data.iter().zip(data.cells.iter()) {
        for (col_data, cell) in col_layout_data.iter().zip(row.iter()) {
            cell.x.map(|p| p.set(col_data.pos));
            cell.width.map(|p| p.set(col_data.size));
            cell.y.map(|p| p.set(row_data.pos));
            cell.height.map(|p| p.set(row_data.size));
        }
    }
}

/// Somehow this is required for the extern "C" things to be exported in a dependent dynlib
#[doc(hidden)]
pub fn dummy() {
    #[derive(Clone)]
    struct Foo;
    foo(Foo);
    fn foo(f: impl Clone) {
        let _ = f.clone();
    }
}
