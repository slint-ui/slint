// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The anti-aliased coverage of the outlines of rectangles whose corners have a
//! [`CornerShape`](i_slint_core::graphics::CornerShape).

use super::{PhysicalPoint, PhysicalRect};
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

pub type Point = euclid::default::Point2D<f32>;

/// Pixels of a row whose coverage is the same in each of `N` layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run<const N: usize> {
    /// The column after the run, relative to [`ShapeCoverage::rect`].
    end: u16,
    /// From 0 to 255.
    pub coverage: [u8; N],
}

/// How much of each pixel `N` layers of polygons cover, row by row, as runs of pixels with the
/// same coverage.
#[derive(Debug)]
pub struct ShapeCoverage<const N: usize> {
    /// The pixels the coverage is known for, in screen coordinates: the polygons' bounds
    /// within the clip they were rasterized for.
    pub rect: PhysicalRect,
    /// Pixels that every layer covers fully.
    pub opaque: PhysicalRect,
    /// The first row of each group of identical rows, relative to `rect`, and the start of its
    /// runs in `runs`.
    rows: Vec<(u16, u32)>,
    runs: Vec<Run<N>>,
}

/// A polygon edge that isn't horizontal, relative to [`ShapeCoverage::rect`].
struct Edge {
    top: Point,
    bottom: Point,
    /// 1 if the polygon runs down along the edge, -1 if it runs up.
    sign: f32,
    layer: u8,
    /// The order of the edge in the polygons, which orders the cells of a pixel.
    index: u32,
}

/// A change of the coverage of one layer, from column `x` on.
struct Cell {
    x: u16,
    layer: u8,
    edge: u32,
    delta: f32,
}

