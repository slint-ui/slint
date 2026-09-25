// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore hypot

use super::{FillData, LinearGradientAxis};
use slint::{LogicalPosition as Point, LogicalSize};

pub fn editing_axis(fill: FillData, size: LogicalSize) -> LinearGradientAxis {
    let slint::Brush::RadialGradient(brush) = super::brushes::fill_brush(fill) else {
        return Default::default();
    };
    let (x, y) = brush.center_or_default(size.width, size.height);
    let radius = brush.radius_x_or_default(size.width, size.height);
    LinearGradientAxis { start: Point::new(x, y), end: Point::new(x + radius, y) }
}

pub fn editing_y_axis(fill: FillData, size: LogicalSize) -> LinearGradientAxis {
    let slint::Brush::RadialGradient(brush) = super::brushes::fill_brush(fill) else {
        return Default::default();
    };
    let (x, y) = brush.center_or_default(size.width, size.height);
    let radius = brush.radius_y_or_default(size.width, size.height);
    LinearGradientAxis { start: Point::new(x, y), end: Point::new(x, y + radius) }
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
    fill
}

pub fn resize_x(
    mut fill: FillData,
    previous: LinearGradientAxis,
    next: LinearGradientAxis,
    match_radius: bool,
) -> FillData {
    let old_radius = (previous.end.x - previous.start.x).hypot(previous.end.y - previous.start.y);
    let radius = (next.end.x - next.start.x).hypot(next.end.y - next.start.y);
    if !old_radius.is_finite() || !radius.is_finite() || radius < 1. || next.start != previous.start
    {
        return fill;
    }
    let changed = (radius - old_radius).abs() > old_radius.max(1.) * 0.00001;
    if !changed && !match_radius {
        return fill;
    }
    let old_y = if fill.custom_radius {
        if fill.radial_ellipse { fill.radius_y } else { fill.radius }
    } else {
        old_radius
    };
    if changed || fill.custom_radius {
        fill.custom_radius = true;
        fill.radius = radius;
    }
    fill.radius_y = if match_radius { radius } else { old_y };
    fill.radial_ellipse = !match_radius && (fill.radial_ellipse || changed);
    fill
}

