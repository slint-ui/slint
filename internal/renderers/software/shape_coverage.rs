// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The anti-aliased coverage of rectangles whose corners have a `CornerShape`.

use super::PhysicalRect;
use alloc::vec::Vec;

/// Pixels of a row whose coverage is the same in each of `N` layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run<const N: usize> {
    /// The column after the run, relative to [`ShapeCoverage::rect`].
    end: u16,
    /// From 0 to 255.
    pub coverage: [u8; N],
}

/// How much of each pixel `N` layers cover, row by row,
/// as runs of pixels with the same coverage.
#[derive(Debug)]
pub struct ShapeCoverage<const N: usize> {
    /// The covered pixels, in screen coordinates.
    pub rect: PhysicalRect,
    /// The first row of each group of identical rows, relative to `rect`,
    /// and the start of its runs in `runs`.
    rows: Vec<(u16, u32)>,
    runs: Vec<Run<N>>,
}

impl<const N: usize> ShapeCoverage<N> {
    /// Run-length encodes `masks`,
    /// which hold a coverage byte for each pixel of `rect`, row by row.
    /// `map` turns the coverage of each mask at a pixel into the stored coverage.
    /// Returns `None` if `map` returns 0 for every pixel.
    #[cfg_attr(
        not(feature = "path"),
        expect(dead_code, reason = "only the path feature draws corner shapes")
    )]
    pub fn new<const L: usize>(
        rect: PhysicalRect,
        masks: [&[u8]; L],
        map: impl Fn([u8; L]) -> [u8; N],
    ) -> Option<Self> {
        let width = rect.width().max(0) as usize;
        let height = rect.height().max(0) as usize;
        debug_assert!(masks.iter().all(|mask| mask.len() == width * height));
        let at = |row: usize, column: usize| map(masks.map(|mask| mask[row * width + column]));

        let (mut left, mut right, mut top, mut bottom) = (width, 0, None, 0);
        for row in 0..height {
            let Some(first) = (0..width).find(|&column| at(row, column) != [0; N]) else {
                continue;
            };
            let last = (first..width).rfind(|&column| at(row, column) != [0; N]).unwrap_or(first);
            (left, right) = (left.min(first), right.max(last + 1));
            top.get_or_insert(row);
            bottom = row + 1;
        }
        let top = top?;
        let (width, height) = (right - left, bottom - top);

        let mut this = Self {
            rect: PhysicalRect::new(
                rect.origin + euclid::vec2(left as i16, top as i16),
                euclid::size2(width as i16, height as i16),
            ),
            rows: Vec::new(),
            runs: Vec::new(),
        };
        for row in 0..height {
            let start = this.runs.len();
            for column in 0..width {
                let coverage = at(top + row, left + column);
                let end = column as u16 + 1;
                let in_row = this.runs.len() > start;
                match this.runs.last_mut() {
                    Some(last) if in_row && last.coverage == coverage => last.end = end,
                    _ => this.runs.push(Run { end, coverage }),
                }
            }
            let previous = this.rows.last().map(|&(_, start)| start as usize);
            if previous.is_some_and(|previous| this.runs[previous..start] == this.runs[start..]) {
                this.runs.truncate(start);
            } else {
                this.rows.push((row as u16, start as u32));
            }
        }

        Some(this)
    }

    /// The largest rectangle of pixels that every layer covers fully.
    #[cfg_attr(
        not(feature = "path"),
        expect(dead_code, reason = "only the path feature draws corner shapes")
    )]
    pub fn opaque(&self) -> PhysicalRect {
        let mut opaque_rows: (core::ops::Range<u16>, core::ops::Range<u16>) = (0..0, 0..0);
        // The largest group of identical rows tends to be the one between the corners.
        for (i, &(first, start)) in self.rows.iter().enumerate() {
            let (end_row, end) = self
                .rows
                .get(i + 1)
                .map_or((self.rect.height() as u16, self.runs.len()), |&(row, start)| {
                    (row, start as usize)
                });
            let mut column = 0;
            for run in &self.runs[start as usize..end] {
                let columns = column..run.end;
                column = run.end;
                let area = |(rows, columns): &(core::ops::Range<u16>, core::ops::Range<u16>)| {
                    rows.len() * columns.len()
                };
                let candidate = (first..end_row, columns);
                if run.coverage == [255; N] && area(&candidate) > area(&opaque_rows) {
                    opaque_rows = candidate;
                }
            }
        }
        let (rows, columns) = opaque_rows;
        PhysicalRect::new(
            self.rect.origin + euclid::vec2(columns.start as i16, rows.start as i16),
            euclid::size2(columns.len() as i16, rows.len() as i16),
        )
    }

    /// The runs of screen line `line`, or `None` outside of [`Self::rect`].
    pub fn row(&self, line: i16) -> Option<&[Run<N>]> {
        let row = line as i32 - self.rect.min_y() as i32;
        if row < 0 || row >= self.rect.height() as i32 {
            return None;
        }
        let i = self.rows.partition_point(|&(first, _)| first as i32 <= row) - 1;
        let end = self.rows.get(i + 1).map_or(self.runs.len(), |&(_, start)| start as usize);
        Some(&self.runs[self.rows[i].1 as usize..end])
    }

    /// Calls `f` with the coverage of each run of the `len` pixels of `line`,
    /// starting at screen column `x`, and the run's range as indices from `x`.
    pub fn for_each_run(
        &self,
        line: i16,
        x: i32,
        len: usize,
        mut f: impl FnMut(core::ops::Range<usize>, [u8; N]),
    ) {
        let Some(runs) = self.row(line) else { return };
        let first = x - self.rect.min_x() as i32;
        let to_index = |column: i32| (column - first).clamp(0, len as i32) as usize;
        let start = runs.partition_point(|run| run.end as i32 <= first);
        let mut column = start.checked_sub(1).map_or(0, |i| runs[i].end as i32);
        for run in &runs[start..] {
            let range = to_index(column)..to_index(run.end as i32);
            column = run.end as i32;
            if range.is_empty() {
                if range.start == len {
                    break;
                }
                continue;
            }
            f(range, run.coverage);
        }
    }
}

