// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore ilog, underflows, hypot

/*!
The outline of rectangle corners with a [`CornerShape`], shared by every renderer.

Corners are computed in the *corner frame*: the corner's outer tip at the origin, `x` running
along the incoming edge and `y` along the outgoing one, so the rectangle lies in `x, y >= 0`.
Every corner shape is symmetric about the frame's diagonal.
*/

use super::border_radius::{BorderRadius, CornerShape, CornerShapes};
use alloc::vec;
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

type Point = euclid::default::Point2D<f32>;
type Vector = euclid::default::Vector2D<f32>;

/// Larger `|k|` is clamped to this: `2^16` is indistinguishable from `Square` or `Notch`,
/// and an unclamped `n = 2^|k|` overflows.
const MAX_SUPERELLIPSE_K: f32 = 16.0;

/// Largest distance, in the units of the input rect, between a flattened curve and the
/// true one.
pub(crate) const TOLERANCE: f32 = 0.05;

/// Most a flattened curve turns, in radians, between two segments.
const MAX_TURN: f32 = 0.3;

/// Most segments a flattened corner, or samples along an offset polyline, take, bounding the
/// cost of a huge radius.
const MAX_SEGMENTS: u32 = 1024;

/// Deepest a superellipse span is halved while flattening, so that a corner stays within
/// [`MAX_SEGMENTS`].
const MAX_SUPERELLIPSE_DEPTH: u32 = MAX_SEGMENTS.ilog2();

/// The superellipse exponent `n = 2^|k|`.
///
/// `k`'s sign selects the convex or the concave branch, not the exponent.
/// See [`superellipse_at`].
pub(crate) fn superellipse_n(k: f32) -> f32 {
    2f32.powf(k.abs().min(MAX_SUPERELLIPSE_K))
}

/// `(p, q)` on the ray of slope `(1 - s) / s`, scaled onto the curve so `p^n + q^n == 1`.
fn superellipse_pq(n: f32, s: f32) -> (f32, f32) {
    // Factoring out the larger keeps the power in `0..=1`, so a large `n` can't overflow it.
    let (larger, smaller) = (s.max(1. - s), s.min(1. - s));
    let scale = larger * (1. + (smaller / larger).powf(n)).powf(1. / n);
    (s / scale, (1. - s) / scale)
}

/// Places `(p, q)` in the corner frame, in units of the radius.
/// A negative `k` is the convex curve mirrored about the bevel chord.
fn superellipse_corner(k: f32, p: f32, q: f32) -> Vector {
    if k < 0. { Vector::new(q, p) } else { Vector::new(1. - p, 1. - q) }
}

/// A point on the `superellipse(k)` corner and the unit tangent there, in the corner frame
/// and in units of the radius.
///
/// Positive `k` gives the convex branch, `(1 - x)^n + (1 - y)^n = 1`, and negative `k` the
/// concave one, `x^n + y^n = 1`, so that `superellipse(-1)` is `Scoop`.
/// `s` runs from 0 at the incoming edge to 1 at the outgoing one, placing the point where the
/// ray of slope `(1 - s) / s` crosses the curve.
/// The CSS spec parametrizes by `t` → `(1 - t^(1/n), 1 - (1-t)^(1/n))` instead, which underflows
/// to 0 across most of the curve for a large `n`.
pub(crate) fn superellipse_at(k: f32, n: f32, s: f32) -> (Vector, Vector) {
    let (p, q) = superellipse_pq(n, s);
    // Perpendicular to the implicit gradient, which stays finite where the slope doesn't.
    let (gp, gq) = (p.powf(n - 1.), q.powf(n - 1.));
    let tangent = if k < 0. { Vector::new(-gp, gq) } else { Vector::new(-gq, gp) };
    (superellipse_corner(k, p, q), tangent.normalize())
}

