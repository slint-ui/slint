// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore ungrown, Béziers

/*!
Paths of rectangles whose corners have a [`CornerShape`], for renderers that draw paths.
Renderers keep drawing a rectangle whose corners are all round with their own rounded
rectangle primitive; see [`CornerShapes::rounded_rect_radius`].
*/

use super::BorderRadius;
#[cfg(doc)]
use super::border_radius::CornerShape;
use super::border_radius::CornerShapes;
use super::corner_geometry::{BorderContours, Contour, ContourVertex, TOLERANCE, offset_outline};
use lyon_path::math::{Point, Vector};
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

/// Builds the outline of `rect` with per-corner `radius` and `corner_shape`.
pub fn outline_path<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    corner_shape: CornerShapes,
) -> lyon_path::Path {
    spread_outline_path(rect, radius, corner_shape, 0.)
}

/// Builds the outline of a rectangle with per-corner `radius` and `corner_shape`, grown
/// outward by `spread` to `rect`.
/// A negative `spread` shrinks it instead, for an inner shadow's hole.
///
/// Each point of the path is `spread` away from the ungrown outline, except at a miter.
/// Only for `Round` corners does [`outline_path`] at a scaled-up radius give the same.
pub fn spread_outline_path<U>(
    rect: euclid::default::Rect<f32>,
    radius: BorderRadius<f32, U>,
    corner_shape: CornerShapes,
    spread: f32,
) -> lyon_path::Path {
    contour_path(&offset_outline(rect.inflate(-spread, -spread), radius, corner_shape, spread))
}

/// Builds the path of `contour`.
pub fn contour_path(contour: &Contour) -> lyon_path::Path {
    let mut b = lyon_path::Path::builder();
    add_contour(&mut b, contour, false);
    b.build()
}

/// Builds the path of a border ring from its [`BorderContours`], with the inner contour
/// reversed so that filling with the nonzero rule paints only the ring.
pub fn border_path(contours: &BorderContours) -> lyon_path::Path {
    let mut b = lyon_path::Path::builder();
    add_contour(&mut b, &contours.outer, false);
    add_contour(&mut b, &contours.inner, true);
    b.build()
}

fn add_contour(b: &mut lyon_path::path::Builder, contour: &Contour, reversed: bool) {
    match contour {
        Contour::Curved(vertices) => {
            let (Some(first), Some(last)) = (vertices.first(), vertices.last()) else { return };
            if reversed {
                // Each vertex ends a segment, so walking back, a vertex's segment ends at the
                // vertex before it.
                b.begin(last.end());
                for (to, v) in vertices.iter().zip(&vertices[1..]).rev() {
                    segment(b, v.end(), *v, to.end());
                }
            } else {
                b.begin(first.end());
                for (from, v) in vertices.iter().zip(&vertices[1..]) {
                    segment(b, from.end(), *v, v.end());
                }
            }
            b.end(true);
        }
        Contour::Polygons(polygons) => {
            for polygon in polygons {
                if reversed {
                    add_polygon(b, polygon.iter().rev().copied());
                } else {
                    add_polygon(b, polygon.iter().copied());
                }
            }
        }
    }
}

fn add_polygon(b: &mut lyon_path::path::Builder, mut points: impl Iterator<Item = Point>) {
    let Some(first) = points.next() else { return };
    b.begin(first);
    for p in points {
        b.line_to(p);
    }
    b.end(true);
}

/// Appends the segment that `vertex` ends, from `from` to `to`, which run backward when
/// walking the contour backward.
fn segment(b: &mut lyon_path::path::Builder, from: Point, vertex: ContourVertex, to: Point) {
    match vertex {
        ContourVertex::Line(_) => {
            b.line_to(to);
        }
        ContourVertex::Arc { center, .. } => arc_to(b, from, center, to),
    }
}

