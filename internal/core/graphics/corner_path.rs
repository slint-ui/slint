// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Béziers

/*!
Paths of rectangles whose corners have a [`CornerShape`], for renderers that draw paths.
*/

#[cfg(doc)]
use super::border_radius::CornerShape;
use super::corner_geometry::{Contour, ContourVertex, TOLERANCE};
use lyon_path::math::{Point, Vector};
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

/// Receives the segments of a path, so that a renderer builds its own path type directly.
pub trait PathSink {
    /// The renderer's path type.
    type Path;
    /// Starts a new subpath at `to`.
    fn move_to(&mut self, to: Point);
    /// Appends a straight segment to `to`.
    fn line_to(&mut self, to: Point);
    /// Appends a cubic Bézier curve to `to`.
    fn cubic_to(&mut self, ctrl1: Point, ctrl2: Point, to: Point);
    /// Closes the current subpath.
    fn close(&mut self);
    /// The path of the segments received so far.
    fn build(self) -> Self::Path;
}

/// The path of `contour`.
pub fn contour_path<S: PathSink + Default>(contour: &Contour) -> S::Path {
    let mut sink = S::default();
    add_contour(&mut sink, contour);
    sink.build()
}

/// Appends `contour` to `sink`.
pub fn add_contour(sink: &mut impl PathSink, contour: &Contour) {
    match contour {
        Contour::Curved(vertices) => {
            let Some(first) = vertices.first() else { return };
            sink.move_to(first.end());
            for (from, v) in vertices.iter().zip(&vertices[1..]) {
                segment(sink, from.end(), *v, v.end());
            }
            sink.close();
        }
        Contour::Polygons(polygons) => {
            for polygon in polygons {
                add_polygon(sink, polygon.iter().copied());
            }
        }
    }
}

fn add_polygon(sink: &mut impl PathSink, mut points: impl Iterator<Item = Point>) {
    let Some(first) = points.next() else { return };
    sink.move_to(first);
    for p in points {
        sink.line_to(p);
    }
    sink.close();
}

/// Appends the segment that `vertex` ends, from `from` to `to`.
fn segment(sink: &mut impl PathSink, from: Point, vertex: ContourVertex, to: Point) {
    match vertex {
        ContourVertex::Line(_) => sink.line_to(to),
        ContourVertex::Arc { center, .. } => arc_to(sink, from, center, to),
    }
}

/// Appends the arc from `from`, the current point, around `center` to `to`, turning less
/// than half a turn, as cubic Béziers within [`TOLERANCE`] of the circle.
fn arc_to(sink: &mut impl PathSink, from: Point, center: Point, to: Point) {
    let (start, end) = (from - center, to - center);
    let sweep = start.cross(end).atan2(start.dot(end));
    // A cubic strays up to `2.7253e-4 * radius` from a quarter circle, with the error
    // scaling as the sixth power of the sweep.
    let quarters = sweep.abs() / core::f32::consts::FRAC_PI_2;
    let per_quarter = (2.7253e-4 * start.length() / TOLERANCE).powf(1. / 6.);
    let segments = (quarters * per_quarter).ceil().max(1.) as u32;
    let step = sweep / segments as f32;
    let handle = 4. / 3. * (step / 4.).tan();
    let tangent = |v: Vector| Vector::new(-v.y, v.x) * handle;
    let mut previous = start;
    for i in 1..=segments {
        let next = if i == segments {
            end
        } else {
            let (sin, cos) = (step * i as f32).sin_cos();
            Vector::new(start.x * cos - start.y * sin, start.x * sin + start.y * cos)
        };
        sink.cubic_to(
            center + previous + tangent(previous),
            center + next - tangent(next),
            center + next,
        );
        previous = next;
    }
}

#[cfg(test)]
mod tests {
    use super::super::BorderRadius;
    use super::*;
    use alloc::vec::Vec;
    use lyon_path::iterator::PathIterator;

    use super::super::border_radius::{CornerShape, CornerShapes};
    use super::super::corner_geometry::tests::{SHAPES, distance_to_polyline};
    use super::super::corner_geometry::{
        TOLERANCE, offset_outline, superellipse_at, superellipse_n,
    };

    struct LyonSink(lyon_path::path::Builder);

    impl Default for LyonSink {
        fn default() -> Self {
            Self(lyon_path::Path::builder())
        }
    }