/// A clip whose corners have a `CornerShape`.
#[derive(Debug)]
pub struct ShapedClip {
    pub coverage: ShapeCoverage<1>,
    /// The pixels that [`Self::coverage`] covers fully.
    pub opaque: PhysicalRect,
}

impl ShapedClip {
    #[cfg_attr(
        not(feature = "path"),
        expect(dead_code, reason = "only the path feature draws corner shapes")
    )]
    pub fn new(coverage: ShapeCoverage<1>) -> Self {
        Self { opaque: coverage.opaque(), coverage }
    }
}

/// The coverage of a single layer of `runs` at `column`, relative to [`ShapeCoverage::rect`].
pub fn coverage_at(runs: &[Run<1>], column: i32) -> u8 {
    runs.get(runs.partition_point(|run| (run.end as i32) <= column)).map_or(0, |r| r.coverage[0])
}

/// The columns from the first covered pixel of `runs` to the last,
/// relative to [`ShapeCoverage::rect`].
pub fn covered_columns(runs: &[Run<1>]) -> Option<core::ops::Range<i32>> {
    let mut column = 0;
    let mut found: Option<core::ops::Range<i32>> = None;
    for run in runs {
        if run.coverage[0] > 0 {
            found = Some(found.map_or(column, |found| found.start)..run.end as i32);
        }
        column = run.end as i32;
    }
    found
}

/// The longest run of `runs` that covers its pixels fully,
/// as columns relative to [`ShapeCoverage::rect`].
pub fn longest_opaque_run(runs: &[Run<1>]) -> Option<core::ops::Range<i32>> {
    let mut column = 0;
    let mut longest: Option<core::ops::Range<i32>> = None;
    for run in runs {
        let range = column..run.end as i32;
        column = run.end as i32;
        if run.coverage[0] == 255 && longest.as_ref().is_none_or(|l| l.len() < range.len()) {
            longest = Some(range);
        }
    }
    longest
}

#[cfg(feature = "path")]
pub use masks::*;

