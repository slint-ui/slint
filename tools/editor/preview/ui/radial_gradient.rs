// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{FillData, LinearGradientAxis};
use slint::{LogicalPosition as Point, LogicalSize};

pub fn editing_axis(fill: FillData, size: LogicalSize, direction: f32) -> LinearGradientAxis {
    let slint::Brush::RadialGradient(brush) = super::brushes::fill_brush(fill) else {
        return Default::default();
    };
    let (x, y) = brush.center_or_default(size.width, size.height);
    let radius = brush.radius_or_default(size.width, size.height);
    let direction = direction.to_radians();
    LinearGradientAxis {
        start: Point::new(x, y),
        end: Point::new(x + radius * direction.cos(), y + radius * direction.sin()),
    }
}

pub fn remap(
    mut fill: FillData,
    previous: LinearGradientAxis,
    next: LinearGradientAxis,
) -> FillData {
    let radius = (next.end.x - next.start.x).hypot(next.end.y - next.start.y);
    if !radius.is_finite() || radius < 1. || !next.start.x.is_finite() || !next.start.y.is_finite()
    {
        return fill;
    }
    if (next.start.x - previous.start.x).abs() > 0.0001
        || (next.start.y - previous.start.y).abs() > 0.0001
    {
        fill.custom_center = true;
        fill.center_x = next.start.x;
        fill.center_y = next.start.y;
    }
    let old_radius = (previous.end.x - previous.start.x).hypot(previous.end.y - previous.start.y);
    if (radius - old_radius).abs() > old_radius.max(1.) * 0.00001 {
        fill.custom_radius = true;
        fill.radius = radius;
    }
    fill
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::ui::{BrushKind, GradientStop};
    use slint::{Model, VecModel};
    use std::rc::Rc;

    fn fill() -> FillData {
        FillData {
            kind: BrushKind::Radial,
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
    fn automatic_geometry_matches_the_brush() {
        for size in [LogicalSize::new(200., 200.), LogicalSize::new(320., 120.)] {
            let original = fill();
            let axis = editing_axis(original.clone(), size, 0.);
            assert_eq!(axis.start, Point::new(size.width / 2., size.height / 2.));
            assert!((axis.end.x - axis.start.x - size.width.hypot(size.height) / 2.).abs() < 0.001);
            let rotated = editing_axis(original.clone(), size, 135.);
            let result = remap(original.clone(), axis, rotated);
            assert_eq!(
                super::super::brushes::fill_brush(result),
                super::super::brushes::fill_brush(original)
            );
        }
    }

    #[test]
    fn translation_and_resize_preserve_stops_and_automatic_fields() {
        let original = fill();
        let size = LogicalSize::new(200., 200.);
        let axis = editing_axis(original.clone(), size, 0.);
        let translated = LinearGradientAxis {
            start: Point::new(axis.start.x - 70., axis.start.y + 25.),
            end: Point::new(axis.end.x - 70., axis.end.y + 25.),
        };
        let moved = remap(original.clone(), axis.clone(), translated.clone());
        assert!(moved.custom_center);
        assert!(!moved.custom_radius);
        assert_eq!((moved.center_x, moved.center_y), (30., 125.));
        let next = LinearGradientAxis { start: translated.start, end: Point::new(30., 345.) };
        let resized = remap(moved, translated, next);
        assert_eq!(resized.radius, 220.);
        assert!(resized.custom_radius);
        assert_eq!(
            original.stops.iter().collect::<Vec<_>>(),
            resized.stops.iter().collect::<Vec<_>>()
        );
        let reopened = editing_axis(resized.clone(), size, 0.);
        assert_eq!(reopened.start, Point::new(30., 125.));
        assert_eq!(reopened.end, Point::new(250., 125.));
        let unchanged = remap(resized.clone(), reopened.clone(), reopened);
        assert_eq!(
            super::super::brushes::fill_brush(unchanged),
            super::super::brushes::fill_brush(resized)
        );
    }

    #[test]
    fn collapsed_and_nonfinite_updates_are_ignored() {
        let original = fill();
        let axis = editing_axis(original.clone(), LogicalSize::new(200., 200.), 0.);
        for end in [Point::new(100., 100.), Point::new(f32::NAN, 100.)] {
            let result = remap(
                original.clone(),
                axis.clone(),
                LinearGradientAxis { start: axis.start, end },
            );
            assert_eq!(
                super::super::brushes::fill_brush(result),
                super::super::brushes::fill_brush(original.clone())
            );
        }
    }
}