/// Appends the corner of `shape` and radius `r` to `out`, as a polyline in the corner frame
/// from `(r, 0)` to `(0, r)`, turning at most `max_turn` between two segments of a curve.
fn flatten_corner(shape: CornerShape, r: f32, max_turn: f32, out: &mut Vec<Point>) {
    if r <= 0. {
        out.push(Point::zero());
        return;
    }
    match shape {
        CornerShape::Square => out.push(Point::zero()),
        CornerShape::Notch => out.extend([Point::new(r, 0.), Point::new(r, r), Point::new(0., r)]),
        CornerShape::Bevel => out.extend([Point::new(r, 0.), Point::new(0., r)]),
        CornerShape::Round => {
            out.push(Point::new(r, 0.));
            flatten_arc(out, Point::new(r, r), Point::new(0., r), max_turn);
        }
        CornerShape::Scoop => {
            out.push(Point::new(r, 0.));
            flatten_arc(out, Point::zero(), Point::new(0., r), max_turn);
        }
        CornerShape::Squircle | CornerShape::Superellipse(_) => {
            let k = shape.superellipse_k();
            let n = superellipse_n(k);
            let start = superellipse_at(k, n, 0.);
            out.push((start.0 * r).to_point());
            flatten_superellipse_span(
                out,
                (k, n, r, max_turn.cos()),
                (0., start),
                (1., superellipse_at(k, n, 1.)),
                MAX_SUPERELLIPSE_DEPTH,
            );
        }
    }
}

/// Appends the arc from the last point of `out` around `center` to `to`, turning less than
/// half a turn, within [`TOLERANCE`] and turning at most `max_turn` between two segments.
fn flatten_arc(out: &mut Vec<Point>, center: Point, to: Point, max_turn: f32) {
    let (start, end) = (*out.last().unwrap() - center, to - center);
    let radius = start.length();
    let sweep = start.cross(end).atan2(start.dot(end));
    let chord_turn = 2. * (1. - (TOLERANCE / radius).min(1.)).acos();
    let steps =
        (sweep.abs() / max_turn.min(chord_turn)).ceil().clamp(1., MAX_SEGMENTS as f32) as u32;
    out.extend((1..steps).map(|i| {
        let (sin, cos) = (sweep * i as f32 / steps as f32).sin_cos();
        center + Vector::new(start.x * cos - start.y * sin, start.x * sin + start.y * cos)
    }));
    out.push(to);
}

/// Appends the superellipse between `s0` and `s1`, excluding `s0` itself, halving the span
/// until it's within [`TOLERANCE`] and turns no more than `min_turn_cos`.
fn flatten_superellipse_span(
    out: &mut Vec<Point>,
    (k, n, r, min_turn_cos): (f32, f32, f32, f32),
    (s0, (p0, t0)): (f32, (Vector, Vector)),
    (s1, (p1, t1)): (f32, (Vector, Vector)),
    depth: u32,
) {
    let s = (s0 + s1) * 0.5;
    let middle = superellipse_at(k, n, s);
    let (start, mid, end) = ((p0 * r).to_point(), (middle.0 * r).to_point(), (p1 * r).to_point());
    let chord = end - start;
    // The curve is convex, so each half of the span lies within the triangle of its chord
    // and its end tangents, and the triangles' tips bound how far it strays from `chord`.
    // The curve's farthest point is rarely at the middle `s`.
    // Each height is compared squared and scaled by `chord`'s length and the tangents'
    // cross product, which avoids a square root and two divisions per span.
    let limit = TOLERANCE * TOLERANCE * chord.square_length();
    let height = |p: Point| chord.cross(p - start);
    let within = |height: f32, scale: f32| height * height <= limit * scale * scale;
    let tip_within = |p: Point, t: Vector, q: Point, u: Vector| {
        let turn = t.cross(u);
        if turn.abs() > 1e-6 {
            within(height(p) * turn + chord.cross(t) * (q - p).cross(u), turn)
        } else {
            within(height(q), 1.)
        }
    };
    let flat = within(height(mid), 1.)
        && tip_within(start, t0, mid, middle.1)
        && tip_within(mid, middle.1, end, t1);
    if depth == 0 || (t0.dot(t1) >= min_turn_cos && flat) {
        out.push((p1 * r).to_point());
    } else {
        let params = (k, n, r, min_turn_cos);
        flatten_superellipse_span(out, params, (s0, (p0, t0)), (s, middle), depth - 1);
        flatten_superellipse_span(out, params, (s, middle), (s1, (p1, t1)), depth - 1);
    }
}