#[cfg(feature = "path")]
mod masks {
    use super::super::RotationInfo;
    use super::PhysicalRect;
    use alloc::vec::Vec;
    use i_slint_core::graphics::{PathSink, ShapedRect};
    use i_slint_core::lengths::PhysicalPx;
    use zeno::{Command, Fill, Join, Mask, Stroke, Style};

    type Point = euclid::default::Point2D<f32>;

    /// The miter limit of the stroke that offsets a shape, as in Skia.
    const MITER_LIMIT: f32 = 4.;

    /// The farthest a point of a mask's path lies from the mask,
    /// which keeps zeno's 24.8 fixed-point coordinates from overflowing.
    const MAX_DISTANCE: f32 = (1 << 22) as f32;

    /// `a * b` for coverages from 0 to 255.
    pub fn multiply(a: u8, b: u8) -> u8 {
        ((a as u32 * b as u32 + 127) / 255) as u8
    }

    /// A [`ShapedRect`] at `origin` in unrotated physical coordinates, as drawn on the screen.
    pub struct ScreenShape {
        pub shape: ShapedRect<PhysicalPx>,
        pub origin: euclid::Vector2D<f32, PhysicalPx>,
        pub rotation: RotationInfo,
    }

    impl ScreenShape {
        fn to_screen(&self, p: Point) -> Point {
            self.rotation.transform_point(p + self.origin.to_untyped())
        }

        /// The screen area the shape may cover.
        pub fn bounds(&self) -> euclid::default::Box2D<f32> {
            // A miter reaches up to `MITER_LIMIT` times half the stroke width from its vertex.
            let grow = self.shape.offset.max(0.) * MITER_LIMIT;
            let rect = self.shape.rect.to_untyped().inflate(grow, grow);
            euclid::default::Box2D::from_points([rect.min(), rect.max()].map(|p| self.to_screen(p)))
        }

        /// The coverage of each pixel of `window`, row by row.
        pub fn mask(&self, window: &PhysicalRect) -> Vec<u8> {
            let to_window = window.origin.cast::<f32>().to_vector().to_untyped();
            let mut sink = Commands::new(|p| {
                let p = (self.to_screen(p) - to_window)
                    .clamp(Point::splat(-MAX_DISTANCE), Point::splat(MAX_DISTANCE));
                zeno::Point::new(p.x, p.y)
            });
            self.shape.add_outline(&mut sink);
            let commands = sink.commands;
            let (width, height) = (window.width().max(0) as u32, window.height().max(0) as u32);
            let render = |style: Style| {
                let mut mask = alloc::vec![0; (width * height) as usize];
                Mask::new(&commands).size(width, height).style(style).render_into(&mut mask, None);
                mask
            };
            let mut mask = render(Fill::NonZero.into());
            let offset = self.shape.offset;
            if offset == 0. {
                return mask;
            }
            let mut stroke = Stroke::new(2. * offset.abs());
            stroke.join(Join::Miter).miter_limit(MITER_LIMIT);
            let stroke = render(Style::Stroke(stroke));
            // Composited as if the two coverages were independent, so that a pixel the outline
            // and the stroke's edge both cross keeps its share.
            for (m, s) in mask.iter_mut().zip(stroke) {
                let overlap = multiply(*m, s);
                *m = if offset > 0. { *m + (s - overlap) } else { *m - overlap };
            }
            mask
        }
    }

