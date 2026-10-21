// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore ilog, underflows, rposition, superellipses, hypot

/*!
The outline of rectangle corners with a [`CornerShape`], shared by every renderer.

As in CSS, a border's outer edge is the corner shape at the full radius, and its inner edge
is that outline offset inward by the border width, with miter joins like a stroked path's.
An offset of any shape but `Round` isn't the same shape at another radius.

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

/// The limit on a miter's length, relative to half the stroke width, past which it's
/// beveled, as in Skia's default stroke.
const MITER_LIMIT: f32 = 4.;

/// The joint where the offsets `a` (ending the segment along `da`) and `b` (starting the
/// segment along `db`) meet, on the outside of the turn at `vertex`, or `None` for a bevel.
fn miter(
    vertex: Point,
    a: Point,
    da: Vector,
    b: Point,
    db: Vector,
    distance: f32,
) -> Option<Point> {
    line_intersection(a, da, b, db)
        .filter(|joint| (*joint - vertex).length() <= MITER_LIMIT * distance)
}

/// The area a stroke of half-width `distance` along `line` covers: a rectangle per segment,
/// and a miter (or bevel) on the outside of each turn.
struct Stroke<'a> {
    line: &'a [Point],
    distance: f32,
    /// Per segment, its unit direction and length.
    directions: Vec<(Vector, f32)>,
    /// The join at `line[i + 1]`, if it turns: a convex polygon given by a point on each
    /// edge and the edge's outward unit normal, or `None` for an edge of no length.
    joins: Vec<Option<[Option<(Point, Vector)>; 4]>>,
    /// Per segment, the extremes of its own and every earlier segment's bounding box:
    /// the largest `y` and the smallest `x`.
    prefix_max_y: Vec<f32>,
    prefix_min_x: Vec<f32>,
    /// Per segment, the extremes of its own and every later segment's bounding box:
    /// the smallest `y` and the largest `x`.
    suffix_min_y: Vec<f32>,
    suffix_max_x: Vec<f32>,
}

impl<'a> Stroke<'a> {
    fn new(line: &'a [Point], distance: f32) -> Self {
        let joins = line
            .windows(3)
            .map(|w| {
                let (before, vertex, after) = (w[0], w[1], w[2]);
                let (da, db) = ((vertex - before).normalize(), (after - vertex).normalize());
                let cross = da.cross(db);
                if cross.abs() < 1e-6 {
                    return None;
                }
                // Toward the outside of the turn.
                let side = if cross > 0. { -distance } else { distance };
                let a = vertex + Vector::new(-da.y, da.x) * side;
                let b = vertex + Vector::new(-db.y, db.x) * side;
                let joint = miter(vertex, a, da, b, db, distance).unwrap_or(b);
                let polygon = [vertex, a, joint, b];
                let area = (a - vertex).cross(joint - vertex) + (joint - vertex).cross(b - vertex);
                let winding = if area > 0. { 1. } else { -1. };
                Some(core::array::from_fn(|i| {
                    let (u, v) = (polygon[i], polygon[(i + 1) % 4]);
                    let e = v - u;
                    let len = e.length();
                    (len > 0.).then(|| (u, Vector::new(e.y, -e.x) * (winding / len)))
                }))
            })
            .collect();
        let directions = line
            .windows(2)
            .map(|w| {
                let length = (w[1] - w[0]).length();
                (if length > 0. { (w[1] - w[0]) / length } else { Vector::zero() }, length)
            })
            .collect();
        let scan = |extreme: fn(f32, f32) -> f32, of: fn(Point, Point) -> f32| {
            let mut acc = None;
            move |w: &[Point]| {
                let value = of(w[0], w[1]);
                let value = acc.map_or(value, |acc| extreme(acc, value));
                acc = Some(value);
                value
            }
        };
        let segments = line.windows(2);
        let prefix_max_y = segments.clone().map(scan(f32::max, |a, b| a.y.max(b.y))).collect();
        let prefix_min_x = segments.clone().map(scan(f32::min, |a, b| a.x.min(b.x))).collect();
        let mut suffix_min_y: Vec<f32> =
            segments.clone().rev().map(scan(f32::min, |a, b| a.y.min(b.y))).collect();
        let mut suffix_max_x: Vec<f32> =
            segments.rev().map(scan(f32::max, |a, b| a.x.max(b.x))).collect();
        suffix_min_y.reverse();
        suffix_max_x.reverse();
        Self {
            line,
            distance,
            directions,
            joins,
            prefix_max_y,
            prefix_min_x,
            suffix_min_y,
            suffix_max_x,
        }
    }

    /// The range of segments whose bounding box, grown by `pad`, may overlap the box from
    /// `lo` to `hi`.
    /// The extents are monotone however the line runs, so a binary search bounds that
    /// range for any line, and exactly for a corner's line.
    fn segments_near(&self, lo: Point, hi: Point, pad: f32) -> core::ops::Range<usize> {
        let first = self
            .prefix_max_y
            .partition_point(|&y| y < lo.y - pad)
            .max(self.prefix_min_x.partition_point(|&x| x > hi.x + pad));
        let end = self
            .suffix_min_y
            .partition_point(|&y| y <= hi.y + pad)
            .min(self.suffix_max_x.partition_point(|&x| x >= lo.x - pad));
        first..end.max(first)
    }

    /// The joins at the vertices between the segments in `segments`.
    fn joins_between(
        &self,
        segments: core::ops::Range<usize>,
    ) -> &[Option<[Option<(Point, Vector)>; 4]>] {
        // The last segment has no join after it, and `segments` can start past it.
        let end = segments.end.saturating_sub(1).min(self.joins.len());
        &self.joins[segments.start.min(end)..end]
    }

    /// Sets `covered` to the parts of the segment from `a` to `b` covered by more than
    /// `margin`, as sorted, disjoint ranges of the segment's parameter.
    ///
    /// Each segment's rectangle and each join is convex, so the part of the segment inside
    /// one of them is a single range, found by clipping against its edges.
    fn covered_ranges(&self, a: Point, b: Point, margin: f32, covered: &mut Vec<(f32, f32)>) {
        covered.clear();
        let reach = self.distance - margin;
        let (lo, hi) = (a.min(b), a.max(b));
        let ab = b - a;
        let parallel = f32::EPSILON * ab.length();
        // The range of `t` where `a + t * ab` lies strictly inside every half-plane
        // `(p - origin).dot(normal) < limit`, for unit normals.
        let clip = |half_planes: &mut dyn Iterator<Item = (Point, Vector, f32)>| {
            let (mut t0, mut t1) = (0f32, 1f32);
            for (origin, normal, limit) in half_planes {
                let start = (a - origin).dot(normal) - limit;
                let rate = ab.dot(normal);
                if rate.abs() <= parallel {
                    if start >= 0. {
                        return None;
                    }
                } else if rate > 0. {
                    t1 = t1.min(-start / rate);
                } else {
                    t0 = t0.max(-start / rate);
                }
                if t0 >= t1 {
                    return None;
                }
            }
            Some((t0, t1))
        };

        let segments = self.segments_near(lo, hi, self.distance + margin.abs());
        let in_segments = segments.filter_map(|i| {
            let (d, length) = self.directions[i];
            if length <= 0. {
                return None;
            }
            let (s0, n) = (self.line[i], Vector::new(d.y, -d.x));
            // Reaching slightly into the joins, so their seams count as covered.
            clip(
                &mut [(s0, -d, margin), (s0, d, length + margin), (s0, n, reach), (s0, -n, reach)]
                    .into_iter(),
            )
        });
        // A join lies within the miter limit of its vertex.
        let joins = self.segments_near(lo, hi, MITER_LIMIT * self.distance + margin.abs());
        let in_joins = self.joins_between(joins).iter().flatten().filter_map(|edges| {
            // Inside every edge of the convex join by more than `margin`.
            clip(&mut edges.iter().flatten().map(|&(u, n)| (u, n, -margin)))
        });

        for range in in_segments.chain(in_joins) {
            if range.0 <= 0. && range.1 >= 1. {
                // Covered whole, which is how most of a fold's segments are.
                covered.clear();
                covered.push(range);
                return;
            }
            covered.push(range);
        }
        covered.sort_by(|x, y| x.0.total_cmp(&y.0));
        // Gaps shorter than `margin` between two covered ranges count as covered too.
        let gap = margin / ab.length().max(f32::MIN_POSITIVE);
        let mut merged = 0;
        for i in 0..covered.len() {
            let (t0, t1) = covered[i];
            if merged > 0 && t0 <= covered[merged - 1].1 + gap {
                covered[merged - 1].1 = covered[merged - 1].1.max(t1);
            } else {
                covered[merged] = (t0, t1);
                merged += 1;
            }
        }
        covered.truncate(merged);
    }
}

/// Offsets `line` by `distance` to its left (outward for a corner traversed from its
/// incoming to its outgoing edge), or to its right for a negative `distance`.
///
/// On the outside of a turn, consecutive offset segments meet in a miter.
/// On the inside, and wherever the offset folds back over itself past a curve's radius of
/// curvature, it runs inside the stroke along `line`; those parts are cut away.
fn offset_polyline(line: &[Point], distance: f32) -> Vec<Point> {
    let (mut raw, fold_free) = raw_offset(line, distance);
    if !fold_free {
        return uncovered_parts(&raw, &Stroke::new(line, distance.abs()));
    }
    raw.dedup_by(|p, last| same_point(*p, *last));
    raw
}

/// The offset of `line` before any fold is cut away, and whether it has none to cut.
///
/// A line turning only one way can't fold on the outside of its turns.
/// On the inside, it can't fold while every offset segment still runs the same way as
/// the segment it offsets.
fn raw_offset(line: &[Point], distance: f32) -> (Vec<Point>, bool) {
    let normal = |a: Point, b: Point| {
        let d = (b - a).normalize();
        Vector::new(-d.y, d.x) * distance
    };
    let mut raw = Vec::with_capacity(line.len() + 8);
    raw.push(line[0] + normal(line[0], line[1]));
    let (mut all_outside, mut all_inside) = (true, true);
    for w in line.windows(3) {
        let (before, vertex, after) = (w[0], w[1], w[2]);
        let (da, db) = (vertex - before, after - vertex);
        let a = vertex + normal(before, vertex);
        let b = vertex + normal(vertex, after);
        let cross = da.cross(db);
        let joint = if cross.abs() <= 1e-6 * da.length() * db.length() {
            Some(b)
        } else if cross * distance < 0. {
            all_inside = false;
            miter(vertex, a, da, b, db, distance.abs())
        } else {
            all_outside = false;
            // On the inside of the turn, where the two offset segments cross.
            let t = (b - a).cross(db) / cross;
            let u = (b - a).cross(da) / cross;
            let joint = ((-1. ..=0.).contains(&t) && (0. ..=1.).contains(&u)).then(|| a + da * t);
            all_inside &= joint.is_some();
            joint
        };
        match joint {
            Some(joint) => raw.push(joint),
            None => raw.extend([a, b]),
        }
    }
    let n = line.len();
    raw.push(line[n - 1] + normal(line[n - 2], line[n - 1]));
    let fold_free = all_outside
        || (all_inside
            && raw
                .windows(2)
                .zip(line.windows(2))
                .all(|(r, l)| (r[1] - r[0]).dot(l[1] - l[0]) > 0.));
    (raw, fold_free)
}

/// The parts of the polyline `raw` outside `stroke`, joined in order.
fn uncovered_parts(raw: &[Point], stroke: &Stroke) -> Vec<Point> {
    // Loose enough for the stroke's own outline to count as outside.
    let margin = stroke.distance * 1e-5 + 1e-4;
    let mut out: Vec<Point> = Vec::with_capacity(raw.len());
    // The raw segment along which the polyline last went into the stroke.
    let mut entered: Option<(Point, Point)> = None;
    // Whether the polyline is inside the stroke where the current segment starts.
    let mut inside = false;
    let mut covered = Vec::new();
    for w in raw.windows(2) {
        let (a, b) = (w[0], w[1]);
        let at = |t: f32| a.lerp(b, t);
        stroke.covered_ranges(a, b, margin, &mut covered);
        // The uncovered ranges are the gaps around the covered ones.
        let starts = core::iter::once(0.).chain(covered.iter().map(|c| c.1));
        let ends = covered.iter().map(|c| c.0).chain(core::iter::once(1.));
        for (u0, u1) in starts.zip(ends).filter(|(u0, u1)| u0 < u1) {
            if u0 > 0. || inside {
                let crossing = at(u0);
                let corner =
                    entered.take().and_then(|(ea, eb)| line_intersection(ea, eb - ea, a, b - a));
                match (corner, out.last_mut()) {
                    // Leaving where the polyline went in: the true corner is where the two
                    // segments meet.
                    (Some(x), Some(last))
                        if (x - *last).length() < stroke.distance
                            && (x - crossing).length() < stroke.distance =>
                    {
                        *last = x;
                    }
                    _ => push_distinct(&mut out, crossing),
                }
            } else {
                push_distinct(&mut out, a);
            }
            if u1 < 1. {
                push_distinct(&mut out, at(u1));
                entered = Some((a, b));
            } else {
                push_distinct(&mut out, b);
            }
        }
        inside = covered.last().is_some_and(|c| c.1 >= 1.);
    }
    out
}

/// Where the lines through `a` along `da` and through `b` along `db` meet, unless they're
/// parallel.
fn line_intersection(a: Point, da: Vector, b: Point, db: Vector) -> Option<Point> {
    let cross = da.cross(db);
    (cross.abs() > 1e-6 * da.length() * db.length()).then(|| a + da * ((b - a).cross(db) / cross))
}

/// Drops the leading points on the line `y == line` and the trailing ones on `x == line`,
/// but the last of each, so the edge starts and ends where it leaves the straight edges.
fn trim_to_corner(points: &mut Vec<Point>, line: f32) {
    const EPSILON: f32 = 1e-3;
    let start = points
        .windows(2)
        .position(|w| (w[1].y - line).abs() > EPSILON)
        .unwrap_or(points.len().saturating_sub(1));
    let end = points
        .windows(2)
        .rposition(|w| (w[0].x - line).abs() > EPSILON)
        .map_or(0, |i| i + 1)
        .max(start);
    points.truncate(end + 1);
    points.drain(..start);
}

/// The corner of `shape` and radius `r` offset by `distance`, outward for a positive
/// `distance` and inward for a negative one, in the corner frame, starting with a
/// [`ContourVertex::Line`].
///
/// It runs from the incoming straight edge, `y == -distance`, to the outgoing one,
/// `x == -distance`, `x` never increasing and `y` never decreasing along the way.
fn corner_edge(shape: CornerShape, r: f32, distance: f32) -> Vec<ContourVertex> {
    use ContourVertex::{Arc, Line};
    let d = distance;
    let k = shape.superellipse_k();
    // The straight edges meet in a miter, or cross inside the corner when shrinking.
    let sharp = || vec![Line(Point::new(-d, -d))];
    let notch = || [(r - d, -d), (r - d, r - d), (-d, r - d)].map(|(x, y)| Line(Point::new(x, y)));
    if r <= 0. || k == f32::INFINITY {
        return sharp();
    }
    match k {
        f32::NEG_INFINITY => notch().into(),
        0. => {
            // The diagonal `x + y == r` moves by `d * sqrt(2)` along its normal.
            let along = r - d * core::f32::consts::SQRT_2 + d;
            if along <= -d {
                return sharp();
            }
            vec![Line(Point::new(along, -d)), Line(Point::new(-d, along))]
        }
        1. => {
            let radius = r + d;
            if radius <= 0. {
                return sharp();
            }
            let center = Point::new(r, r);
            vec![Line(Point::new(r, -d)), Arc { center, to: Point::new(-d, r) }]
        }
        // The scoop is a circle around the tip, meeting the straight edges at right angles.
        -1. if d >= 0. => {
            // Growing past the radius leaves the miters of both ends, meeting in a notch.
            let radius = r - d;
            if radius <= 0. {
                return notch().into();
            }
            let mut edge = Vec::with_capacity(4);
            if d > 0. {
                edge.push(Line(Point::new(radius, -d)));
            }
            edge.push(Line(Point::new(radius, 0.)));
            edge.push(Arc { center: Point::zero(), to: Point::new(0., radius) });
            if d > 0. {
                edge.push(Line(Point::new(-d, radius)));
            }
            edge
        }
        -1. => {
            // The arc ends where it meets the straight edges, at `y == inset` and `x == inset`.
            let (inset, radius) = (-d, r - d);
            let reach = (radius * radius - inset * inset).max(0.).sqrt();
            if reach <= inset {
                return sharp();
            }
            vec![
                Line(Point::new(reach, inset)),
                Arc { center: Point::zero(), to: Point::new(inset, reach) },
            ]
        }
        _ => polyline_offset(shape, r, distance).into_iter().map(ContourVertex::Line).collect(),
    }
}

/// [`corner_edge`] as a polyline.
#[cfg(test)]
fn corner_offset(shape: CornerShape, r: f32, distance: f32) -> Vec<Point> {
    let mut points = Vec::new();
    extend_flattened(&mut points, corner_edge(shape, r, distance));
    points
}

/// [`corner_edge`] from offsetting the flattened corner, for any shape.
/// Only superellipses without a closed form need it.
fn polyline_offset(shape: CornerShape, r: f32, distance: f32) -> Vec<Point> {
    let line = corner_line(shape, r, distance);
    let mut edge = if distance != 0. { offset_polyline(&line, distance) } else { line };
    trim_to_corner(&mut edge, -distance);
    edge
}

/// The corner [`polyline_offset`] offsets, extended along both straight edges.
fn corner_line(shape: CornerShape, r: f32, distance: f32) -> Vec<Point> {
    let d = distance.abs();
    // The error between a miter and the true offset of a curve, `d * turn^2 / 8`, stays
    // within the tolerance.
    let max_turn = if d > 0. { (8. * TOLERANCE / d).sqrt().min(MAX_TURN) } else { MAX_TURN };
    // Long enough that no join reaches the end of a run.
    let run = MITER_LIMIT * d + 1.;
    let r = r.max(0.);

    let mut line = vec![Point::new(r + run, 0.)];
    flatten_corner(shape, r, max_turn, &mut line);
    line.push(Point::new(0., r + run));
    line.dedup_by(|p, last| same_point(*p, *last));
    line
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

/// An outline from [`offset_outline`].
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

/// The outline of `rect` with corner radii `radius` and shapes `shapes`, offset by
/// `distance`: grown for a positive `distance` and shrunk for a negative one.
/// `Round` and `Scoop` corners stay arcs, unless the corners reach past each other.
/// Shrinking a rectangle whose corners do that can split it into several polygons.
pub fn offset_outline<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
    distance: f32,
) -> Contour {
    let [outline] = offset_outlines(rect, radius, shapes, [distance]);
    outline
}

/// [`offset_outline`] by each of `distances`, fitting the radii once.
pub fn offset_outlines<U, const N: usize>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
    distances: [f32; N],
) -> [Contour; N] {
    let radius = fit_radius(rect, radius, shapes);
    distances.map(|distance| offset_fitted_outline(rect, radius, shapes, distance))
}

/// [`offset_outline`] with `radius` already fitted by [`fit_radius`].
fn offset_fitted_outline<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
    distance: f32,
) -> Contour {
    let bounds = rect.inflate(distance, distance);
    if bounds.width() <= 0. || bounds.height() <= 0. {
        return Contour::Polygons(Vec::new());
    }
    let mut corners = corners_in_path_order(radius, shapes).into_iter();
    let corners = corner_frames(rect).map(|(tip, e_in, e_out)| {
        let (r, shape) = corners.next().unwrap();
        corner_edge(shape, r, distance)
            .into_iter()
            .map(|v| v.map(|p| tip + e_in * p.x + e_out * p.y))
            .collect::<Vec<_>>()
    });
    // Each corner's offset spans the box from its tip to its far ends. Where every box stays
    // within the outline's bounds and no two overlap, the corners can't reach past each other.
    // An arc stays within a quadrant, and so within the box of its ends.
    let boxes = corners
        .each_ref()
        .map(|corner| euclid::default::Box2D::from_points(corner.iter().map(|v| v.end())));
    let within = bounds.to_box2d().inflate(1e-3, 1e-3);
    let apart = boxes.iter().all(|b| within.contains_box(b))
        && (0..4).all(|i| (i + 1..4).all(|j| !boxes[i].intersects(&boxes[j])));
    if apart {
        let mut vertices: Vec<ContourVertex> =
            Vec::with_capacity(corners.iter().map(Vec::len).sum());
        for v in corners.into_iter().flatten() {
            let repeats = |last: &ContourVertex| same_point(v.end(), last.end());
            if !matches!(v, ContourVertex::Line(_)) || !vertices.last().is_some_and(repeats) {
                vertices.push(v);
            }
        }
        return Contour::Curved(vertices);
    }
    Contour::Polygons(column_regions(
        corners.map(|corner| {
            let mut points = Vec::with_capacity(corner.len());
            extend_flattened(&mut points, corner);
            points
        }),
        bounds,
    ))
}

/// The region within `bounds` that none of `corners`, in the order of [`corner_frames`],
/// cuts away.
///
/// Every corner's offset is monotone, so it cuts each column of `bounds` from its own side.
/// The region spans, in each column, from the lowest edge the top corners leave to the
/// highest edge the bottom corners leave.
fn column_regions(corners: [Vec<Point>; 4], bounds: euclid::default::Rect<f32>) -> Vec<Vec<Point>> {
    let [top_right, bottom_right, bottom_left, top_left] =
        corners.map(|corner| column_profile(corner, bounds.min_x(), bounds.max_x()));
    let top = envelope(&top_left, &top_right, f32::max);
    let bottom = envelope(&bottom_left, &bottom_right, f32::min);
    regions_between(&top, &bottom)
}

/// `corner`, a polyline monotone in `x`, as a function of `x` over `x0..=x1`: sorted by `x`,
/// clipped to that range, and extended level to both ends of it.
/// Consecutive points with the same `x` make the function jump there.
fn column_profile(mut corner: Vec<Point>, x0: f32, x1: f32) -> Vec<Point> {
    if corner.first().zip(corner.last()).is_some_and(|(first, last)| first.x > last.x) {
        corner.reverse();
    }
    let mut profile: Vec<Point> = Vec::with_capacity(corner.len() + 2);
    let mut x = f32::NEG_INFINITY;
    for p in corner {
        // Rounding can turn a vertical step slightly backward.
        x = x.max(p.x);
        let p = Point::new(x, p.y);
        if let Some(&last) = profile.last() {
            for edge in [x0, x1] {
                if last.x < edge && p.x > edge {
                    profile.push(last.lerp(p, (edge - last.x) / (p.x - last.x)));
                }
            }
        }
        profile.push(p);
    }
    profile.retain(|p| (x0..=x1).contains(&p.x));
    if let Some(first) = profile.first_mut()
        && first.x > x0
        && let Some(level) = snap_end(first, x0)
    {
        profile.insert(0, level);
    }
    if let Some(last) = profile.last_mut()
        && last.x < x1
        && let Some(level) = snap_end(last, x1)
    {
        profile.push(level);
    }
    profile
}

/// Moves `end` to `edge` when it's within rounding of it, as a step there would make a
/// convex outline concave, or returns the point that extends the profile level to `edge`.
fn snap_end(end: &mut Point, edge: f32) -> Option<Point> {
    const SNAP: f32 = 1e-3;
    if (edge - end.x).abs() <= SNAP {
        end.x = edge;
        None
    } else {
        Some(Point::new(edge, end.y))
    }
}

/// The values of the [`column_profile`] `profile` just left and just right of `x`.
fn profile_at(profile: &[Point], x: f32) -> (f32, f32) {
    let (start, end) =
        (profile.partition_point(|p| p.x < x), profile.partition_point(|p| p.x <= x));
    if start < end {
        return (profile[start].y, profile[end - 1].y);
    }
    let y = match (start.checked_sub(1).map(|i| profile[i]), profile.get(start).copied()) {
        (Some(a), Some(b)) => a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x),
        (Some(p), None) | (None, Some(p)) => p.y,
        (None, None) => 0.,
    };
    (y, y)
}

/// The `x` of every vertex of `a` and `b`, sorted, without duplicates.
fn merged_breaks(a: &[Point], b: &[Point]) -> Vec<f32> {
    let mut xs: Vec<f32> = a.iter().chain(b).map(|p| p.x).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    xs
}

/// The values of two profiles at one `x`.
#[derive(Clone, Copy)]
struct Column {
    x: f32,
    a: f32,
    b: f32,
    /// Whether `x` is a vertex of each profile, or where they cross.
    at_vertex: [bool; 2],
}

/// The values of the profiles `a` and `b` along their common range, in order of `x`: at
/// every vertex of either, once from the left and once from the right where they differ,
/// and at every `x` between vertices where they cross.
fn columns(a: &[Point], b: &[Point]) -> Vec<Column> {
    let is_vertex = |profile: &[Point], x: f32| {
        profile.get(profile.partition_point(|p| p.x < x)).is_some_and(|p| p.x == x)
    };
    let mut columns: Vec<Column> = Vec::with_capacity(2 * (a.len() + b.len()));
    for x in merged_breaks(a, b) {
        let ((left_a, right_a), (left_b, right_b)) = (profile_at(a, x), profile_at(b, x));
        if let Some(previous) = columns.last() {
            let (d0, d1) = (previous.a - previous.b, left_a - left_b);
            if d0 * d1 < 0. {
                let t = d0 / (d0 - d1);
                let y = previous.a + (left_a - previous.a) * t;
                let x = previous.x + (x - previous.x) * t;
                columns.push(Column { x, a: y, b: y, at_vertex: [true; 2] });
            }
        }
        let at_vertex = [is_vertex(a, x), is_vertex(b, x)];
        columns.push(Column { x, a: left_a, b: left_b, at_vertex });
        if (right_a, right_b) != (left_a, left_b) {
            columns.push(Column { x, a: right_a, b: right_b, at_vertex });
        }
    }
    columns
}

/// The maximum or minimum, by `pick`, of the profiles `a` and `b`.
fn envelope(a: &[Point], b: &[Point], pick: fn(f32, f32) -> f32) -> Vec<Point> {
    let columns = columns(a, b);
    let side = |c: &Column| usize::from(pick(c.a, c.b) != c.a);
    let mut out: Vec<Point> = Vec::new();
    for (i, c) in columns.iter().enumerate() {
        let s = side(c);
        // Where one profile steps past the other, the envelope turns at a point of neither.
        let steps = |j: Option<usize>| {
            j.and_then(|j| columns.get(j)).is_some_and(|n| n.x == c.x && side(n) != s)
        };
        let vertex = c.at_vertex[s] || (c.a == c.b && c.at_vertex.contains(&true));
        if vertex || steps(i.checked_sub(1)) || steps(Some(i + 1)) {
            push_distinct(&mut out, Point::new(c.x, pick(c.a, c.b)));
        }
    }
    out
}

/// The polygons enclosed between the profiles `top` and `bottom` wherever `top` lies above
/// `bottom`, each running left to right along `top` and back along `bottom`.
fn regions_between(top: &[Point], bottom: &[Point]) -> Vec<Vec<Point>> {
    let columns = columns(top, bottom);
    let open = |c: &Column| c.a < c.b;
    let mut regions = Vec::new();
    let mut i = 0;
    while i < columns.len() {
        if !open(&columns[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while columns.get(i).is_some_and(open) {
            i += 1;
        }
        let run = &columns[start..i];
        // A region either narrows to where the edges cross, which `columns` reports as a
        // column of its own, or ends along a vertical step.
        let meeting = |c: Option<&Column>, x: f32| {
            c.filter(|c| c.a == c.b && c.x != x).map(|c| Point::new(c.x, c.a))
        };
        let first = meeting(start.checked_sub(1).map(|j| &columns[j]), run[0].x);
        let last = meeting(columns.get(i), run[run.len() - 1].x);
        let mut polygon: Vec<Point> = first.into_iter().collect();
        let mut lower: Vec<Point> = Vec::new();
        for (k, c) in run.iter().enumerate() {
            let step = (k == 0 && first.is_none()) || (k == run.len() - 1 && last.is_none());
            if step || c.at_vertex[0] {
                push_distinct(&mut polygon, Point::new(c.x, c.a));
            }
            if step || c.at_vertex[1] {
                push_distinct(&mut lower, Point::new(c.x, c.b));
            }
        }
        polygon.extend(last);
        for p in lower.into_iter().rev() {
            push_distinct(&mut polygon, p);
        }
        if polygon.len() > 1 && same_point(polygon[0], polygon[polygon.len() - 1]) {
            polygon.pop();
        }
        if polygon.len() >= 3 {
            regions.push(polygon);
        }
    }
    regions
}

fn push_distinct(points: &mut Vec<Point>, p: Point) {
    if !points.last().is_some_and(|&last| same_point(p, last)) {
        points.push(p);
    }
}

fn same_point(a: Point, b: Point) -> bool {
    (a - b).square_length() <= 1e-8
}

/// The outer and inner edges of a rectangle's border.
#[derive(Debug, Clone)]
pub struct BorderContours {
    /// The border's outer edge.
    pub outer: Contour,
    /// The border's inner edge, with no area when the border covers the whole rectangle.
    pub inner: Contour,
}

/// The edges of a border of width `border_width` whose outer edge is the outline of the
/// rectangle `rect`, with corner radii `radius` and shapes `shapes`, grown by `spread`.
pub fn border_contours<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
    border_width: f32,
    spread: f32,
) -> BorderContours {
    let [outer, inner] =
        offset_outlines(rect, radius, shapes, [spread, spread - border_width.max(0.)]);
    BorderContours { outer, inner }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Where the segments `a0 a1` and `b0 b1` properly cross, if they do.
    fn segment_intersection(a0: Point, a1: Point, b0: Point, b1: Point) -> Option<Point> {
        let (da, db) = (a1 - a0, b1 - b0);
        let denom = da.cross(db);
        if denom.abs() <= f32::EPSILON * da.length() * db.length() {
            return None;
        }
        let t = (b0 - a0).cross(db) / denom;
        let u = (b0 - a0).cross(da) / denom;
        ((0. ..=1.).contains(&t) && (0. ..1.).contains(&u)).then(|| a0 + da * t)
    }

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

    /// A finely flattened corner, extended along the straight edges.
    fn extended_corner(shape: CornerShape, r: f32, d: f32) -> Vec<Point> {
        let mut corner = Vec::new();
        flatten_corner(shape, r, 0.01, &mut corner);
        core::iter::once(Point::new(r + 4. * d + 10., 0.))
            .chain(corner)
            .chain(core::iter::once(Point::new(0., r + 4. * d + 10.)))
            .collect()
    }

    fn assert_monotone(shape: CornerShape, edge: &[Point]) {
        for w in edge.windows(2) {
            assert!(
                w[1].x <= w[0].x + 1e-3 && w[1].y >= w[0].y - 1e-3,
                "{shape:?}: {edge:?} isn't monotone at {:?} -> {:?}",
                w[0],
                w[1]
            );
        }
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

    /// A round corner offset by `d` is the circle of radius `r - d` around the corner's center.
    #[test]
    fn round_offsets_are_concentric_circles() {
        for (r, d) in [(20f32, 2f32), (6., 5.), (100., 0.5)] {
            let center = Point::new(r, r);
            for distance in [d, -d] {
                for p in corner_offset(CornerShape::Round, r, distance) {
                    let error = (p - center).length() - (r + distance);
                    assert!(error.abs() < TOLERANCE, "r={r} d={distance}: {p:?} is {error} off");
                }
            }
        }
        // Past the radius, the inner straight edges meet in a sharp corner.
        let inner = corner_offset(CornerShape::Round, 3., -10.);
        assert_eq!(inner.len(), 1);
        assert!((inner[0] - Point::new(10., 10.)).length() < 1e-3, "{inner:?}");
    }

    /// The closed-form offsets match offsetting the flattened corners.
    #[test]
    fn closed_form_offsets_match_the_polyline_offset() {
        // The flattened arc and the offset polyline each stray from the true offset.
        const SLACK: f32 = 3. * TOLERANCE;
        let one_way = |a: &[Point], b: &[Point]| {
            a.iter().map(|p| distance_to_polyline(*p, b)).fold(0f32, f32::max)
        };
        for shape in [
            CornerShape::Square,
            CornerShape::Notch,
            CornerShape::Bevel,
            CornerShape::Round,
            CornerShape::Scoop,
            CornerShape::Superellipse(-1.),
            CornerShape::Superellipse(0.),
        ] {
            for r in [0.5f32, 6., 40.] {
                for d in [0f32, 0.3, 3., 6., 20., 50.] {
                    for distance in [d, -d] {
                        // A growing scoop is mitered where it meets the straight edges, and the
                        // flattened curve's first chord tilts that miter by up to half a
                        // segment's turn, which the polyline offset doesn't bound.
                        if shape.superellipse_k() == -1. && distance > 0. {
                            continue;
                        }
                        // Each continues along the straight edges, wherever it's trimmed.
                        let extend = |edge: Vec<Point>| {
                            let run = r + d + 10.;
                            core::iter::once(Point::new(run, -distance))
                                .chain(edge)
                                .chain(core::iter::once(Point::new(-distance, run)))
                                .collect::<Vec<_>>()
                        };
                        let arc = extend(corner_offset(shape, r, distance));
                        let polyline = extend(polyline_offset(shape, r, distance));
                        let deviation = one_way(&arc, &polyline).max(one_way(&polyline, &arc));
                        assert!(
                            deviation < SLACK,
                            "{shape:?} r={r} d={distance}: {arc:?} vs {polyline:?}"
                        );
                    }
                }
            }
        }
    }

    /// A growing scoop leaves square miters where it meets the straight edges, which meet in a
    /// notch once it grows past its radius.
    #[test]
    fn growing_scoop_is_mitered() {
        let r = 6f32;
        for d in [2f32, 9.] {
            let wall = r - d;
            let edge = corner_offset(CornerShape::Scoop, r, d);
            let (start, end) = (
                [Point::new(wall, -d), Point::new(wall, 0.)],
                [Point::new(0., wall), Point::new(-d, wall)],
            );
            if wall > 0. {
                assert_eq!(edge[..2], start, "{d}: {edge:?}");
                assert_eq!(edge[edge.len() - 2..], end, "{d}: {edge:?}");
            } else {
                assert_eq!(edge, [start[0], Point::new(wall, wall), end[1]]);
            }
        }
    }

    /// Offsetting a scoop inward past its radius leaves a short arc between the two straight
    /// edges, narrower than any fixed sampling of the raw offset would find.
    #[test]
    fn thin_uncovered_parts_are_kept() {
        let (r, d) = (8f32, 16f32);
        let edge = corner_offset(CornerShape::Scoop, r, -d);
        let arc_radius = r + d;
        let on_arc = Point::new(1., 1.).to_vector().normalize() * arc_radius;
        let distance = distance_to_polyline(on_arc.to_point(), &edge);
        assert!(distance < TOLERANCE, "{edge:?} misses the arc by {distance}");
    }

    /// A huge radius, even with a sub-pixel offset, takes a bounded number of points.
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
            for (r, d) in [(1e9f32, 1f32), (1e5, 0.005)] {
                for distance in [d, -d] {
                    let edge = corner_offset(shape, r, distance);
                    assert!(
                        edge.len() <= 4 * MAX_SEGMENTS as usize,
                        "{shape:?} r={r} d={distance}: {}",
                        edge.len()
                    );
                }
            }
        }
    }

    /// The bevel's diagonal and the notch's and square's miters, offset either way, from their
    /// closed forms.
    #[test]
    fn straight_shapes_match_their_closed_form() {
        let (r, d) = (20f32, 3f32);
        let offset = d * core::f32::consts::SQRT_2;
        let outer = corner_offset(CornerShape::Bevel, r, d);
        let inner = corner_offset(CornerShape::Bevel, r, -d);
        assert_eq!(outer.len(), 2);
        assert_eq!(inner.len(), 2);
        for p in &outer {
            assert!((p.x + p.y - (r - offset)).abs() < 1e-3, "outer {p:?}");
        }
        for p in &inner {
            assert!((p.x + p.y - (r + offset)).abs() < 1e-3, "inner {p:?}");
        }
        assert!((inner[0].y - d).abs() < 1e-3 && (inner[1].x - d).abs() < 1e-3);

        let close = |a: &[Point], b: &[Point]| {
            a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (*p - *q).length() < 1e-3)
        };
        for distance in [d, -d] {
            let notch = corner_offset(CornerShape::Notch, r, distance);
            let (wall, edge) = (r - distance, -distance);
            let expected = [Point::new(wall, edge), Point::new(wall, wall), Point::new(edge, wall)];
            assert!(close(&notch, &expected), "{distance}: {notch:?}");

            let square = corner_offset(CornerShape::Square, r, distance);
            assert!(close(&square, &[Point::new(edge, edge)]), "{distance}: {square:?}");
        }
    }

    /// No offset point comes closer to the corner than the offset distance, however large it is
    /// against the curve: the offset's loops are all cut away.
    #[test]
    fn offsets_keep_their_distance() {
        for shape in SHAPES {
            for (r, d) in [(30f32, 1f32), (30., 10.), (8., 15.), (100., 45.), (100., 90.)] {
                let corner = extended_corner(shape, r, d);
                for distance in [d, -d] {
                    let edge = corner_offset(shape, r, distance);
                    assert_monotone(shape, &edge);
                    for p in &edge {
                        let to_corner = distance_to_polyline(*p, &corner);
                        assert!(
                            to_corner > d - 0.05,
                            "{shape:?} r={r} d={distance}: {p:?} is {to_corner} from the corner"
                        );
                    }
                }
            }
        }
    }

    /// Every point of the corner is the offset distance from its offsets.
    #[test]
    fn offsets_follow_the_corner() {
        // Where the curve bends tighter than the offset, the offset rounds it off and stays
        // further away; these shapes don't.
        for shape in [
            CornerShape::Round,
            CornerShape::Scoop,
            CornerShape::Squircle,
            CornerShape::Superellipse(0.5),
        ] {
            for (r, d) in [(40f32, 4f32), (80., 10.)] {
                let mut corner = Vec::new();
                flatten_corner(shape, r, 0.01, &mut corner);
                // A concave corner meets the straight edges at an angle, where the inner
                // offset is trimmed further away.
                let (start, end) = (corner[0], *corner.last().unwrap());
                let away_from_ends =
                    |p: &Point| (*p - start).length() > 2. * d && (*p - end).length() > 2. * d;
                for distance in [d, -d] {
                    let edge = corner_offset(shape, r, distance);
                    for p in corner.iter().filter(|p| away_from_ends(p)) {
                        let to_edge = distance_to_polyline(*p, &edge);
                        assert!(
                            (to_edge - d).abs() < 2. * TOLERANCE,
                            "{shape:?} r={r} d={distance}: the offset is {to_edge} from {p:?}"
                        );
                    }
                }
            }
        }
    }

    /// A border's outer edge is the shape itself, so that it matches every other outline drawn
    /// at the same radius, such as a clip or a shadow.
    #[test]
    fn border_outer_edge_is_the_shape() {
        let rect = euclid::default::Rect::new(Point::new(5., 5.), euclid::size2(100., 60.));
        for shape in SHAPES {
            let contours = border_polygons(
                rect,
                BorderRadius::new_uniform(20.),
                CornerShapes::new_uniform(shape),
                10.,
                0.,
            );
            assert_eq!(contours.outer.len(), 1, "{shape:?}");
            for p in &contours.outer[0] {
                // Each point belongs to the corner of the nearest tip.
                let (tip, e_in, e_out) = corner_frames(rect)
                    .into_iter()
                    .min_by(|a, b| (*p - a.0).length().total_cmp(&(*p - b.0).length()))
                    .unwrap();
                let local = Point::new((*p - tip).dot(e_in), (*p - tip).dot(e_out));
                let corner = distance_to_polyline(local, &extended_corner(shape, 20., 10.));
                assert!(corner < 0.03, "{shape:?}: {p:?} is {corner} off the shape");
            }
        }
    }

    /// [`border_contours`] with both edges as polygons.
    struct BorderPolygons {
        outer: Vec<Vec<Point>>,
        inner: Vec<Vec<Point>>,
    }

    fn border_polygons(
        rect: euclid::default::Rect<f32>,
        radius: BorderRadius<f32, ()>,
        shapes: CornerShapes,
        border_width: f32,
        spread: f32,
    ) -> BorderPolygons {
        let BorderContours { outer, inner } =
            border_contours(rect, radius, shapes, border_width, spread);
        BorderPolygons { outer: outer.into_polygons(), inner: inner.into_polygons() }
    }

    fn polygon_is_simple(polygon: &[Point]) -> bool {
        let n = polygon.len();
        (0..n).all(|i| {
            (i + 2..n).all(|j| {
                if i == 0 && j == n - 1 {
                    return true;
                }
                let (a0, a1) = (polygon[i], polygon[(i + 1) % n]);
                let (b0, b1) = (polygon[j], polygon[(j + 1) % n]);
                segment_intersection(a0, a1, b0, b1).is_none_or(|p| {
                    // Touching at a shared vertex isn't a crossing.
                    [a0, a1].iter().any(|v| (*v - p).length() < 1e-3)
                        && [b0, b1].iter().any(|v| (*v - p).length() < 1e-3)
                })
            })
        })
    }

    /// Whether `p` lies inside `polygon`, by the even-odd rule.
    fn contains(polygon: &[Point], p: Point) -> bool {
        let n = polygon.len();
        (0..n).fold(false, |inside, i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % n]);
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                !inside
            } else {
                inside
            }
        })
    }

    fn signed_area(polygon: &[Point]) -> f32 {
        let n = polygon.len();
        (0..n).map(|i| polygon[i].to_vector().cross(polygon[(i + 1) % n].to_vector())).sum::<f32>()
            / 2.
    }

    /// Asserts that `region` is made of simple, clockwise polygons, and that it holds every
    /// point of `inside` and none of `outside`.
    fn assert_region(region: &[Vec<Point>], inside: &[(f32, f32)], outside: &[(f32, f32)]) {
        for polygon in region {
            assert!(polygon_is_simple(polygon), "{polygon:?}");
            assert!(signed_area(polygon) > 0., "{polygon:?} isn't clockwise");
        }
        let covers = |p: Point| region.iter().filter(|polygon| contains(polygon, p)).count();
        for &p in inside {
            assert_eq!(covers(p.into()), 1, "{p:?} isn't in {region:?}");
        }
        for &p in outside {
            assert_eq!(covers(p.into()), 0, "{p:?} is in {region:?}");
        }
    }

    /// Adjacent corners whose inner edges reach past each other are cut where they cross,
    /// leaving a simple polygon.
    #[test]
    fn border_contours_join_overlapping_corners() {
        let square = euclid::default::Rect::new(Point::new(5., 5.), euclid::size2(50., 50.));
        // Notches overlapping along a side leave no area in the rows they cover.
        let tall = euclid::default::Rect::new(Point::new(5., 5.), euclid::size2(50., 90.));
        for (shape, rect) in
            [(CornerShape::Bevel, square), (CornerShape::Scoop, square), (CornerShape::Notch, tall)]
        {
            let contours = border_polygons(
                rect,
                BorderRadius::new_uniform(20.),
                CornerShapes::new_uniform(shape),
                10.,
                0.,
            );
            assert_eq!(contours.inner.len(), 1, "{shape:?}: {:?}", contours.inner);
            assert_eq!(contours.outer.len(), 1, "{shape:?}: {:?}", contours.outer);
            assert_region(&contours.inner, &[], &[]);
            assert_region(&contours.outer, &[], &[]);
            let inner_rect = rect.inflate(-10., -10.);
            for p in &contours.inner[0] {
                assert!(
                    p.x >= inner_rect.min_x() - 1e-3
                        && p.x <= inner_rect.max_x() + 1e-3
                        && p.y >= inner_rect.min_y() - 1e-3
                        && p.y <= inner_rect.max_y() + 1e-3,
                    "{shape:?}: {p:?} lies outside the inner rectangle"
                );
            }
        }
        let notched = border_polygons(
            square,
            BorderRadius::new_uniform(20.),
            CornerShapes::new_uniform(CornerShape::Notch),
            10.,
            0.,
        );
        assert!(notched.inner.is_empty(), "{:?}", notched.inner);
        let covered = border_polygons(
            square,
            BorderRadius::new_uniform(4.),
            CornerShapes::new_uniform(CornerShape::Bevel),
            30.,
            0.,
        );
        assert!(covered.inner.is_empty());
    }

    /// Convex corners give a single convex outline, grown or shrunk, with no step left by
    /// rounding where a corner meets a straight edge.
    #[test]
    fn convex_corners_keep_the_outline_convex() {
        let rect = euclid::default::Rect::new(Point::new(2., 2.), euclid::size2(98.5, 49.));
        let shapes = CornerShapes::new(
            CornerShape::Round,
            CornerShape::Squircle,
            CornerShape::Square,
            CornerShape::Superellipse(4.),
        );
        for distance in [-5f32, 0., 5.] {
            let contour =
                offset_outline::<()>(rect, BorderRadius::new_uniform(16.), shapes, distance)
                    .into_polygons();
            assert_eq!(contour.len(), 1, "{distance}: {contour:?}");
            let polygon = &contour[0];
            let n = polygon.len();
            for i in 0..n {
                let (a, b, c) = (polygon[i], polygon[(i + 1) % n], polygon[(i + 2) % n]);
                assert!(
                    (b - a).cross(c - b) >= 0.,
                    "{distance}: turns back at {b:?} in {polygon:?}"
                );
            }
        }
    }

    /// Where corners stay apart, joining them in order gives the region that cutting each
    /// column gives.
    #[test]
    fn apart_corners_join_like_columns() {
        let rect = euclid::default::Rect::new(Point::new(3., 7.), euclid::size2(120., 70.));
        for shape in SHAPES {
            let shapes = CornerShapes::new(shape, shape, shape, CornerShape::Squircle);
            for r in [0f32, 6., 25.] {
                let radius = BorderRadius::<f32, ()>::new(r, r * 0.5, r, r * 1.5);
                for distance in [-6f32, 0., 6.] {
                    let joined = offset_outline(rect, radius, shapes, distance).into_polygons();
                    let fitted = fit_radius(rect, radius, shapes);
                    let mut corners = corners_in_path_order(fitted, shapes).into_iter();
                    let corners = corner_frames(rect).map(|(tip, e_in, e_out)| {
                        let (r, shape) = corners.next().unwrap();
                        corner_offset(shape, r, distance)
                            .into_iter()
                            .map(|p| tip + e_in * p.x + e_out * p.y)
                            .collect::<Vec<_>>()
                    });
                    let cut = column_regions(corners, rect.inflate(distance, distance));
                    let area =
                        |region: &[Vec<Point>]| region.iter().map(|p| signed_area(p)).sum::<f32>();
                    let context = alloc::format!("{shape:?} r{r} d{distance}");
                    assert_eq!(joined.len(), 1, "{context}");
                    assert!(
                        (area(&joined) - area(&cut)).abs() < 0.1,
                        "{context}: {joined:?} vs {cut:?}"
                    );
                    assert_region(&joined, &[], &[]);
                }
            }
        }
    }

    /// A corner whose inner edge reaches past the whole adjacent corner still leaves the
    /// inner region below it.
    #[test]
    fn inner_edge_reaching_past_the_adjacent_corner() {
        let rect = euclid::default::Rect::new(Point::zero(), euclid::size2(50., 200.));
        for top_left in [CornerShape::Notch, CornerShape::Scoop] {
            for (top_right, r) in
                [(CornerShape::Bevel, 5.), (CornerShape::Round, 5.), (CornerShape::Round, 0.)]
            {
                let shapes =
                    CornerShapes::new(top_left, top_right, CornerShape::Round, CornerShape::Round);
                let contours =
                    border_polygons(rect, BorderRadius::new(45., r, 0., 0.), shapes, 10., 0.);
                let context = alloc::format!("{top_left:?} {top_right:?} r{r}");
                assert_eq!(contours.inner.len(), 1, "{context}: {:?}", contours.inner);
                // A notch's inner wall lies at 55, past the far side, so it covers whole rows.
                let inside = if top_left == CornerShape::Notch { (25., 60.) } else { (35., 60.) };
                assert_region(&contours.inner, &[inside, (25., 180.)], &[(25., 20.), (5., 100.)]);
            }
        }
    }

    /// Opposite notches whose inner edges overlap in the middle split the inner region in two.
    #[test]
    fn opposite_corners_split_the_inner_region() {
        let rect = euclid::default::Rect::new(Point::zero(), euclid::size2(100., 100.));
        let shapes = CornerShapes::new(
            CornerShape::Notch,
            CornerShape::Round,
            CornerShape::Notch,
            CornerShape::Round,
        );
        let contours = border_polygons(rect, BorderRadius::new(50., 0., 50., 0.), shapes, 10., 0.);
        assert_eq!(contours.inner.len(), 2, "{:?}", contours.inner);
        assert_region(
            &contours.inner,
            &[(75., 25.), (25., 75.)],
            &[(50., 50.), (30., 30.), (70., 70.), (5., 50.)],
        );
        // The notches meet at the center, so the outline touches itself there.
        assert_region(&contours.outer, &[(75., 25.), (25., 75.)], &[(25., 25.), (75., 75.)]);
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

    /// An offset `raw_offset` reports as fold-free has nothing for `uncovered_parts` to cut.
    #[test]
    fn fold_free_offsets_need_no_trimming() {
        let shapes =
            SHAPES.into_iter().chain((-40..=40).map(|k| CornerShape::Superellipse(k as f32 / 8.)));
        for shape in shapes {
            for r in [0., 0.3, 1., 3., 8., 20., 100., 400.] {
                for d in [0.05, 0.25, 0.5, 1., 2.5, 5., 8.5, 20., 55., 150.] {
                    for distance in [d, -d] {
                        let line = corner_line(shape, r, distance);
                        let (raw, fold_free) = raw_offset(&line, distance);
                        if !fold_free {
                            continue;
                        }
                        let trimmed = uncovered_parts(&raw, &Stroke::new(&line, d));
                        assert_eq!(
                            offset_polyline(&line, distance),
                            trimmed,
                            "{shape:?} r{r} {distance}"
                        );
                    }
                }
            }
        }
        // A small offset of a convex corner takes the fast path on both sides.
        for shape in [CornerShape::Round, CornerShape::Squircle, CornerShape::Bevel] {
            let line = corner_line(shape, 100., 1.);
            assert!(raw_offset(&line, 1.).1 && raw_offset(&line, -1.).1, "{shape:?}");
        }
    }
}
