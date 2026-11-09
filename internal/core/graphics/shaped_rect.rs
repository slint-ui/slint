// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore superellipses, Béziers, underflows

/*!
The outline of a rectangle whose corners have a [`CornerShape`], shared by every renderer.

Corners are computed in the *corner frame*: the corner's outer tip at the origin, `x` running
along the incoming edge and `y` along the outgoing one, so the rectangle lies in `x, y >= 0`.
*/

use super::border_radius::{BorderRadius, CornerShape, CornerShapes};
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

type Point = euclid::default::Point2D<f32>;
type Vector = euclid::default::Vector2D<f32>;

/// Larger `|k|` is clamped to this: `2^16` is indistinguishable from `Square` or `Notch`,
/// and an unclamped `n = 2^|k|` overflows.
const MAX_SUPERELLIPSE_K: f32 = 16.0;

/// Largest distance, in the units of the rect, between a cubic and the curve it approximates.
const TOLERANCE: f32 = 0.05;

/// Deepest a curve is halved while fitting cubics, bounding the cost of a huge radius.
const MAX_DEPTH: u32 = 10;

/// Receives the segments of a path, so that a renderer builds its own path type directly.
pub trait PathSink {
    /// Starts a new subpath at `to`.
    fn move_to(&mut self, to: Point);
    /// Appends a straight segment to `to`.
    fn line_to(&mut self, to: Point);
    /// Appends a cubic Bézier curve to `to`.
    fn cubic_to(&mut self, ctrl1: Point, ctrl2: Point, to: Point);
    /// Closes the current subpath.
    fn close(&mut self);
}

/// A rectangle whose corners have radii and [`CornerShape`]s, grown by `offset`, or shrunk
/// for a negative `offset`.
///
/// A renderer draws it as the rounded rectangle [`Self::rounded_rect`] when
/// [`Self::is_rounded`].
/// Otherwise, it builds the outline with [`Self::add_outline`], and offsets it with a stroke
/// of width `2 * offset.abs()` and miter joins: it unites the outline with the stroke to grow
/// it, and subtracts the stroke to shrink it.
pub struct ShapedRect<U> {
    /// The rectangle, before the offset.
    pub rect: euclid::Rect<f32, U>,
    /// The corner radii, before the offset.
    pub radius: BorderRadius<f32, U>,
    /// The shape of each corner.
    pub shape: CornerShapes,
    /// How far the outline moves outward, or inward when negative.
    pub offset: f32,
}

impl<U> Clone for ShapedRect<U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<U> Copy for ShapedRect<U> {}

impl<U> PartialEq for ShapedRect<U> {
    fn eq(&self, other: &Self) -> bool {
        self.rect == other.rect
            && self.radius == other.radius
            && self.shape == other.shape
            && self.offset == other.offset
    }
}

impl<U> core::fmt::Debug for ShapedRect<U> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShapedRect")
            .field("rect", &self.rect)
            .field("radius", &self.radius)
            .field("shape", &self.shape)
            .field("offset", &self.offset)
            .finish()
    }
}

impl<U> ShapedRect<U> {
    /// Returns `rect` with corners of `radius` and `shape`, grown by `offset`.
    pub fn new(
        rect: euclid::Rect<f32, U>,
        radius: BorderRadius<f32, U>,
        shape: CornerShapes,
        offset: f32,
    ) -> Self {
        Self { rect, radius, shape, offset }
    }

    /// Returns a copy grown by `offset` more.
    pub fn grown(self, offset: f32) -> Self {
        Self { offset: self.offset + offset, ..self }
    }

    /// Returns a copy in the units of `scale`.
    pub fn scale<U2>(self, scale: euclid::Scale<f32, U, U2>) -> ShapedRect<U2> {
        ShapedRect {
            rect: self.rect * scale,
            radius: self.radius * scale,
            shape: self.shape,
            offset: self.offset * scale.get(),
        }
    }

    /// Whether every corner is round or sharp, so that [`Self::rounded_rect`] draws it.
    pub fn is_rounded(&self) -> bool {
        self.shape.rounded_rect_radius(self.radius).is_some()
    }

