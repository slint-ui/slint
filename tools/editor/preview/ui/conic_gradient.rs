// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{FillData, LinearGradientAxis};
use slint::{LogicalPosition as Point, LogicalSize};

pub fn editing_axis(fill: FillData, size: LogicalSize) -> LinearGradientAxis {
    let start = if fill.custom_center {
        Point::new(fill.center_x, fill.center_y)
    } else {
        Point::new(size.width / 2., size.height / 2.)
    };
    let radius = (size.width.min(size.height) * 0.63).clamp(48., 160.);
    let angle = (fill.angle - 90.).to_radians();
    LinearGradientAxis {
        start,
        end: Point::new(start.x + radius * angle.cos(), start.y + radius * angle.sin()),
    }
}

pub fn point(axis: LinearGradientAxis, position: f32) -> Point {
    let (sin, cos) = (position * std::f32::consts::TAU).sin_cos();
    let x = axis.end.x - axis.start.x;
    let y = axis.end.y - axis.start.y;
    Point::new(axis.start.x + x * cos - y * sin, axis.start.y + x * sin + y * cos)
}

pub fn angular_delta(center: Point, previous: Point, next: Point) -> f32 {
    let a = Point::new(previous.x - center.x, previous.y - center.y);
    let b = Point::new(next.x - center.x, next.y - center.y);
    if !a.x.is_finite()
        || !a.y.is_finite()
        || !b.x.is_finite()
        || !b.y.is_finite()
        || a.x * a.x + a.y * a.y < 1.
        || b.x * b.x + b.y * b.y < 1.
    {
        return 0.;
    }
    (a.x * b.y - a.y * b.x).atan2(a.x * b.x + a.y * b.y).to_degrees()
}

pub fn position(axis: LinearGradientAxis, p: Point) -> f32 {
    angular_delta(axis.start, axis.end, p).rem_euclid(360.) / 360.
}

pub fn remap(
    mut fill: FillData,
    previous: LinearGradientAxis,
    next: LinearGradientAxis,
) -> FillData {
    if !next.start.x.is_finite()
        || !next.start.y.is_finite()
        || !next.end.x.is_finite()
        || !next.end.y.is_finite()
        || (next.end.x - next.start.x).powi(2) + (next.end.y - next.start.y).powi(2) < 1.
    {
        return fill;
    }
    let translation = Point::new(next.start.x - previous.start.x, next.start.y - previous.start.y);
    if translation.x.abs() > 0.0001 || translation.y.abs() > 0.0001 {
        fill.custom_center = true;
        fill.center_x = next.start.x;
        fill.center_y = next.start.y;
    }
    let delta = angular_delta(
        previous.start,
        previous.end,
        Point::new(next.end.x - translation.x, next.end.y - translation.y),
    );
    if delta.abs() > 0.0001 {
        fill.angle += delta;
    }
    fill
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::ui::{BrushKind, GradientStop};
    use slint::{Color, Model, VecModel};
    use std::rc::Rc;

    fn fill(angle: f32) -> FillData {
        FillData {
            kind: BrushKind::Conic,
            angle,
            stops: Rc::new(VecModel::from(vec![
                GradientStop { position: -0.1, color: Color::from_rgb_u8(255, 0, 0) },
                GradientStop { position: 0., color: Color::from_rgb_u8(255, 0, 0) },
                GradientStop { position: 0.5, color: Color::from_argb_u8(128, 0, 255, 0) },
                GradientStop { position: 0.5, color: Color::from_rgb_u8(0, 0, 255) },
                GradientStop { position: 1., color: Color::from_rgb_u8(0, 0, 255) },
                GradientStop { position: 1.2, color: Color::from_rgb_u8(255, 255, 255) },
            ]))
            .into(),
            ..Default::default()
        }
    }

    fn near(a: Point, b: Point) {
        assert!((a.x - b.x).powi(2) + (a.y - b.y).powi(2) < 0.00001, "{a:?} != {b:?}");
    }

    #[test]
    fn cardinal_angles_follow_css_clockwise_from_north() {
        for size in [LogicalSize::new(200., 200.), LogicalSize::new(320., 120.)] {
            let c = Point::new(size.width / 2., size.height / 2.);
            let r = size.width.min(size.height) * 0.63;
            for (angle, dx, dy) in [(0., 0., -r), (90., r, 0.), (180., 0., r), (270., -r, 0.)] {
                let axis = editing_axis(fill(angle), size);
                near(axis.start, c);
                near(axis.end, Point::new(c.x + dx, c.y + dy));
                for t in [0., 0.1, 0.25, 0.5, 0.9] {
                    assert!((position(axis.clone(), point(axis.clone(), t)) - t).abs() < 0.00001);
                }
                near(point(axis.clone(), 0.), point(axis.clone(), 1.));
            }
        }
    }

    #[test]
    fn seam_crossings_use_small_signed_deltas() {
        let size = LogicalSize::new(200., 200.);
        let a = editing_axis(fill(359.), size);
        let b = editing_axis(fill(1.), size);
        assert!((angular_delta(a.start, a.end, b.end) - 2.).abs() < 0.0001);
        assert!((angular_delta(a.start, b.end, a.end) + 2.).abs() < 0.0001);
        let result = remap(fill(359.), a.clone(), b);
        assert!((result.angle - 361.).abs() < 0.0001);
        assert!(!result.custom_center);
        assert_eq!(angular_delta(a.start, a.end, a.start), 0.);
        assert_eq!(angular_delta(a.start, a.end, Point::new(f32::NAN, 0.)), 0.);
    }

    #[test]
    fn translation_preserves_angle_stops_and_automatic_radius() {
        let original = fill(450.);
        let size = LogicalSize::new(200., 200.);
        let axis = editing_axis(original.clone(), size);
        let next = LinearGradientAxis {
            start: Point::new(axis.start.x - 75., axis.start.y + 23.),
            end: Point::new(axis.end.x - 75., axis.end.y + 23.),
        };
        let result = remap(original.clone(), axis, next.clone());
        assert!(result.custom_center);
        assert_eq!((result.center_x, result.center_y, result.angle), (25., 123., 450.));
        assert!(!result.custom_radius);
        assert_eq!(
            original.stops.iter().collect::<Vec<_>>(),
            result.stops.iter().collect::<Vec<_>>()
        );
        let reopened = editing_axis(result.clone(), size);
        near(next.start, reopened.start);
        near(next.end, reopened.end);
        assert!(
            super::super::brushes::fill_expression(result).contains("from 450deg at 25px 123px")
        );
    }

    #[test]
    fn guide_size_and_noops_do_not_change_the_brush() {
        let original = fill(-720.);
        let axis = editing_axis(original.clone(), LogicalSize::new(200., 200.));
        for end in [
            axis.end,
            axis.start,
            Point::new(
                axis.start.x + (axis.end.x - axis.start.x) * 2.,
                axis.start.y + (axis.end.y - axis.start.y) * 2.,
            ),
            Point::new(f32::NAN, 0.),
        ] {
            let next = LinearGradientAxis { start: axis.start, end };
            assert_eq!(
                super::super::brushes::fill_brush(remap(original.clone(), axis.clone(), next)),
                super::super::brushes::fill_brush(original.clone())
            );
        }
    }
}
