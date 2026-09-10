// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{FillData, LinearGradientAxis};
use slint::{LogicalPosition as Point, LogicalSize, Model, VecModel};
use std::rc::Rc;

pub fn canonical_axis(angle: f32, size: LogicalSize) -> LinearGradientAxis {
    let (start, end) =
        i_slint_core::graphics::line_for_angle(angle, [size.width, size.height].into());
    LinearGradientAxis { start: Point::new(start.x, start.y), end: Point::new(end.x, end.y) }
}

pub fn position(axis: LinearGradientAxis, point: Point) -> f32 {
    let dx = axis.end.x - axis.start.x;
    let dy = axis.end.y - axis.start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared < f32::EPSILON {
        return 0.;
    }
    ((point.x - axis.start.x) * dx + (point.y - axis.start.y) * dy) / length_squared
}

pub fn point(axis: &LinearGradientAxis, position: f32) -> Point {
    Point::new(
        axis.start.x + (axis.end.x - axis.start.x) * position,
        axis.start.y + (axis.end.y - axis.start.y) * position,
    )
}

pub fn editing_axis(fill: FillData, size: LogicalSize) -> LinearGradientAxis {
    let axis = canonical_axis(fill.angle, size);
    let first = fill.stops.iter().map(|s| s.position).min_by(f32::total_cmp).unwrap_or(0.);
    let last = fill.stops.iter().map(|s| s.position).max_by(f32::total_cmp).unwrap_or(1.);
    let start = point(&axis, first);
    let end = point(&axis, last);
    if (end.x - start.x).hypot(end.y - start.y) < 1. {
        axis
    } else {
        LinearGradientAxis { start, end }
    }
}

pub fn remap(
    mut fill: FillData,
    size: LogicalSize,
    previous: LinearGradientAxis,
    next: LinearGradientAxis,
) -> FillData {
    let dx = next.end.x - next.start.x;
    let dy = next.end.y - next.start.y;
    if !dx.is_finite()
        || !dy.is_finite()
        || dx.hypot(dy) < 1.
        || size.width <= 0.
        || size.height <= 0.
    {
        return fill;
    }
    let old_canonical = canonical_axis(fill.angle, size);
    fill.angle = dx.atan2(-dy).to_degrees().rem_euclid(360.);
    let new_canonical = canonical_axis(fill.angle, size);
    fill.stops = Rc::new(VecModel::from(
        fill.stops
            .iter()
            .map(|mut stop| {
                let relative = position(previous.clone(), point(&old_canonical, stop.position));
                stop.position = position(new_canonical.clone(), point(&next, relative));
                stop
            })
            .collect::<Vec<_>>(),
    ))
    .into();
    fill
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::ui::{BrushKind, GradientStop};

    fn fill() -> FillData {
        FillData {
            kind: BrushKind::Linear,
            angle: 90.,
            stops: Rc::new(VecModel::from(vec![
                GradientStop { position: -0.2, color: slint::Color::from_rgb_u8(255, 0, 0) },
                GradientStop { position: 0.4, color: slint::Color::from_argb_u8(128, 0, 255, 0) },
                GradientStop { position: 0.4, color: slint::Color::from_rgb_u8(0, 0, 255) },
                GradientStop { position: 1.3, color: slint::Color::from_rgb_u8(255, 255, 255) },
            ]))
            .into(),
            ..Default::default()
        }
    }

    #[test]
    fn endpoints_preserve_the_gradient_field() {
        for size in [LogicalSize::new(200., 200.), LogicalSize::new(320., 120.)] {
            for (start, end) in [
                ((40., 100.), (160., 100.)),
                ((100., 20.), (100., 180.)),
                ((180., 180.), (20., 20.)),
                ((-80., 240.), (360., -40.)),
                ((240., 100.), (-40., 100.)),
            ] {
                let original = fill();
                let previous = editing_axis(original.clone(), size);
                let next = LinearGradientAxis {
                    start: Point::new(start.0, start.1),
                    end: Point::new(end.0, end.1),
                };
                let result = remap(original.clone(), size, previous.clone(), next.clone());
                let canonical = canonical_axis(result.angle, size);
                for (before, after) in original.stops.iter().zip(result.stops.iter()) {
                    let relative = position(
                        previous.clone(),
                        point(&canonical_axis(original.angle, size), before.position),
                    );
                    let expected = point(&next, relative);
                    assert!(
                        (position(canonical.clone(), expected) - after.position).abs() < 0.00001
                    );
                    assert_eq!(before.color, after.color);
                }
                let reopened = editing_axis(result.clone(), size);
                let round_trip = remap(result.clone(), size, reopened.clone(), reopened);
                for (a, b) in result.stops.iter().zip(round_trip.stops.iter()) {
                    assert!((a.position - b.position).abs() < 0.00001);
                }
            }
        }
    }

    #[test]
    fn translation_and_shortening_have_known_positions() {
        let mut original = fill();
        original.stops = Rc::new(VecModel::from(vec![
            GradientStop { position: 0., ..Default::default() },
            GradientStop { position: 1., ..Default::default() },
        ]))
        .into();
        let size = LogicalSize::new(200., 200.);
        let result = remap(
            original.clone(),
            size,
            editing_axis(original, size),
            LinearGradientAxis { start: Point::new(50., 60.), end: Point::new(150., 60.) },
        );
        assert!((result.angle - 90.).abs() < 0.00001);
        assert!((result.stops.row_data(0).unwrap().position - 0.25).abs() < 0.00001);
        assert!((result.stops.row_data(1).unwrap().position - 0.75).abs() < 0.00001);
    }

    #[test]
    fn collapsed_axis_does_not_change_the_fill() {
        let original = fill();
        let size = LogicalSize::new(200., 200.);
        let result = remap(
            original.clone(),
            size,
            editing_axis(original.clone(), size),
            LinearGradientAxis { start: Point::new(5., 5.), end: Point::new(5.1, 5.1) },
        );
        assert_eq!(original.angle, result.angle);
        assert_eq!(
            original.stops.iter().collect::<Vec<_>>(),
            result.stops.iter().collect::<Vec<_>>()
        );
    }
}