    impl PathSink for LyonSink {
        type Path = lyon_path::Path;
        fn move_to(&mut self, to: Point) {
            self.0.begin(to);
        }
        fn line_to(&mut self, to: Point) {
            self.0.line_to(to);
        }
        fn cubic_to(&mut self, ctrl1: Point, ctrl2: Point, to: Point) {
            self.0.cubic_bezier_to(ctrl1, ctrl2, to);
        }
        fn close(&mut self) {
            self.0.end(true);
        }
        fn build(self) -> lyon_path::Path {
            self.0.build()
        }
    }

    fn outline_path(
        rect: euclid::default::Rect<f32>,
        radius: BorderRadius<f32, ()>,
        shapes: CornerShapes,
        spread: f32,
    ) -> lyon_path::Path {
        contour_path::<LyonSink>(&offset_outline(rect, radius, shapes, spread))
    }

    /// The outline's vertices, flattened, in the corner frame of the top-right corner of a `3r` square,
    /// keeping those that belong to that corner, in order, extended along the straight edges
    /// that the corner joins.
    fn outline_corner(shape: CornerShape, r: f32) -> Vec<Point> {
        // Corners meeting along a side leave a sliver of no area, which the outline drops.
        let rect = euclid::default::Rect::new(Point::zero(), euclid::size2(3. * r, 3. * r));
        let path =
            outline_path(rect, BorderRadius::new_uniform(r), CornerShapes::new_uniform(shape), 0.);
        let corner = path
            .iter()
            .flattened(1e-3)
            .filter_map(|event| match event {
                lyon_path::Event::Begin { at } | lyon_path::Event::Line { to: at, .. } => {
                    Some(Point::new(3. * r - at.x, at.y))
                }
                _ => None,
            })
            .filter(|p| p.x >= -0.001 && p.x <= r + 0.001 && p.y >= -0.001 && p.y <= r + 0.001);
        core::iter::once(Point::new(2. * r, 0.))
            .chain(corner)
            .chain(core::iter::once(Point::new(0., 2. * r)))
            .collect()
    }

    #[test]
    fn outline_matches_the_shape() {
        // Slack for the `f32` rounding of the samples.
        let tolerance = TOLERANCE + 1e-3;
        for shape in SHAPES {
            let k = shape.superellipse_k();
            if !k.is_finite() || k == 0. {
                // `Square`, `Notch`, and `Bevel` are straight, so their outline is exact.
                continue;
            }
            let n = superellipse_n(k);
            for r in [4f32, 16., 64., 256.] {
                let outline = outline_corner(shape, r);
                const SAMPLES: u32 = 20_000;
                let deviation = (0..=SAMPLES)
                    .map(|i| {
                        let on_curve = superellipse_at(k, n, i as f32 / SAMPLES as f32).0 * r;
                        distance_to_polyline(on_curve.to_point(), &outline)
                    })
                    .fold(0f32, f32::max);
                assert!(
                    deviation <= tolerance,
                    "{shape:?} at r={r}: deviation {deviation} exceeds {tolerance}"
                );
            }
        }
    }

    /// An arc's cubics stay within the tolerance of its circle, whatever its radius.
    #[test]
    fn arcs_stay_on_their_circle() {
        let center = Point::new(3., -2.);
        for radius in [0.5f32, 10., 180., 200., 1e3, 1e5] {
            for (a0, a1) in [(0f32, 90f32), (90., 0.), (-30., 100.), (10., 15.)] {
                let at = |degrees: f32| {
                    let (sin, cos) = degrees.to_radians().sin_cos();
                    center + Vector::new(cos, sin) * radius
                };
                let mut b = LyonSink::default();
                b.move_to(at(a0));
                arc_to(&mut b, at(a0), center, at(a1));
                b.0.end(false);
                // f32 rounding alone reaches the tolerance at the largest radius.
                let center = center.cast::<f64>();
                for event in b.build().iter() {
                    let lyon_path::Event::Cubic { from, ctrl1, ctrl2, to } = event else {
                        continue;
                    };
                    let cubic = lyon_path::geom::CubicBezierSegment {
                        from: from.cast::<f64>(),
                        ctrl1: ctrl1.cast(),
                        ctrl2: ctrl2.cast(),
                        to: to.cast(),
                    };
                    for i in 0..=64 {
                        let p = cubic.sample(f64::from(i) / 64.);
                        let error = ((p - center).length() - f64::from(radius)).abs();
                        assert!(
                            error <= f64::from(TOLERANCE),
                            "r={radius} {a0}..{a1}: {p:?} is {error} off"
                        );
                    }
                }
            }
        }
    }
}