    /// The rounded rectangle that draws this outline when [`Self::is_rounded`], or that
    /// approximates it with round corners otherwise.
    pub fn rounded_rect(&self) -> (euclid::Rect<f32, U>, BorderRadius<f32, U>) {
        let radius = self.shape.rounded_rect_radius(self.radius).unwrap_or(self.radius);
        let grown = self.rect.inflate(self.offset, self.offset);
        (
            euclid::Rect::new(grown.origin, grown.size.max(euclid::Size2D::zero())),
            radius.spread(self.offset),
        )
    }

    /// Appends the outline, before the offset, as one closed clockwise subpath.
    pub fn add_outline(&self, sink: &mut impl PathSink) {
        let rect = self.rect.to_untyped();
        let radius = fit_radius(rect, self.radius, self.shape);
        let corners = corners_in_path_order(radius, self.shape);
        for (i, ((tip, e_in, e_out), (r, shape))) in
            corner_frames(rect).into_iter().zip(corners).enumerate()
        {
            let to_rect = |p: Point| tip + e_in * p.x + e_out * p.y;
            let start = to_rect(if r > 0. { Point::new(r, 0.) } else { Point::zero() });
            if i == 0 {
                sink.move_to(start);
            } else {
                sink.line_to(start);
            }
            add_corner(sink, shape, r, to_rect);
        }
        sink.close();
    }
}