impl<const N: usize> ShapeCoverage<N> {
    /// Rasterizes `layers` of disjoint polygons, in screen coordinates, within `clip`.
    /// `map` turns the share of a pixel that each layer covers, from 0 to 1, into the stored
    /// coverage.
    /// Returns `None` if the polygons have no area within `clip`.
    pub fn new<const L: usize>(
        layers: [&[Vec<Point>]; L],
        clip: &PhysicalRect,
        map: impl Fn([f32; L]) -> [u8; N],
    ) -> Option<Self> {
        let bounds = euclid::default::Box2D::from_points(
            layers.iter().flat_map(|layer| layer.iter().flatten()),
        )
        .round_out();
        if bounds.is_empty() {
            return None;
        }
        let limit = |v: f32| v.clamp(i16::MIN as f32, i16::MAX as f32) as i32;
        let left = limit(bounds.min.x).max(clip.min_x() as i32);
        let right = limit(bounds.max.x).min(clip.max_x() as i32);
        let top = limit(bounds.min.y).max(clip.min_y() as i32);
        let bottom = limit(bounds.max.y).min(clip.max_y() as i32);
        let width = right - left;
        if top >= bottom || width <= 0 || width > i16::MAX as i32 {
            return None;
        }
        let height = bottom - top;

        let origin = euclid::default::Vector2D::new(left as f32, top as f32);
        let mut edges = Vec::new();
        for (layer, polygons) in layers.iter().enumerate() {
            for polygon in polygons.iter() {
                let Some(&last) = polygon.last() else { continue };
                let mut from = last - origin;
                for &to in polygon {
                    let to = to - origin;
                    let (top, bottom, sign) =
                        if from.y < to.y { (from, to, 1.) } else { (to, from, -1.) };
                    if top.y < bottom.y && bottom.y > 0. && top.y < height as f32 {
                        let index = edges.len() as u32;
                        edges.push(Edge { top, bottom, sign, layer: layer as u8, index });
                    }
                    from = to;
                }
            }
        }
        edges.sort_unstable_by(|a, b| a.top.y.total_cmp(&b.top.y));

        let mut this = Self {
            rect: PhysicalRect::new(
                PhysicalPoint::new(left as i16, top as i16),
                euclid::size2(width as i16, height as i16),
            ),
            opaque: PhysicalRect::default(),
            rows: Vec::new(),
            runs: Vec::new(),
        };
        let mut pending = edges.iter().peekable();
        let mut active: Vec<&Edge> = Vec::new();
        let mut cells = Vec::new();
        let mut row = 0;
        while row < height {
            let y = row as f32;
            active.retain(|edge| edge.bottom.y > y);
            while let Some(edge) = pending.next_if(|edge| edge.top.y < y + 1.) {
                active.push(edge);
            }
            cells.clear();
            for edge in &active {
                add_cells(&mut cells, edge, y, width);
            }
            // By edge within a pixel, so that the sums don't depend on the clip.
            cells.sort_unstable_by_key(|cell| (cell.x, cell.edge));

            let start = this.runs.len();
            let push = |runs: &mut Vec<Run<N>>, end: u16, coverage: [u8; N]| {
                let in_row = runs.len() > start;
                match runs.last_mut() {
                    Some(last) if in_row && last.coverage == coverage => last.end = end,
                    _ if end > 0 => runs.push(Run { end, coverage }),
                    _ => {}
                }
            };
            let mut sums = [0f32; L];
            let mut coverage = map(sums);
            let mut x = 0;
            for (i, cell) in cells.iter().enumerate() {
                if cell.x > x {
                    push(&mut this.runs, cell.x, coverage);
                    x = cell.x;
                }
                sums[cell.layer as usize] += cell.delta;
                if cells.get(i + 1).is_none_or(|next| next.x != cell.x) {
                    coverage = map(sums.map(|sum| sum.abs().min(1.)));
                }
            }
            push(&mut this.runs, width as u16, coverage);

            let previous = this.rows.last().map(|&(_, start)| start as usize);
            if previous.is_some_and(|previous| this.runs[previous..start] == this.runs[start..]) {
                this.runs.truncate(start);
            } else {
                this.rows.push((row as u16, start as u32));
            }

            row += 1;
            // Rows that only vertical edges cross, none of them starting or ending, are the
            // same as this one.
            if active.iter().all(|edge| edge.top.x == edge.bottom.x && edge.top.y <= y) {
                let next = active
                    .iter()
                    .map(|edge| edge.bottom.y)
                    .chain(pending.peek().map(|edge| edge.top.y))
                    .fold(height as f32, f32::min);
                row = row.max(next as i32);
            }
        }
        let mut opaque_rows: (core::ops::Range<u16>, core::ops::Range<u16>) = (0..0, 0..0);
        // The largest group of identical rows tends to be the one between the corners.
        for (i, &(first, start)) in this.rows.iter().enumerate() {
            let (end_row, end) = this
                .rows
                .get(i + 1)
                .map_or((height as u16, this.runs.len()), |&(row, start)| (row, start as usize));
            let mut column = 0;
            for run in &this.runs[start as usize..end] {
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
        this.opaque = PhysicalRect::new(
            this.rect.origin + euclid::vec2(columns.start as i16, rows.start as i16),
            euclid::size2(columns.len() as i16, rows.len() as i16),
        );
        Some(this)
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

    /// Calls `f` with each range of the `len` pixels of `line` from screen column `x` that have
    /// the same coverage, as indices from `x`.
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

/// A share of a pixel, from 0 to 1, as a coverage from 0 to 255.
pub fn to_u8(share: f32) -> u8 {
    (share.clamp(0., 1.) * 255. + 0.5) as u8
}

/// The coverage of a single layer of `runs` at `column`, relative to [`ShapeCoverage::rect`].
pub fn coverage_at(runs: &[Run<1>], column: i32) -> u8 {
    runs.get(runs.partition_point(|run| (run.end as i32) <= column)).map_or(0, |r| r.coverage[0])
}

/// The columns from the first covered pixel of `runs` to the last, relative to
/// [`ShapeCoverage::rect`].
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

/// The longest run of `runs` that covers its pixels fully, as columns relative to
/// [`ShapeCoverage::rect`].
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

/// Adds the cells of `edge` in the row starting at `y`, in a rasterized rect `width` pixels
/// wide.
fn add_cells(cells: &mut Vec<Cell>, edge: &Edge, y: f32, width: i32) {
    let (top, bottom) = (edge.top, edge.bottom);
    let (y0, y1) = (top.y.max(y), bottom.y.min(y + 1.));
    if y1 <= y0 {
        return;
    }
    let x_at = |y: f32| top.x + (bottom.x - top.x) * (y - top.y) / (bottom.y - top.y);
    let (x0, x1) = (x_at(y0), x_at(y1));
    let (lo, hi) = (x0.min(x1).clamp(0., width as f32), x0.max(x1).clamp(0., width as f32));
    let dy = (y1 - y0) * edge.sign;
    // Each pixel gets the share of the edge's height that lies left of it, so a sum over
    // a row's cells is the area that its edges enclose.
    let mut push = |x: i32, delta: f32| {
        if delta != 0. && x < width {
            cells.push(Cell { x: x as u16, layer: edge.layer, edge: edge.index, delta });
        }
    };
    let (first, last) = (lo.floor() as i32, hi.floor() as i32);
    let mut previous = 0.;
    for x in first..=last {
        let share = dy * share_right_of(x as f32, lo, hi);
        push(x, share - previous);
        previous = share;
    }
    push(last + 1, dy - previous);
}

/// The share of the pixel column starting at `column` right of a line running from `lo` to
/// `hi` over the pixel's height.
fn share_right_of(column: f32, lo: f32, hi: f32) -> f32 {
    let right = column + 1.;
    if hi - lo < 1e-6 {
        return (right - lo).clamp(0., 1.);
    }
    // Integrate `clamp(right - x, 0, 1)` over `lo..hi`: 1 left of the column, falling
    // linearly across it, 0 right of it.
    let full = (hi.min(column) - lo).max(0.);
    let (a, b) = (lo.max(column), hi.min(right));
    let partial = if b > a { (b - a) * (right - (a + b) / 2.) } else { 0. };
    (full + partial) / (hi - lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polygon(points: &[(f32, f32)]) -> Vec<Point> {
        points.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

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
    fn share_right_of_a_line() {
        // A vertical line through the middle of the pixel.
        assert_eq!(share_right_of(2., 2.5, 2.5), 0.5);
        // Left and right of the pixel.
        assert_eq!(share_right_of(2., 1., 1.), 1.);
        assert_eq!(share_right_of(2., 3.5, 3.5), 0.);
        // A diagonal across the pixel leaves half of it on either side.
        assert!((share_right_of(2., 2., 3.) - 0.5).abs() < 1e-6);
        // A diagonal across two pixels leaves a quarter of the left one right of it, and
        // three quarters of the right one.
        assert!((share_right_of(2., 2., 4.) - 0.25).abs() < 1e-6);
        assert!((share_right_of(3., 2., 4.) - 0.75).abs() < 1e-6);
        // A nearly vertical line far from the origin.
        assert!((share_right_of(40., 40.0001, 40.00015) - 0.99988).abs() < 1e-4);
    }

    #[test]
    fn coverage_is_the_area() {
        let clip = PhysicalRect::new(PhysicalPoint::new(-100, -100), euclid::size2(400, 400));
        // A triangle with a fractional corner, wound either way.
        let triangle = [(1.5, 2.25), (17.25, 2.25), (1.5, 20.)];
        for points in [triangle.to_vec(), triangle.iter().rev().copied().collect()] {
            let coverage =
                ShapeCoverage::new([&[polygon(&points)]], &clip, |[c]| [(c * 255.).round() as u8])
                    .unwrap();
            assert_eq!(
                coverage.rect,
                PhysicalRect::new(PhysicalPoint::new(1, 2), euclid::size2(17, 18))
            );
            let area: f32 = pixels(&coverage).iter().flatten().map(|[c]| *c as f32 / 255.).sum();
            let expected = 15.75 * 17.75 / 2.;
            assert!((area - expected).abs() < 0.1, "{area} vs {expected}");
        }
    }

    #[test]
    fn pixels_outside_the_clip_are_skipped_without_changing_the_others() {
        let square = polygon(&[(0.5, 0.5), (30.3, 0.5), (30.3, 30.7), (0.5, 30.7)]);
        let diamond = polygon(&[(15.4, 0.5), (30.3, 15.6), (15.4, 30.7), (0.5, 15.6)]);
        let layers: [&[Vec<Point>]; 2] = [core::slice::from_ref(&square), &[diamond]];
        let map = |c: [f32; 2]| c.map(|c| (c * 255.).round() as u8);
        let full = PhysicalRect::new(PhysicalPoint::new(0, 0), euclid::size2(40, 40));
        let whole = ShapeCoverage::new(layers, &full, map).unwrap();
        let part = PhysicalRect::new(PhysicalPoint::new(10, 7), euclid::size2(5, 9));
        let clipped = ShapeCoverage::new(layers, &part, map).unwrap();
        assert_eq!(clipped.rect, part);
        let whole_part: Vec<_> =
            pixels(&whole)[7..16].iter().map(|row| row[10..15].to_vec()).collect();
        assert_eq!(whole_part, pixels(&clipped));
        let square = ShapeCoverage::new([&[square]], &full, |[c]| [(c * 255.).round() as u8]);
        assert_eq!(
            square.unwrap().opaque,
            PhysicalRect::new(PhysicalPoint::new(1, 1), euclid::size2(29, 29))
        );
    }

    #[test]
    fn polygons_wider_than_the_coordinates_are_rasterized_within_the_clip() {
        let wide = polygon(&[(-40000., 2.), (40000., 2.), (40000., 8.), (-40000., 8.)]);
        let clip = PhysicalRect::new(PhysicalPoint::new(0, 0), euclid::size2(20, 20));
        let coverage =
            ShapeCoverage::new([&[wide]], &clip, |[c]| [(c * 255.).round() as u8]).unwrap();
        assert_eq!(
            coverage.rect,
            PhysicalRect::new(PhysicalPoint::new(0, 2), euclid::size2(20, 6))
        );
        assert_eq!(coverage.opaque, coverage.rect);
    }

    #[test]
    fn runs_from_a_column() {
        let square = polygon(&[(2., 0.), (6.5, 0.), (6.5, 1.), (2., 1.)]);
        let clip = PhysicalRect::new(PhysicalPoint::new(0, 0), euclid::size2(10, 10));
        let coverage =
            ShapeCoverage::new([&[square]], &clip, |[c]| [(c * 255.).round() as u8]).unwrap();
        let mut runs = Vec::new();
        coverage.for_each_run(0, 3, 4, |range, [c]| runs.push((range, c)));
        assert_eq!(runs, [(0..3, 255), (3..4, 128)]);
        let row = coverage.row(0).unwrap();
        assert_eq!(coverage_at(row, 4), 128);
        assert_eq!(covered_columns(row), Some(0..5));
        assert_eq!(longest_opaque_run(row), Some(0..4));
    }
}