pub fn remap_y(
    mut fill: FillData,
    previous: LinearGradientAxis,
    next: LinearGradientAxis,
    match_radius: bool,
) -> FillData {
    let radius = next.end.y - next.start.y;
    if !radius.is_finite() || radius < 1. || next.start != previous.start {
        return fill;
    }
    let old_radius = previous.end.y - previous.start.y;
    let changed = (radius - old_radius).abs() > old_radius.max(1.) * 0.00001;
    if !changed && !match_radius {
        return fill;
    }
    let old_x = if fill.custom_radius { fill.radius } else { old_radius };
    if changed || fill.custom_radius {
        fill.custom_radius = true;
        fill.radius = if match_radius { radius } else { old_x };
    }
    fill.radius_y = radius;
    fill.radial_ellipse = !match_radius && (fill.radial_ellipse || changed);
    fill
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{BrushKind, GradientStop};
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
            let axis = editing_axis(original.clone(), size);
            assert_eq!(axis.start, Point::new(size.width / 2., size.height / 2.));
            assert!((axis.end.x - axis.start.x - size.width.hypot(size.height) / 2.).abs() < 0.001);
            assert_eq!(axis.end.y, axis.start.y);
        }
    }

    #[test]
    fn translation_and_resize_preserve_stops_and_automatic_fields() {
        let original = fill();
        let size = LogicalSize::new(200., 200.);
        let axis = editing_axis(original.clone(), size);
        let translated = LinearGradientAxis {
            start: Point::new(axis.start.x - 70., axis.start.y + 25.),
            end: Point::new(axis.end.x - 70., axis.end.y + 25.),
        };
        let moved = remap(original.clone(), axis.clone(), translated.clone());
        assert!(moved.custom_center);
        assert!(!moved.custom_radius);
        assert_eq!((moved.center_x, moved.center_y), (30., 125.));
        let next = LinearGradientAxis { start: translated.start, end: Point::new(250., 125.) };
        let resized = resize_x(moved, translated, next, false);
        assert_eq!(resized.radius, 220.);
        assert!(resized.custom_radius);
        assert!(resized.radial_ellipse);
        assert_eq!(
            original.stops.iter().collect::<Vec<_>>(),
            resized.stops.iter().collect::<Vec<_>>()
        );
        let reopened = editing_axis(resized.clone(), size);
        assert_eq!(reopened.start, Point::new(30., 125.));
        assert_eq!(reopened.end, Point::new(250., 125.));
        let unchanged = remap(resized.clone(), reopened.clone(), reopened);
        assert_eq!(
            super::super::brushes::fill_brush(unchanged),
            super::super::brushes::fill_brush(resized)
        );
    }

    #[test]
    fn collapsed_and_non_finite_updates_are_ignored() {
        let original = fill();
        let axis = editing_axis(original.clone(), LogicalSize::new(200., 200.));
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
            let result = resize_x(
                original.clone(),
                axis.clone(),
                LinearGradientAxis { start: axis.start, end },
                false,
            );
            assert_eq!(
                super::super::brushes::fill_brush(result),
                super::super::brushes::fill_brush(original.clone())
            );
        }
    }

    #[test]
    fn ellipse_radii_can_be_resized_independently() {
        let original = FillData { radial_ellipse: true, ..fill() };
        let size = LogicalSize::new(200., 120.);
        let x_axis = editing_axis(original.clone(), size);
        let y_axis = editing_y_axis(original.clone(), size);
        let default_radius = size.width.hypot(size.height) / 2.;
        assert!((x_axis.end.x - x_axis.start.x - default_radius).abs() < 0.001);
        assert!((y_axis.end.y - y_axis.start.y - default_radius).abs() < 0.001);

        let taller = remap_y(
            original.clone(),
            y_axis.clone(),
            LinearGradientAxis {
                start: y_axis.start,
                end: Point::new(y_axis.end.x, y_axis.end.y + 30.),
            },
            false,
        );
        assert!(taller.custom_radius);
        assert!((taller.radius - default_radius).abs() < 0.001);
        assert!((taller.radius_y - default_radius - 30.).abs() < 0.001);
        assert!(!taller.custom_center);

        let x_axis = editing_axis(taller.clone(), size);
        let wider = resize_x(
            taller.clone(),
            x_axis.clone(),
            LinearGradientAxis {
                start: x_axis.start,
                end: Point::new(x_axis.end.x + 20., x_axis.end.y),
            },
            false,
        );
        assert!((wider.radius - default_radius - 20.).abs() < 0.001);
        assert_eq!(wider.radius_y, taller.radius_y);
        assert_eq!(
            original.stops.iter().collect::<Vec<_>>(),
            wider.stops.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn radius_handles_create_ellipses_and_shift_restores_circles() {
        let size = LogicalSize::new(200., 200.);
        let circle = fill();
        let x = editing_axis(circle.clone(), size);
        let y = editing_y_axis(circle.clone(), size);
        let wider = resize_x(
            circle.clone(),
            x.clone(),
            LinearGradientAxis { start: x.start, end: Point::new(x.end.x + 30., x.end.y) },
            false,
        );
        assert!(wider.radial_ellipse);
        assert!((wider.radius_y - (x.end.x - x.start.x)).abs() < 0.001);
        let taller = remap_y(
            circle,
            y.clone(),
            LinearGradientAxis { start: y.start, end: Point::new(y.end.x, y.end.y + 20.) },
            false,
        );
        assert!(taller.radial_ellipse);
        assert!((taller.radius - (y.end.y - y.start.y)).abs() < 0.001);

        let matched_x = resize_x(
            wider.clone(),
            editing_axis(wider.clone(), size),
            LinearGradientAxis { start: x.start, end: Point::new(x.end.x + 45., x.end.y) },
            true,
        );
        assert!(!matched_x.radial_ellipse);
        assert_eq!(matched_x.radius, matched_x.radius_y);
        let matched_y = remap_y(
            taller.clone(),
            editing_y_axis(taller.clone(), size),
            LinearGradientAxis { start: y.start, end: Point::new(y.end.x, y.end.y + 35.) },
            true,
        );
        assert!(!matched_y.radial_ellipse);
        assert_eq!(matched_y.radius, matched_y.radius_y);
    }
}