/// The corner of `shape` and radius `r` in the corner frame, from `(r, 0)` to `(0, r)`,
/// starting with a [`ContourVertex::Line`].
fn corner_edge(shape: CornerShape, r: f32) -> Vec<ContourVertex> {
    use ContourVertex::{Arc, Line};
    if r <= 0. {
        return vec![Line(Point::zero())];
    }
    match shape {
        CornerShape::Round => {
            vec![Line(Point::new(r, 0.)), Arc { center: Point::new(r, r), to: Point::new(0., r) }]
        }
        CornerShape::Scoop => {
            vec![Line(Point::new(r, 0.)), Arc { center: Point::zero(), to: Point::new(0., r) }]
        }
        _ => {
            let mut points = Vec::new();
            flatten_corner(shape, r, MAX_TURN, &mut points);
            points.into_iter().map(Line).collect()
        }
    }
}

/// The corners of a rectangle in path order: tip, incoming edge direction, outgoing edge
/// direction.
fn corner_frames(rect: euclid::default::Rect<f32>) -> [(Point, Vector, Vector); 4] {
    let (x0, y0, x1, y1) = (rect.min_x(), rect.min_y(), rect.max_x(), rect.max_y());
    [
        (Point::new(x1, y0), Vector::new(-1., 0.), Vector::new(0., 1.)),
        (Point::new(x1, y1), Vector::new(0., -1.), Vector::new(-1., 0.)),
        (Point::new(x0, y1), Vector::new(1., 0.), Vector::new(0., -1.)),
        (Point::new(x0, y0), Vector::new(0., 1.), Vector::new(1., 0.)),
    ]
}

/// A rectangle's radii and shapes in the order of [`corner_frames`].
fn corners_in_path_order<U>(
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
) -> [(f32, CornerShape); 4] {
    [
        (radius.top_right, shapes.top_right()),
        (radius.bottom_right, shapes.bottom_right()),
        (radius.bottom_left, shapes.bottom_left()),
        (radius.top_left, shapes.top_left()),
    ]
}

/// `radius` for `rect` with corners of `shapes`: a [`CornerShape::Square`] corner has no
/// radius, and where corners would overlap, all radii are scaled down by one shared factor.
/// Adjacent corners are fitted along each side, as for a rounded rectangle, and opposite
/// corners so that their inner corner hulls don't intersect.
/// See <https://drafts.csswg.org/css-backgrounds-3/#corner-overlap> and
/// <https://drafts.csswg.org/css-borders-4/#corner-shape-constrain-radii>.
fn fit_radius<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
) -> BorderRadius<f32, U> {
    let corner =
        |r: f32, shape: CornerShape| if shape == CornerShape::Square { 0. } else { r.max(0.) };
    let r = BorderRadius::<f32, U>::new(
        corner(radius.top_left, shapes.top_left()),
        corner(radius.top_right, shapes.top_right()),
        corner(radius.bottom_right, shapes.bottom_right()),
        corner(radius.bottom_left, shapes.bottom_left()),
    );
    let (width, height) = (rect.width().max(0.), rect.height().max(0.));
    let adjacent = [
        (width, r.top_left + r.top_right),
        (width, r.bottom_left + r.bottom_right),
        (height, r.top_left + r.bottom_left),
        (height, r.top_right + r.bottom_right),
    ]
    .into_iter()
    .filter(|&(_, sum)| sum > 0.)
    .fold(1f32, |scale, (side, sum)| scale.min(side / sum));
    r * adjacent.min(opposite_corner_scale(rect, corners_in_path_order(r, shapes)))
}

/// The CSS normalized inner corner hull of `shape`: a convex polygon in the corner frame, in
/// units of the radius, that holds the corner's curve, starting at the tip.
/// A convex corner's hull is the triangle of the tip and the corner's ends.
/// A concave one also reaches toward the corner's center, up to the tangent at the middle of
/// its curve.
/// Repeating the last point pads the triangle to the pentagon's length.
fn corner_hull(shape: CornerShape) -> [Vector; 5] {
    let k = shape.superellipse_k();
    let (tip, ends) = (Vector::zero(), [Vector::new(0., 1.), Vector::new(1., 0.)]);
    if k >= 0. {
        return [tip, ends[0], ends[1], ends[1], ends[1]];
    }
    // `1 - h` is where the curve crosses the frame's diagonal.
    let h = if k == f32::NEG_INFINITY { 0. } else { 1. - 0.5f32.powf(1. / superellipse_n(k)) };
    [tip, ends[0], Vector::new(1. - 2. * h, 1.), Vector::new(1., 1. - 2. * h), ends[1]]
}