/// Appends the arc from `from`, the current point, around `center` to `to`, turning less
/// than half a turn, as cubic Béziers within [`TOLERANCE`] of the circle.
fn arc_to(b: &mut lyon_path::path::Builder, from: Point, center: Point, to: Point) {
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
        b.cubic_bezier_to(
            center + previous + tangent(previous),
            center + next - tangent(next),
            center + next,
        );
        previous = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use lyon_path::iterator::PathIterator;

    use super::super::border_radius::CornerShape;
    use super::super::corner_geometry::{
        TOLERANCE, superellipse_at, superellipse_k, superellipse_n,
    };

    /// The outline's vertices, flattened, in the corner frame of the top-right corner of a `3r` square,
    /// keeping those that belong to that corner, in order, extended along the straight edges
    /// that the corner joins.
    fn outline_corner(shape: CornerShape, r: f32) -> Vec<Point> {
        // Corners meeting along a side leave a sliver of no area, which the outline drops.
        let rect = euclid::default::Rect::new(Point::zero(), euclid::size2(3. * r, 3. * r));
        let path = outline_path::<()>(
            rect,
            BorderRadius::new_uniform(r),
            CornerShapes::new_uniform(shape),
        );
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
        let shapes = [
            CornerShape::Square,
            CornerShape::Notch,
            CornerShape::Bevel,
            CornerShape::Round,
            CornerShape::Scoop,
            CornerShape::Squircle,
            CornerShape::Superellipse(-4.),
            CornerShape::Superellipse(-1.),
            CornerShape::Superellipse(-0.5),
            CornerShape::Superellipse(0.),
            CornerShape::Superellipse(0.5),
            CornerShape::Superellipse(3.),
            CornerShape::Superellipse(6.),
            CornerShape::Superellipse(f32::NAN),
        ];
        // Slack for the `f32` rounding of the samples.
        let tolerance = TOLERANCE + 1e-3;
        for shape in shapes {
            let k = superellipse_k(shape);
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

    /// Flattens `path` into a polyline within `tolerance` of it.
    fn flatten(path: &lyon_path::Path, tolerance: f32) -> Vec<Point> {
        path.iter()
            .flattened(tolerance)
            .flat_map(|event| match event {
                lyon_path::Event::Begin { at } => Some(at),
                lyon_path::Event::Line { to, .. } => Some(to),
                lyon_path::Event::End { first, .. } => Some(first),
                _ => None,
            })
            .collect()
    }

    fn distance_to_polyline(p: Point, line: &[Point]) -> f32 {
        line.windows(2)
            .map(|w| lyon_path::geom::LineSegment { from: w[0], to: w[1] }.distance_to_point(p))
            .fold(f32::MAX, f32::min)
    }

    /// Every point of a spread path is `spread` away from the ungrown shape, except at a
    /// miter, where two offset runs meet at a right angle and reach `spread * sqrt(2)`.
    #[test]
    fn spread_path_keeps_its_distance() {
        // Both the reference outline and the spread path are within `TOLERANCE` of their true
        // curves.
        const SLACK: f32 = 2. * TOLERANCE;
        let ungrown = euclid::default::Rect::new(Point::zero(), euclid::size2(200., 160.));
        let shapes = [
            CornerShape::Square,
            CornerShape::Notch,
            CornerShape::Bevel,
            CornerShape::Round,
            CornerShape::Scoop,
            CornerShape::Squircle,
            CornerShape::Superellipse(-4.),
            CornerShape::Superellipse(-0.5),
            CornerShape::Superellipse(0.5),
            CornerShape::Superellipse(3.),
        ];
        for shape in shapes {
            // 79 nearly fills the short side, so its corners almost meet.
            for r in [8f32, 40., 79.] {
                let radius = BorderRadius::<f32, ()>::new_uniform(r);
                let shapes = CornerShapes::new_uniform(shape);
                let reference = flatten(&outline_path(ungrown, radius, shapes), 0.001);
                for spread in [-60f32, -16., -4., 4., 16.] {
                    let path = spread_outline_path(
                        ungrown.inflate(spread, spread),
                        radius,
                        shapes,
                        spread,
                    );
                    for p in flatten(&path, 0.01) {
                        let d = distance_to_polyline(p, &reference);
                        assert!(
                            d >= spread.abs() - SLACK
                                && d <= spread.abs() * core::f32::consts::SQRT_2 + SLACK,
                            "{shape:?} r={r} spread={spread}: {p:?} is {d} from the shape"
                        );
                    }
                }
            }
        }
    }

    /// Each segment of `path` as its points from start to end, including the closing one.
    fn segments(path: &lyon_path::Path) -> Vec<Vec<Point>> {
        path.iter()
            .filter_map(|event| match event {
                lyon_path::Event::Line { from, to } => Some(vec![from, to]),
                lyon_path::Event::Cubic { from, ctrl1, ctrl2, to } => {
                    Some(vec![from, ctrl1, ctrl2, to])
                }
                lyon_path::Event::End { last, first, close: true } => Some(vec![last, first]),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn border_path_reverses_the_inner_contour() {
        let square = |min: f32, max: f32| {
            vec![[(min, min), (max, min), (max, max), (min, max)].map(Point::from).into()]
        };
        let curved = Contour::Curved(vec![
            ContourVertex::Line(Point::new(0., 0.)),
            ContourVertex::Line(Point::new(6., 0.)),
            ContourVertex::Arc { center: Point::new(6., 4.), to: Point::new(10., 4.) },
            ContourVertex::Line(Point::new(10., 10.)),
            ContourVertex::Line(Point::new(0., 10.)),
        ]);
        for inner in [Contour::Polygons(square(2., 8.)), curved] {
            let outer = Contour::Polygons(square(-1., 11.));
            let forward = segments(&contour_path(&inner));
            let ring = segments(&border_path(&BorderContours { outer: outer.clone(), inner }));
            let (outer_segments, inner_segments) =
                ring.split_at(segments(&contour_path(&outer)).len());
            assert_eq!(outer_segments, segments(&contour_path(&outer)));
            assert_eq!(inner_segments.len(), forward.len());
            // Both start at the same vertex, so each path closes with its own last segment.
            let (closing, open) = forward.split_last().unwrap();
            let expected = open.iter().rev().chain(core::iter::once(closing));
            for (reversed, segment) in inner_segments.iter().zip(expected) {
                let close = reversed.len() == segment.len()
                    && reversed
                        .iter()
                        .zip(segment.iter().rev())
                        .all(|(a, b)| (*a - *b).length() < 1e-4);
                assert!(close, "{reversed:?} doesn't reverse {segment:?}");
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
                let mut b = lyon_path::Path::builder();
                b.begin(at(a0));
                arc_to(&mut b, at(a0), center, at(a1));
                b.end(false);
                for p in flatten(&b.build(), radius * 1e-6) {
                    let error = ((p - center).length() - radius).abs();
                    assert!(error <= TOLERANCE, "r={radius} {a0}..{a1}: {p:?} is {error} off");
                }
            }
        }
    }
}