    /// The pixels of `clip` that any of `shapes` may cover, or `None` if there are none.
    pub fn mask_window<'a>(
        shapes: impl IntoIterator<Item = &'a ScreenShape>,
        clip: &PhysicalRect,
    ) -> Option<PhysicalRect> {
        let bounds =
            shapes.into_iter().map(ScreenShape::bounds).reduce(|a, b| a.union(&b))?.round_out();
        let clip = clip.cast::<f32>().to_box2d().to_untyped();
        let window =
            euclid::default::Box2D::new(bounds.min.max(clip.min), bounds.max.min(clip.max));
        (!window.is_empty()).then(|| PhysicalRect::from_untyped(&window.to_rect().cast()))
    }

    /// The length of the lines that [`Commands`] adds at the ends of a cubic.
    const TANGENT_LINE: f32 = 1. / 64.;

    /// The farthest the lines that replace a cubic stray from it, as for the outline's cubics.
    const FLATNESS: f32 = 0.05;

    /// Halving a cubic more often than this stops, bounding the cost of a huge one.
    const MAX_DEPTH: u32 = 16;

    /// Builds zeno commands from lines only, mapping each point with `map`.
    ///
    /// zeno flattens a curve to within half a pixel, which visibly shrinks a shape,
    /// so cubics are flattened here instead.
    /// zeno also bevels any join that turns by more than a right angle.
    /// The end of a flattened cubic can turn slightly more than the cubic at a right-angled corner,
    /// so each cubic starts and ends with a short line along its exact tangent.
    struct Commands<F> {
        commands: Vec<Command>,
        current: zeno::Point,
        map: F,
    }

    impl<F: Fn(Point) -> zeno::Point> Commands<F> {
        fn new(map: F) -> Self {
            Self { commands: Vec::new(), current: zeno::Point::ZERO, map }
        }

        fn line_to(&mut self, to: zeno::Point) {
            self.commands.push(Command::LineTo(to));
            self.current = to;
        }

        /// Appends lines from the current point along the cubic to `to`.
        fn flatten(&mut self, ctrl1: zeno::Point, ctrl2: zeno::Point, to: zeno::Point, depth: u32) {
            let from = self.current;
            let chord = to - from;
            let length = chord.length();
            // The cubic lies within the hull of its control points.
            let distance = |p: zeno::Point| {
                let v = p - from;
                if length > 0. {
                    (v.x * chord.y - v.y * chord.x).abs() / length
                } else {
                    v.length()
                }
            };
            if depth >= MAX_DEPTH || distance(ctrl1).max(distance(ctrl2)) <= FLATNESS {
                self.line_to(to);
                return;
            }
            let mid = |a: zeno::Point, b: zeno::Point| (a + b) * 0.5;
            let (ab, bc, cd) = (mid(from, ctrl1), mid(ctrl1, ctrl2), mid(ctrl2, to));
            let (abc, bcd) = (mid(ab, bc), mid(bc, cd));
            let middle = mid(abc, bcd);
            self.flatten(ab, abc, middle, depth + 1);
            self.flatten(bcd, cd, to, depth + 1);
        }
    }

    /// The point `TANGENT_LINE` from `from` toward the first of `towards` that differs from it.
    fn along_tangent(from: zeno::Point, towards: [zeno::Point; 3]) -> zeno::Point {
        let Some(direction) = towards.iter().map(|&p| p - from).find(|d| d.x != 0. || d.y != 0.)
        else {
            return from;
        };
        from + direction.normalize() * TANGENT_LINE
    }

    impl<F: Fn(Point) -> zeno::Point> PathSink for Commands<F> {
        fn move_to(&mut self, to: Point) {
            let to = (self.map)(to);
            self.commands.push(Command::MoveTo(to));
            self.current = to;
        }
        fn line_to(&mut self, to: Point) {
            self.line_to((self.map)(to));
        }
        fn cubic_to(&mut self, ctrl1: Point, ctrl2: Point, to: Point) {
            let (ctrl1, ctrl2, to) = ((self.map)(ctrl1), (self.map)(ctrl2), (self.map)(to));
            self.line_to(along_tangent(self.current, [ctrl1, ctrl2, to]));
            let end = along_tangent(to, [ctrl2, ctrl1, self.current]);
            self.flatten(ctrl1, ctrl2, end, 0);
            self.line_to(to);
        }
        fn close(&mut self) {
            self.commands.push(Command::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PhysicalPoint;

    fn pixels<const N: usize>(coverage: &ShapeCoverage<N>) -> Vec<Vec<[u8; N]>> {
        let rect = coverage.rect;
        (rect.min_y()..rect.max_y())
            .map(|line| {
                let mut row = alloc::vec![[0; N]; rect.width() as usize];
                coverage.for_each_run(line, rect.min_x() as i32, row.len(), |range, c| {
                    row[range].fill(c)
                });
                row
            })
            .collect()
    }

    #[test]
    fn coverage_shrinks_to_the_covered_pixels() {
        #[rustfmt::skip]
        let mask = [
            0, 0, 0, 0, 0,
            0, 0, 9, 0, 0,
            0, 255, 255, 255, 0,
            0, 255, 255, 255, 0,
            0, 0, 0, 0, 0,
        ];
        let rect = PhysicalRect::new(PhysicalPoint::new(10, 20), euclid::size2(5, 5));
        let coverage = ShapeCoverage::new(rect, [&mask], |m| m).unwrap();
        assert_eq!(
            coverage.rect,
            PhysicalRect::new(PhysicalPoint::new(11, 21), euclid::size2(3, 3))
        );
        assert_eq!(
            coverage.opaque(),
            PhysicalRect::new(PhysicalPoint::new(11, 22), euclid::size2(3, 2))
        );
        assert_eq!(pixels(&coverage), [[[0], [9], [0]], [[255]; 3], [[255]; 3]]);
        assert!(ShapeCoverage::new(rect, [&mask], |_| [0]).is_none());
    }

    #[test]
    fn runs_from_a_column() {
        let mut mask = [0; 10];
        mask[2..6].fill(255);
        mask[6] = 128;
        let rect = PhysicalRect::new(PhysicalPoint::new(0, 0), euclid::size2(10, 1));
        let coverage = ShapeCoverage::new(rect, [&mask], |m| m).unwrap();
        let mut runs = Vec::new();
        coverage.for_each_run(0, 3, 4, |range, [c]| runs.push((range, c)));
        assert_eq!(runs, [(0..3, 255), (3..4, 128)]);
        let row = coverage.row(0).unwrap();
        assert_eq!(coverage_at(row, 4), 128);
        assert_eq!(covered_columns(row), Some(0..5));
        assert_eq!(longest_opaque_run(row), Some(0..4));
    }

    #[cfg(feature = "path")]
    mod masks {
        use super::super::super::{RenderingRotation, RotationInfo};
        use super::*;
        use i_slint_core::graphics::{BorderRadius, CornerShape, CornerShapes, ShapedRect};

        fn square(offset: f32) -> ScreenShape {
            ScreenShape {
                shape: ShapedRect::new(
                    euclid::rect(5.5, 6.5, 10., 12.),
                    BorderRadius::new_uniform(3.),
                    CornerShapes::new_uniform(CornerShape::Square),
                    offset,
                ),
                origin: euclid::vec2(2., 1.),
                rotation: RotationInfo {
                    orientation: RenderingRotation::NoRotation,
                    screen_size: euclid::size2(40, 40),
                },
            }
        }

        fn area(mask: &[u8]) -> f32 {
            mask.iter().map(|&c| c as f32 / 255.).sum()
        }

        #[test]
        fn offset_grows_and_shrinks_the_area() {
            let clip = PhysicalRect::new(PhysicalPoint::new(0, 0), euclid::size2(40, 40));
            for (offset, expected) in [(0., 120.), (2.25, 14.5 * 16.5), (-2.25, 5.5 * 7.5)] {
                let shape = square(offset);
                let window = mask_window([&shape], &clip).unwrap();
                let area = area(&shape.mask(&window));
                assert!((area - expected).abs() < 0.5, "offset {offset}: {area} vs {expected}");
            }
        }

        #[test]
        fn mask_does_not_depend_on_the_window() {
            let mut shape = square(1.7);
            shape.shape.shape = CornerShapes::new(
                CornerShape::Notch,
                CornerShape::Scoop,
                CornerShape::Superellipse(-0.6),
                CornerShape::Bevel,
            );
            let clip = PhysicalRect::new(PhysicalPoint::new(0, 0), euclid::size2(40, 40));
            let window = mask_window([&shape], &clip).unwrap();
            let full = shape.mask(&window);
            let part = PhysicalRect::new(PhysicalPoint::new(6, 9), euclid::size2(7, 5));
            let expected: Vec<u8> = (part.min_y()..part.max_y())
                .flat_map(|y| {
                    let start = ((y - window.min_y()) * window.width() + part.min_x()
                        - window.min_x()) as usize;
                    full[start..start + part.width() as usize].iter().copied()
                })
                .collect();
            assert_eq!(shape.mask(&part), expected);
        }
    }
}