/// The largest factor by which opposite corners of `corners`, in the order of
/// [`corner_frames`], can scale their radii before their [`corner_hull`]s intersect,
/// or infinity when no factor makes them.
fn opposite_corner_scale(
    rect: euclid::default::Rect<f32>,
    corners: [(f32, CornerShape); 4],
) -> f32 {
    // Convex hulls are triangles, which stay apart within the adjacent corner scale.
    if corners.iter().all(|&(_, shape)| shape.superellipse_k() >= 0.) {
        return f32::INFINITY;
    }
    let frames = corner_frames(rect);
    // Each hull relative to its tip, so scaling the radius scales the offsets.
    let hull = |i: usize| {
        let ((tip, e_in, e_out), (r, shape)) = (frames[i], corners[i]);
        (tip, corner_hull(shape).map(|p| e_in * (p.x * r) + e_out * (p.y * r)))
    };
    let normals = |offsets: [Vector; 5]| {
        (0..5).filter_map(move |i| {
            let edge = offsets[(i + 1) % 5] - offsets[i];
            (edge.square_length() > 0.).then(|| Vector::new(edge.y, -edge.x))
        })
    };
    let extent = |offsets: &[Vector; 5], axis: Vector| {
        offsets
            .iter()
            .map(|p| p.dot(axis))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), d| (lo.min(d), hi.max(d)))
    };
    [(0, 2), (1, 3)]
        .into_iter()
        .map(|(a, b)| {
            let ((tip_a, hull_a), (tip_b, hull_b)) = (hull(a), hull(b));
            // Separating axes: the hulls are convex, and every hull contains its tip, so along
            // each axis an interval reaching past the tip grows with the scale, and the
            // largest scale that keeps one axis separating is the answer.
            normals(hull_a)
                .chain(normals(hull_b))
                .flat_map(|axis| {
                    let gap = (tip_b - tip_a).dot(axis);
                    let ((min_a, max_a), (min_b, max_b)) =
                        (extent(&hull_a, axis), extent(&hull_b, axis));
                    [(max_a - min_b, gap), (max_b - min_a, -gap)]
                })
                .map(|(reach, gap)| {
                    if reach > 0. {
                        gap / reach
                    } else if gap >= 0. {
                        f32::INFINITY
                    } else {
                        0.
                    }
                })
                .fold(f32::NEG_INFINITY, f32::max)
        })
        // A hull without edges is a point, and no scale makes it intersect.
        .map(|scale| if scale == f32::NEG_INFINITY { f32::INFINITY } else { scale })
        .fold(f32::INFINITY, f32::min)
}

/// A vertex of a [`Contour::Curved`] outline: the end of the segment from the previous
/// vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContourVertex {
    /// The end of a straight segment.
    Line(Point),
    /// The end of a circular arc, turning less than half a turn.
    Arc {
        /// The arc's center.
        center: Point,
        /// The arc's end.
        to: Point,
    },
}

impl ContourVertex {
    /// Where the segment ends.
    pub fn end(self) -> Point {
        match self {
            Self::Line(p) | Self::Arc { to: p, .. } => p,
        }
    }

    fn map(self, f: impl Fn(Point) -> Point) -> Self {
        match self {
            Self::Line(p) => Self::Line(f(p)),
            Self::Arc { center, to } => Self::Arc { center: f(center), to: f(to) },
        }
    }
}

/// An outline from [`outline`].
#[derive(Debug, Clone)]
pub enum Contour {
    /// A single closed, clockwise polygon that keeps its circular arcs, starting with a
    /// [`ContourVertex::Line`].
    Curved(Vec<ContourVertex>),
    /// Disjoint, clockwise polygons, none when the outline leaves no area.
    Polygons(Vec<Vec<Point>>),
}

impl Contour {
    /// The outline as disjoint, clockwise polygons, with its arcs flattened.
    pub fn into_polygons(self) -> Vec<Vec<Point>> {
        match self {
            Self::Curved(vertices) => {
                let mut polygon = Vec::with_capacity(vertices.len());
                extend_flattened(&mut polygon, vertices);
                vec![polygon]
            }
            Self::Polygons(polygons) => polygons,
        }
    }
}

