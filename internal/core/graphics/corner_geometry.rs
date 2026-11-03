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
/// The corners mustn't overlap.
/// `Round` and `Scoop` corners stay arcs.
pub fn outline<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
) -> Contour {
    if rect.width() <= 0. || rect.height() <= 0. {
        return Contour::Polygons(Vec::new());
    }
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
}