/// The superellipse exponent `n = 2^|k|`.
///
/// `k`'s sign selects the convex or the concave branch, not the exponent.
/// See [`superellipse_at`].
fn superellipse_n(k: f32) -> f32 {
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

/// The `s` of [`superellipse_at`] whose ray passes through `point`, in the corner frame and
/// in units of the radius, or `None` when no ray does.
fn superellipse_s(k: f32, point: Vector) -> Option<f32> {
    let (p, q) = if k < 0. { (point.y, point.x) } else { (1. - point.x, 1. - point.y) };
    (p >= 0. && q >= 0. && p + q > 0.).then(|| p / (p + q))
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
fn superellipse_at(k: f32, n: f32, s: f32) -> (Vector, Vector) {
    let (p, q) = superellipse_pq(n, s);
    // Perpendicular to the implicit gradient, which stays finite where the slope doesn't.
    let (gp, gq) = (p.powf(n - 1.), q.powf(n - 1.));
    let tangent = if k < 0. { Vector::new(-gp, gq) } else { Vector::new(-gq, gp) };
    (superellipse_corner(k, p, q), tangent.normalize())
}

/// Appends the corner of `shape` and radius `r` from `(r, 0)`, the current point, to
/// `(0, r)`, mapping the corner frame to the rect with `to_rect`.
fn add_corner(
    sink: &mut impl PathSink,
    shape: CornerShape,
    r: f32,
    to_rect: impl Fn(Point) -> Point,
) {
    if r <= 0. {
        return;
    }
    let k = shape.superellipse_k();
    match k {
        f32::INFINITY => sink.line_to(to_rect(Point::zero())),
        f32::NEG_INFINITY => {
            sink.line_to(to_rect(Point::new(r, r)));
            sink.line_to(to_rect(Point::new(0., r)));
        }
        0. => sink.line_to(to_rect(Point::new(0., r))),
        _ => {
            let n = superellipse_n(k);
            let curve = Superellipse { k, n, r };
            curve.add_span(
                sink,
                &to_rect,
                (0., superellipse_at(k, n, 0.)),
                (1., superellipse_at(k, n, 1.)),
                MAX_DEPTH,
            );
        }
    }
}

/// A superellipse corner of radius `r`, fitted with cubics.
struct Superellipse {
    k: f32,
    n: f32,
    r: f32,
}

impl Superellipse {
    /// Appends the curve from `s0` to `s1`, halving the span until one cubic is within
    /// [`TOLERANCE`] of it.
    fn add_span(
        &self,
        sink: &mut impl PathSink,
        to_rect: &impl Fn(Point) -> Point,
        (s0, (p0, t0)): (f32, (Vector, Vector)),
        (s1, (p1, t1)): (f32, (Vector, Vector)),
        depth: u32,
    ) {
        let (start, end) = ((p0 * self.r).to_point(), (p1 * self.r).to_point());
        // The handle length that makes the cubic a circular arc between these tangents,
        // which also suits curves whose curvature changes slowly.
        let cos_half_turn = ((1. + t0.dot(t1).clamp(-1., 1.)) / 2.).sqrt();
        let handle = 2. * (end - start).length() / (3. * (1. + cos_half_turn));
        let (ctrl1, ctrl2) = (start + t0 * handle, end - t1 * handle);
        if depth == 0 || self.fits(start, ctrl1, ctrl2, end) {
            sink.cubic_to(to_rect(ctrl1), to_rect(ctrl2), to_rect(end));
            return;
        }
        let s = (s0 + s1) / 2.;
        let middle = (s, superellipse_at(self.k, self.n, s));
        self.add_span(sink, to_rect, (s0, (p0, t0)), middle, depth - 1);
        self.add_span(sink, to_rect, middle, (s1, (p1, t1)), depth - 1);
    }

    /// Whether the cubic stays within [`TOLERANCE`] of the curve, sampled at every eighth.
    fn fits(&self, p0: Point, c1: Point, c2: Point, p1: Point) -> bool {
        (1..8).map(|i| i as f32 / 8.).all(|t| {
            let u = 1. - t;
            let point = (p0.to_vector() * (u * u * u)
                + c1.to_vector() * (3. * u * u * t)
                + c2.to_vector() * (3. * u * t * t)
                + p1.to_vector() * (t * t * t))
                / self.r;
            superellipse_s(self.k, point).is_some_and(|s| {
                let on_curve = superellipse_at(self.k, self.n, s).0;
                (point - on_curve).length() * self.r <= TOLERANCE
            })
        })
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
/// radius, and where adjacent corners would overlap along a side, all radii are scaled down
/// by one shared factor, as for a rounded rectangle.
/// See <https://drafts.csswg.org/css-backgrounds-3/#corner-overlap>.
fn fit_radius<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    shapes: CornerShapes,
) -> BorderRadius<f32, U> {
    let (width, height) = (rect.width().max(0.), rect.height().max(0.));
    // Clamping first keeps an infinite radius from scaling to `inf * 0`.
    let corner = |r: f32, shape: CornerShape| {
        if shape == CornerShape::Square { 0. } else { r.max(0.).min(width.max(height)) }
    };
    let r = BorderRadius::<f32, U>::new(
        corner(radius.top_left, shapes.top_left()),
        corner(radius.top_right, shapes.top_right()),
        corner(radius.bottom_right, shapes.bottom_right()),
        corner(radius.bottom_left, shapes.bottom_left()),
    );
    let adjacent = [
        (width, r.top_left + r.top_right),
        (width, r.bottom_left + r.bottom_right),
        (height, r.top_left + r.bottom_left),
        (height, r.top_right + r.bottom_right),
    ]
    .into_iter()
    .filter(|&(_, sum)| sum > 0.)
    .fold(1f32, |scale, (side, sum)| scale.min(side / sum));
    r * adjacent
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const SHAPES: [CornerShape; 14] = [
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

    /// The outline as polylines, one per subpath, with each cubic sampled finely.
    #[derive(Default)]
    struct Sampler {
        polylines: Vec<Vec<Point>>,
        cubics: usize,
        closed: usize,
    }

    impl PathSink for Sampler {
        fn move_to(&mut self, to: Point) {
            self.polylines.push(alloc::vec![to]);
        }
        fn line_to(&mut self, to: Point) {
            self.polylines.last_mut().unwrap().push(to);
        }
        fn cubic_to(&mut self, c1: Point, c2: Point, to: Point) {
            self.cubics += 1;
            let line = self.polylines.last_mut().unwrap();
            let from = *line.last().unwrap();
            line.extend((1..=32).map(|i| {
                let t = i as f32 / 32.;
                let u = 1. - t;
                (from.to_vector() * (u * u * u)
                    + c1.to_vector() * (3. * u * u * t)
                    + c2.to_vector() * (3. * u * t * t)
                    + to.to_vector() * (t * t * t))
                    .to_point()
            }));
        }
        fn close(&mut self) {
            self.closed += 1;
        }
    }

    /// `shape`'s curve in the corner frame and in units of the radius, densely sampled.
    fn dense_curve(shape: CornerShape) -> Vec<Point> {
        match shape {
            CornerShape::Notch => [(1., 0.), (1., 1.), (0., 1.)].map(Point::from).into(),
            CornerShape::Bevel => [(1., 0.), (0., 1.)].map(Point::from).into(),
            _ => {
                let k = shape.superellipse_k();
                let n = superellipse_n(k);
                (0..=20000).map(|i| superellipse_at(k, n, i as f32 / 20000.).0.to_point()).collect()
            }
        }
    }

    fn distance_to_polyline(p: Point, line: &[Point]) -> f32 {
        line.windows(2)
            .map(|w| {
                let d = w[1] - w[0];
                let t =
                    ((p - w[0]).dot(d) / d.square_length().max(f32::MIN_POSITIVE)).clamp(0., 1.);
                (p - (w[0] + d * t)).length()
            })
            .fold(f32::INFINITY, f32::min)
    }

    fn sample(shape: CornerShapes, radius: f32, size: f32) -> Sampler {
        let mut sampler = Sampler::default();
        let rect = euclid::default::Rect::new(Point::new(10., 20.), euclid::size2(size, size));
        ShapedRect::new(rect, BorderRadius::new_uniform(radius), shape, 0.)
            .add_outline(&mut sampler);
        sampler
    }

    /// Every sampled point of the top right corner lies within the tolerance of its curve.
    #[test]
    fn outline_follows_the_curve() {
        for shape in SHAPES {
            for r in [3f32, 40., 300.] {
                let sampler = sample(CornerShapes::new_uniform(shape), r, 4. * r);
                assert_eq!((sampler.polylines.len(), sampler.closed), (1, 1));
                let tip = Point::new(10. + 4. * r, 20.);
                let corner: Vec<_> = sampler.polylines[0]
                    .iter()
                    .map(|p| Point::new(tip.x - p.x, p.y - tip.y) / r)
                    .filter(|p| p.x <= 1. + 1e-4 && p.y <= 1. + 1e-4)
                    .collect();
                assert!(!corner.is_empty(), "{shape:?} r={r}");
                if CornerShape::Square == shape {
                    assert!(corner.iter().all(|p| p.to_vector().length() < 1e-4));
                    continue;
                }
                let curve = dense_curve(shape);
                for p in corner {
                    let error = distance_to_polyline(p, &curve) * r;
                    assert!(error <= TOLERANCE * 1.01, "{shape:?} r={r}: {p:?} is {error} off");
                }
            }
        }
    }

    /// `Round` takes a single cubic for a small radius, like a rounded rectangle.
    #[test]
    fn round_corner_is_one_cubic() {
        assert_eq!(sample(CornerShapes::default(), 20., 100.).cubics, 4);
    }

    /// A huge radius takes a bounded number of cubics.
    #[test]
    fn huge_radius_is_bounded() {
        for shape in [
            CornerShape::Round,
            CornerShape::Squircle,
            CornerShape::Superellipse(-3.5),
            CornerShape::Superellipse(12.),
        ] {
            let sampler = sample(CornerShapes::new_uniform(shape), 1e9, 2e9);
            assert!(sampler.cubics <= 4 << MAX_DEPTH, "{shape:?}: {}", sampler.cubics);
        }
    }

    /// An infinite radius fills the rect instead of producing NaN.
    #[test]
    fn infinite_radius_stays_finite() {
        for shape in SHAPES {
            let sampler = sample(CornerShapes::new_uniform(shape), f32::INFINITY, 50.);
            assert!(
                sampler.polylines[0].iter().all(|p| p.x.is_finite() && p.y.is_finite()),
                "{shape:?}"
            );
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

    #[test]
    fn rounded_rect_offsets_like_a_shadow_spread() {
        let rect = euclid::default::Rect::new(Point::new(0., 0.), euclid::size2(40., 20.));
        let radius = BorderRadius::<f32, euclid::UnknownUnit>::new(8., 0., 8., 0.);
        let shape = CornerShapes::new(
            CornerShape::Round,
            CornerShape::Square,
            CornerShape::Square,
            CornerShape::Round,
        );
        let shaped = ShapedRect::new(rect, radius, shape, -2.);
        assert!(shaped.is_rounded());
        assert_eq!(
            shaped.rounded_rect(),
            (euclid::rect(2., 2., 36., 16.), BorderRadius::new(6., 0., 0., 0.))
        );
        let shrunk_away = ShapedRect::new(rect, radius, shape, -15.).rounded_rect();
        assert_eq!(shrunk_away.0.size, euclid::size2(10., 0.));
        assert!(
            !ShapedRect { shape: CornerShapes::new_uniform(CornerShape::Bevel), ..shaped }
                .is_rounded()
        );
    }
}