/// Appends `vertices` to `out` as a polyline, flattening their arcs.
fn extend_flattened(out: &mut Vec<Point>, vertices: impl IntoIterator<Item = ContourVertex>) {
    for vertex in vertices {
        match vertex {
            ContourVertex::Line(p) => push_distinct(out, p),
            ContourVertex::Arc { center, to } => flatten_arc(out, center, to, MAX_TURN),
        }
    }
}

/// The outline of `rect` with corner radii `radius` and shapes `shapes`.
/// Where corners would overlap, the radii are scaled down as CSS specifies.
/// `Round` and `Scoop` corners stay arcs.
pub fn outline<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
) -> Contour {
    if rect.width() <= 0. || rect.height() <= 0. {
        return Contour::Polygons(Vec::new());
    }
    let radius = fit_radius(rect, radius, shapes);
    let mut corners = corners_in_path_order(radius, shapes).into_iter();
    let corners = corner_frames(rect).map(|(tip, e_in, e_out)| {
        let (r, shape) = corners.next().unwrap();
        corner_edge(shape, r)
            .into_iter()
            .map(|v| v.map(|p| tip + e_in * p.x + e_out * p.y))
            .collect::<Vec<_>>()
    });
    let mut vertices: Vec<ContourVertex> = Vec::with_capacity(corners.iter().map(Vec::len).sum());
    for v in corners.into_iter().flatten() {
        let repeats = |last: &ContourVertex| same_point(v.end(), last.end());
        if !matches!(v, ContourVertex::Line(_)) || !vertices.last().is_some_and(repeats) {
            vertices.push(v);
        }
    }
    Contour::Curved(vertices)
}

fn push_distinct(points: &mut Vec<Point>, p: Point) {
    if !points.last().is_some_and(|&last| same_point(p, last)) {
        points.push(p);
    }
}

fn same_point(a: Point, b: Point) -> bool {
    (a - b).square_length() <= 1e-8
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const SHAPES: [CornerShape; 14] = [
        CornerShape::Square,
        CornerShape::Notch,
        CornerShape::Bevel,
        CornerShape::Round,
        CornerShape::Scoop,
        CornerShape::Squircle,
        CornerShape::Superellipse(-6.),
        CornerShape::Superellipse(-2.),
        CornerShape::Superellipse(-0.5),
        CornerShape::Superellipse(0.),
        CornerShape::Superellipse(0.5),
        CornerShape::Superellipse(3.),
        CornerShape::Superellipse(8.),
        CornerShape::Superellipse(f32::INFINITY),
    ];

    fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
        let d = b - a;
        let t = if d.square_length() > 0. {
            ((p - a).dot(d) / d.square_length()).clamp(0., 1.)
        } else {
            0.
        };
        (p - (a + d * t)).length()
    }

    pub(crate) fn distance_to_polyline(p: Point, line: &[Point]) -> f32 {
        if line.len() == 1 {
            return (p - line[0]).length();
        }
        line.windows(2).map(|w| distance_to_segment(p, w[0], w[1])).fold(f32::INFINITY, f32::min)
    }

    /// Every flattened vertex lies on the curve, checked against the curve's own equation.
    #[test]
    fn flatten_corner_lies_on_the_curve() {
        for shape in SHAPES {
            for r in [3f32, 40., 300.] {
                let mut points = Vec::new();
                flatten_corner(shape, r, MAX_TURN, &mut points);
                for p in &points {
                    let (x, y) = (p.x / r, p.y / r);
                    let error = match shape {
                        CornerShape::Square => p.to_vector().length() / r,
                        CornerShape::Notch => distance_to_polyline(
                            Point::new(x, y),
                            &[Point::new(1., 0.), Point::new(1., 1.), Point::new(0., 1.)],
                        ),
                        CornerShape::Bevel => (x + y - 1.).abs(),
                        CornerShape::Round => ((1. - x).hypot(1. - y) - 1.).abs(),
                        CornerShape::Scoop => (x.hypot(y) - 1.).abs(),
                        _ => {
                            let k = shape.superellipse_k();
                            let n = superellipse_n(k);
                            let (u, v) = if k < 0. { (x, y) } else { (1. - x, 1. - y) };
                            (u.max(0.).powf(n) + v.max(0.).powf(n)).powf(1. / n) - 1.
                        }
                    };
                    assert!(error.abs() < 1e-3, "{shape:?} r={r}: {p:?} is {error} off");
                }
            }
        }
    }

    /// `Scoop` and `Round` are `superellipse(-1)` and `superellipse(1)`.
    #[test]
    fn named_shapes_match_their_superellipse() {
        let deviation = |a: CornerShape, b: CornerShape| {
            let (mut pa, mut pb) = (Vec::new(), Vec::new());
            flatten_corner(a, 100., MAX_TURN, &mut pa);
            flatten_corner(b, 100., MAX_TURN, &mut pb);
            pa.iter().map(|p| distance_to_polyline(*p, &pb)).fold(0f32, f32::max)
        };
        for (named, k) in
            [(CornerShape::Round, 1.), (CornerShape::Scoop, -1.), (CornerShape::Squircle, 2.)]
        {
            let d = deviation(named, CornerShape::Superellipse(k));
            assert!(d < 0.05, "{named:?} vs superellipse({k}): {d}");
        }
    }

    /// Overlapping radii shrink by one shared factor, so a corner keeps its radius when
    /// another one changes shape, as long as neither is `Square`.
    #[test]
    fn fit_radius_scales_overlapping_corners() {
        let rect = euclid::default::Rect::new(Point::new(5., 5.), euclid::size2(40., 100.));
        let radius = BorderRadius::<f32, ()>::new(40., 10., 0., -3.);
        let round = CornerShapes::new_uniform(CornerShape::Round);
        assert_eq!(fit_radius(rect, radius, round), BorderRadius::new(32., 8., 0., 0.));
        let mixed = CornerShapes::new(
            CornerShape::Round,
            CornerShape::Bevel,
            CornerShape::Round,
            CornerShape::Round,
        );
        assert_eq!(fit_radius(rect, radius, mixed), fit_radius(rect, radius, round));
        let square = CornerShapes::new(
            CornerShape::Round,
            CornerShape::Square,
            CornerShape::Round,
            CornerShape::Round,
        );
        assert_eq!(fit_radius(rect, radius, square), BorderRadius::new(40., 0., 0., 0.));
        let fits = BorderRadius::<f32, ()>::new(20., 20., 20., 20.);
        assert_eq!(fit_radius(rect, fits, round), fits);
    }

    /// A huge radius takes a bounded number of points.
    #[test]
    fn huge_radius_is_bounded() {
        let shapes = [
            CornerShape::Round,
            CornerShape::Squircle,
            CornerShape::Superellipse(-3.5),
            CornerShape::Superellipse(12.),
        ];
        for shape in shapes {
            for r in [1e9f32, 1e5] {
                let mut corner = Vec::new();
                flatten_corner(shape, r, MAX_TURN, &mut corner);
                assert!(
                    corner.len() <= MAX_SEGMENTS as usize + 1,
                    "{shape:?} r={r}: {}",
                    corner.len()
                );
            }
        }
    }

    /// Opposite concave corners shrink until their hulls no longer intersect, which convex
    /// corners within a rounded rectangle's limits never do.
    #[test]
    fn fit_radius_separates_opposite_corners() {
        let rect = euclid::default::Rect::new(Point::zero(), euclid::size2(100., 100.));
        let radius = BorderRadius::<f32, ()>::new(80., 0., 80., 0.);
        // The scoop's hull reaches to `x + y == 80 * sqrt(2)` from its tip, so the hulls
        // touch when both radii are `100 / sqrt(2)`.
        let scoops = CornerShapes::new_uniform(CornerShape::Scoop);
        let fitted = fit_radius(rect, radius, scoops);
        let expected = 100. / core::f32::consts::SQRT_2;
        assert!((fitted.top_left - expected).abs() < 1e-3, "{fitted:?}");
        assert!((fitted.bottom_right - expected).abs() < 1e-3, "{fitted:?}");
        // A notch's hull is the corner's whole square, so notches meet sooner.
        let notches = CornerShapes::new_uniform(CornerShape::Notch);
        assert_eq!(fit_radius(rect, radius, notches), BorderRadius::new(50., 0., 50., 0.));
        for shape in [CornerShape::Round, CornerShape::Bevel, CornerShape::Squircle] {
            let shapes = CornerShapes::new_uniform(shape);
            assert_eq!(fit_radius(rect, radius, shapes), radius, "{shape:?}");
            let full = BorderRadius::<f32, ()>::new_uniform(50.);
            assert_eq!(fit_radius(rect, full, shapes), full, "{shape:?}");
        }
    }
}
