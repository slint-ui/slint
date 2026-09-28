// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Handle colors and brushes in the UI

use slint::{Model, VecModel};

use crate::ui;

use itertools::Itertools as _;
use std::rc::Rc;

pub fn setup(api: &ui::Api<'_>) {
    api.on_fill_brush(fill_brush);
    api.on_fill_expression(fill_expression);
    api.on_linear_gradient_axis(super::linear_gradient::editing_axis);
    api.on_radial_gradient_axis(super::radial_gradient::editing_axis);
    api.on_remap_radial_gradient(super::radial_gradient::remap);
    api.on_conic_gradient_axis(super::conic_gradient::editing_axis);
    api.on_remap_conic_gradient(super::conic_gradient::remap);
    api.on_conic_gradient_point(super::conic_gradient::point);
    api.on_conic_gradient_position(super::conic_gradient::position);
    api.on_gradient_angular_delta(super::conic_gradient::angular_delta);
    api.on_nearest_conic_gradient_stop(super::conic_gradient::nearest_stop);
    api.on_gradient_axis_point(|axis, position| super::linear_gradient::point(&axis, position));
    api.on_gradient_axis_position(super::linear_gradient::position);
    api.on_linear_gradient_point(|angle, size, position| {
        super::linear_gradient::point(
            &super::linear_gradient::canonical_axis(angle, size),
            position,
        )
    });
    api.on_linear_gradient_position(|angle, size, point| {
        super::linear_gradient::position(super::linear_gradient::canonical_axis(angle, size), point)
    });
    api.on_remap_linear_gradient(super::linear_gradient::remap);
    api.on_nearest_gradient_stop(|stops, position| {
        stops
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.position - position).abs().total_cmp(&(b.position - position).abs())
            })
            .map_or(-1, |(index, _)| index as i32)
    });
    api.on_gradient_stop_gap(|stops| {
        let mut positions = vec![0., 1.];
        positions.extend(stops.iter().map(|s| s.position.clamp(0., 1.)));
        positions.sort_by(f32::total_cmp);
        let mut best = (0., 0.5);
        for pair in positions.windows(2) {
            let gap = pair[1] - pair[0];
            if gap > best.0 {
                best = (gap, (pair[0] + pair[1]) / 2.);
            }
        }
        best.1
    });
    api.on_add_gradient_stop(add_gradient_stop);
    api.on_remove_gradient_stop(remove_gradient_stop);
    api.on_gradient_stop_order(gradient_stop_order);
    // Skia interpolates linear/radial gradients in premultiplied alpha, but conic gradients in straight alpha.
    api.on_sample_fill_stop(|fill, position| {
        gradient_stop_at_position(fill.stops, position, fill.kind != ui::BrushKind::Conic)
    });
    api.on_clone_gradient_stops(clone_gradient_stops);

    api.on_create_brush(create_brush);

    api.on_string_to_color(|s| string_to_color(s.as_ref()).unwrap_or_default());
    api.on_string_is_color(|s| string_to_color(s.as_ref()).is_some());
    api.on_color_to_data(|c| ui::ColorData {
        a: c.alpha() as i32,
        r: c.red() as i32,
        g: c.green() as i32,
        b: c.blue() as i32,
        text: color_to_string(c),
        short_text: color_to_short_string(c).into(),
    });
    api.on_rgba_to_color(|r, g, b, a| {
        if (0..256).contains(&r)
            && (0..256).contains(&g)
            && (0..256).contains(&b)
            && (0..256).contains(&a)
        {
            slint::Color::from_argb_u8(a as u8, r as u8, g as u8, b as u8)
        } else {
            slint::Color::default()
        }
    });
    api.on_color_field_color_edit(color_field_color_edit);
    api.on_color_field_opacity_edit(color_field_opacity_edit);
    api.on_color_field_fill_with_opacity(color_field_fill_with_opacity);
}

pub fn color_to_string(color: slint::Color) -> slint::SharedString {
    let a = color.alpha();
    let r = color.red();
    let g = color.green();
    let b = color.blue();

    if a == 255 {
        slint::format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        slint::format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

fn color_to_short_string(color: slint::Color) -> String {
    let r = color.red();
    let g = color.green();
    let b = color.blue();

    format!("{r:02x}{g:02x}{b:02x}")
}

pub fn color_field_data(property: ui::PropertyValue) -> ui::ColorFieldData {
    let explicit_resolved_value = !property.code.is_empty()
        && property.value_resolved
        && property.kind != ui::PropertyValueKind::Code
        && property.value_string.is_empty();
    let allow_gradient = property.kind == ui::PropertyValueKind::Brush
        || property.value_kind == ui::PropertyValueKind::Brush;
    let mode = if explicit_resolved_value && property.fill.kind == ui::BrushKind::Solid {
        ui::ColorFieldMode::Solid
    } else if explicit_resolved_value && allow_gradient {
        ui::ColorFieldMode::Gradient
    } else {
        ui::ColorFieldMode::Source
    };
    let label = match property.fill.kind {
        ui::BrushKind::Linear => "Linear Gradient",
        ui::BrushKind::Radial => "Radial Gradient",
        ui::BrushKind::Conic => "Conic Gradient",
        ui::BrushKind::Solid => "",
    };
    let color = property.fill.color;

    ui::ColorFieldData {
        mode,
        fill: property.fill,
        color_text: color_to_short_string(color).to_uppercase().into(),
        opacity: ((color.alpha() as f32 * 100. / 255.).round()) as i32,
        label: label.into(),
        unsupported: !property.value_resolved,
        allow_gradient,
    }
}

fn color_field_expression(text: &str) -> Option<(String, slint::Color)> {
    if let Some(color) = string_to_color(text) {
        return Some((text.to_owned(), color));
    }
    let expression = format!("#{text}");
    string_to_color(&expression).map(|color| (expression, color))
}

fn color_field_input_has_alpha(expression: &str) -> bool {
    let expression = expression.to_ascii_lowercase();
    expression.strip_prefix('#').is_some_and(|digits| matches!(digits.len(), 4 | 8))
        || expression.starts_with("rgba(")
        || expression.starts_with("hsla(")
}

fn color_field_edit(color: slint::Color) -> ui::ColorFieldEdit {
    ui::ColorFieldEdit {
        valid: true,
        expression: color_to_string(color),
        display: color_to_short_string(color).to_uppercase().into(),
    }
}

fn color_field_color_edit(fill: ui::FillData, text: slint::SharedString) -> ui::ColorFieldEdit {
    let Some((expression, parsed)) = color_field_expression(text.as_str()) else {
        return Default::default();
    };
    let alpha =
        if color_field_input_has_alpha(&expression) { parsed.alpha() } else { fill.color.alpha() };
    color_field_edit(slint::Color::from_argb_u8(alpha, parsed.red(), parsed.green(), parsed.blue()))
}

fn color_field_fill_with_opacity(mut fill: ui::FillData, opacity: f32) -> ui::FillData {
    if !opacity.is_finite() {
        return fill;
    }
    let color = fill.color;
    let alpha = (opacity.clamp(0., 100.) * 255. / 100.).round() as u8;
    fill.color = slint::Color::from_argb_u8(alpha, color.red(), color.green(), color.blue());
    fill
}

fn color_field_opacity_edit(fill: ui::FillData, text: slint::SharedString) -> ui::ColorFieldEdit {
    let Ok(opacity) = text.parse::<f32>() else {
        return Default::default();
    };
    if !opacity.is_finite() {
        return Default::default();
    }
    let opacity = opacity.clamp(0., 100.);
    let fill = color_field_fill_with_opacity(fill, opacity);
    let color = fill.color;
    ui::ColorFieldEdit {
        valid: true,
        expression: color_to_string(color),
        display: format!("{opacity:.0}").into(),
    }
}

pub fn string_to_color(text: &str) -> Option<slint::Color> {
    i_slint_common::color_parsing::parse_color_literal(text).map(slint::Color::from_argb_encoded)
}

fn sorted_gradient_stops(
    stops: slint::ModelRc<ui::GradientStop>,
) -> Vec<i_slint_core::graphics::GradientStop> {
    let mut result = stops
        .iter()
        .map(|gs| i_slint_core::graphics::GradientStop { position: gs.position, color: gs.color })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.position.total_cmp(&right.position));

    result
}

pub fn create_brush(
    kind: ui::BrushKind,
    angle: f32,
    color: slint::Color,
    stops: slint::ModelRc<ui::GradientStop>,
) -> slint::Brush {
    let mut stops = sorted_gradient_stops(stops);

    match kind {
        ui::BrushKind::Solid => slint::Brush::SolidColor(color),
        ui::BrushKind::Linear => slint::Brush::LinearGradient(
            i_slint_core::graphics::LinearGradientBrush::new(angle, stops.drain(..)),
        ),
        ui::BrushKind::Radial => slint::Brush::RadialGradient(
            i_slint_core::graphics::RadialGradientBrush::new_circle(stops.drain(..)),
        ),
        ui::BrushKind::Conic => slint::Brush::ConicGradient(
            i_slint_core::graphics::ConicGradientBrush::new(angle, stops.drain(..)),
        ),
    }
}

pub fn fill_from_brush(brush: slint::Brush) -> ui::FillData {
    let mut fill = ui::FillData::default();
    let stops: Vec<_> = match brush {
        slint::Brush::SolidColor(color) => {
            fill.color = color;
            Vec::new()
        }
        slint::Brush::LinearGradient(g) => {
            fill.kind = ui::BrushKind::Linear;
            fill.angle = g.angle();
            g.stops().copied().collect()
        }
        slint::Brush::RadialGradient(g) => {
            fill.kind = ui::BrushKind::Radial;
            let center = g.center_or_default(0., 0.);
            fill.custom_center = center == g.center_or_default(2., 2.);
            (fill.center_x, fill.center_y) = center;
            fill.radius = g.radius_or_default(0., 0.);
            fill.custom_radius = fill.radius == g.radius_or_default(2., 2.);
            g.stops().copied().collect()
        }
        slint::Brush::ConicGradient(g) => {
            fill.kind = ui::BrushKind::Conic;
            let center = g.center_or_default(0., 0.);
            fill.custom_center = center == g.center_or_default(2., 2.);
            (fill.center_x, fill.center_y) = center;
            g.stops().copied().collect()
        }
        _ => Vec::new(),
    };
    fill.stops = Rc::new(VecModel::from(
        stops
            .into_iter()
            .map(|s| ui::GradientStop { color: s.color, position: s.position })
            .collect::<Vec<_>>(),
    ))
    .into();
    fill
}

pub fn fill_brush(fill: ui::FillData) -> slint::Brush {
    let brush = create_brush(fill.kind, fill.angle, fill.color, fill.stops);
    match brush {
        slint::Brush::RadialGradient(mut g) => {
            if fill.custom_center {
                g = g.with_center(fill.center_x, fill.center_y);
            }
            if fill.custom_radius {
                g = g.with_radius(fill.radius);
            }
            slint::Brush::RadialGradient(g)
        }
        slint::Brush::ConicGradient(mut g) => {
            if fill.custom_center {
                g = g.with_center(fill.center_x, fill.center_y);
            }
            slint::Brush::ConicGradient(g)
        }
        brush => brush,
    }
}

fn stop_position(position: f32, units: f64, suffix: &str) -> String {
    let scaled = f64::from(position) * units;
    for precision in 0..17 {
        let candidate = format!("{scaled:.precision$e}").parse::<f64>().unwrap();
        if (candidate / units) as f32 == position {
            return if candidate < 0. {
                format!("0{suffix} - {}{suffix}", -candidate)
            } else {
                format!("{candidate}{suffix}")
            };
        }
    }
    format!("{scaled}{suffix}")
}

pub fn fill_expression(fill: ui::FillData) -> slint::SharedString {
    if fill.kind == ui::BrushKind::Solid {
        return color_to_string(fill.color);
    }
    let stops = sorted_gradient_stops(fill.stops);
    if fill.kind == ui::BrushKind::Linear {
        return slint::format!(
            "@linear-gradient({}deg{})",
            fill.angle,
            stops
                .iter()
                .map(|s| format!(
                    ", {} {}",
                    color_to_string(s.color),
                    stop_position(s.position, 100., "%")
                ))
                .join("")
        );
    }
    let center = if fill.custom_center {
        format!(" at {}px {}px", fill.center_x, fill.center_y)
    } else {
        String::new()
    };
    if fill.kind == ui::BrushKind::Radial {
        let radius = if fill.custom_radius { format!(" {}px", fill.radius) } else { String::new() };
        slint::format!(
            "@radial-gradient(circle{radius}{center}{})",
            stops
                .iter()
                .map(|s| format!(
                    ", {} {}",
                    color_to_string(s.color),
                    stop_position(s.position, 100., "%")
                ))
                .join("")
        )
    } else {
        slint::format!(
            "@conic-gradient(from {}deg{center}, {})",
            fill.angle,
            stops
                .iter()
                .map(|s| format!(
                    "{} {}",
                    color_to_string(s.color),
                    stop_position(s.position, 360., "deg")
                ))
                .join(", ")
        )
    }
}

fn add_gradient_stop(model: slint::ModelRc<ui::GradientStop>, value: ui::GradientStop) -> i32 {
    let insert_pos = model
        .iter()
        .enumerate()
        .filter(|(_, stop)| stop.position == value.position)
        .map(|(index, _)| index + 1)
        .last()
        .or_else(|| model.iter().position(|stop| stop.position > value.position))
        .unwrap_or(model.row_count());
    let m = model.as_any().downcast_ref::<VecModel<_>>().unwrap();
    m.insert(insert_pos, value);
    insert_pos as i32
}

fn remove_gradient_stop(model: slint::ModelRc<ui::GradientStop>, row: i32) {
    if row < 0 {
        return;
    }
    let row = row as usize;
    if row < model.row_count() {
        model.as_any().downcast_ref::<VecModel<ui::GradientStop>>().unwrap().remove(row);
    }
}

fn gradient_stop_order(
    model: slint::ModelRc<ui::GradientStop>,
    selected: i32,
) -> ui::GradientStopOrder {
    let mut stops = model.iter().enumerate().collect::<Vec<_>>();
    stops.sort_by(|a, b| a.1.position.total_cmp(&b.1.position));
    ui::GradientStopOrder {
        selected: stops
            .iter()
            .position(|(index, _)| *index as i32 == selected)
            .map_or(-1, |row| row as i32),
        indices: Rc::new(VecModel::from(
            stops.into_iter().map(|(index, _)| index as i32).collect::<Vec<_>>(),
        ))
        .into(),
    }
}

fn interpolate_color(
    a: slint::Color,
    b: slint::Color,
    t: f32,
    premultiplied: bool,
) -> slint::Color {
    let alpha = a.alpha() as f32 * (1. - t) + b.alpha() as f32 * t;
    let channel = |a_channel: u8, b_channel: u8| {
        let value = if premultiplied && alpha > 0. {
            (a_channel as f32 * a.alpha() as f32 * (1. - t)
                + b_channel as f32 * b.alpha() as f32 * t)
                / alpha
        } else {
            a_channel as f32 * (1. - t) + b_channel as f32 * t
        };
        value.round() as u8
    };
    slint::Color::from_argb_u8(
        alpha.round() as u8,
        channel(a.red(), b.red()),
        channel(a.green(), b.green()),
        channel(a.blue(), b.blue()),
    )
}

fn fallback_gradient_stop(position: f32) -> ui::GradientStop {
    ui::GradientStop { position, color: slint::Color::from_argb_u8(0xff, 0x80, 0x80, 0x80) }
}

fn gradient_stop_at_position(
    model: slint::ModelRc<ui::GradientStop>,
    position: f32,
    premultiplied: bool,
) -> ui::GradientStop {
    if model.row_count() == 0 {
        return fallback_gradient_stop(position);
    }

    let stops = sorted_gradient_stops(model);
    let mut prev = stops[0];
    let mut next = stops[stops.len() - 1];

    for current in stops {
        if current.position > position {
            next = current;
            break;
        }

        if current.position <= position {
            prev = current;
        }
    }

    if next.position <= prev.position {
        return ui::GradientStop { position, color: prev.color };
    }
    let factor = (position - prev.position) / (next.position - prev.position);

    ui::GradientStop {
        position,
        color: interpolate_color(prev.color, next.color, factor, premultiplied),
    }
}

fn clone_gradient_stops(
    model: slint::ModelRc<ui::GradientStop>,
) -> slint::ModelRc<ui::GradientStop> {
    let cloned_data = model.iter().collect::<Vec<_>>();
    Rc::new(VecModel::from(cloned_data)).into()
}

#[cfg(test)]
mod tests {
    use crate::ui;

    use slint::{Model, ModelRc, VecModel};

    use std::rc::Rc;

    fn color_property(color: slint::Color) -> ui::PropertyValue {
        ui::PropertyValue {
            code: "#1a2dac".into(),
            value_resolved: true,
            kind: ui::PropertyValueKind::Color,
            value_kind: ui::PropertyValueKind::Color,
            fill: ui::FillData { color, ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn color_field_data_describes_solid_gradients_and_source_values() {
        let solid = super::color_field_data(color_property(slint::Color::from_argb_u8(
            77, 0x1a, 0x2d, 0xac,
        )));
        assert_eq!(solid.mode, ui::ColorFieldMode::Solid);
        assert_eq!(solid.color_text, "1A2DAC");
        assert_eq!(solid.opacity, 30);
        assert_eq!(solid.label, "");
        assert!(!solid.unsupported);
        assert!(!solid.allow_gradient);

        for (kind, label) in [
            (ui::BrushKind::Linear, "Linear Gradient"),
            (ui::BrushKind::Radial, "Radial Gradient"),
            (ui::BrushKind::Conic, "Conic Gradient"),
        ] {
            let data = super::color_field_data(ui::PropertyValue {
                code: "@gradient".into(),
                value_resolved: true,
                kind: ui::PropertyValueKind::Brush,
                value_kind: ui::PropertyValueKind::Brush,
                fill: ui::FillData { kind, ..Default::default() },
                ..Default::default()
            });
            assert_eq!(data.mode, ui::ColorFieldMode::Gradient);
            assert_eq!(data.label, label);
            assert!(data.allow_gradient);
        }

        let source = super::color_field_data(ui::PropertyValue {
            code: "Colors.primary".into(),
            value_resolved: true,
            kind: ui::PropertyValueKind::Code,
            value_kind: ui::PropertyValueKind::Color,
            ..Default::default()
        });
        assert_eq!(source.mode, ui::ColorFieldMode::Source);
    }

    #[test]
    fn color_field_edits_preserve_or_replace_alpha_in_rust() {
        let fill = ui::FillData {
            color: slint::Color::from_argb_u8(77, 0xff, 0xff, 0xff),
            ..Default::default()
        };
        for input in ["1a2dac", "#1a2dac"] {
            let edit = super::color_field_color_edit(fill.clone(), input.into());
            assert!(edit.valid);
            assert_eq!(edit.expression, "#1a2dac4d");
            assert_eq!(edit.display, "1A2DAC");
        }

        let edit = super::color_field_color_edit(fill, "#1234".into());
        assert!(edit.valid);
        assert_eq!(edit.expression, "#11223344");

        let invalid = super::color_field_color_edit(Default::default(), "not a color".into());
        assert!(!invalid.valid);
    }

    #[test]
    fn color_field_opacity_edits_clamp_and_format_in_rust() {
        let fill = ui::FillData {
            color: slint::Color::from_rgb_u8(0x1a, 0x2d, 0xac),
            ..Default::default()
        };
        for (input, expression, display, alpha) in [
            ("-1", "#1a2dac00", "0", 0),
            ("30", "#1a2dac4d", "30", 77),
            ("101", "#1a2dac", "100", 255),
        ] {
            let edit = super::color_field_opacity_edit(fill.clone(), input.into());
            assert!(edit.valid);
            assert_eq!(edit.expression, expression);
            assert_eq!(edit.display, display);
            assert_eq!(super::string_to_color(expression).unwrap().alpha(), alpha);
        }

        let invalid = super::color_field_opacity_edit(fill, "opaque".into());
        assert!(!invalid.valid);
    }

    fn make_empty_model() -> ModelRc<ui::GradientStop> {
        Rc::new(VecModel::default()).into()
    }

    #[test]
    fn sampling_crossed_stops_matches_sorted_stops() {
        let stops = vec![
            ui::GradientStop { position: 1.2, color: slint::Color::from_argb_u8(128, 255, 0, 0) },
            ui::GradientStop { position: -0.2, color: slint::Color::from_rgb_u8(0, 0, 255) },
            ui::GradientStop { position: 0.5, color: slint::Color::from_rgb_u8(0, 255, 0) },
        ];
        let unsorted: ModelRc<_> = Rc::new(VecModel::from(stops.clone())).into();
        let mut sorted = stops;
        sorted.sort_by(|a, b| a.position.total_cmp(&b.position));
        let sorted: ModelRc<_> = Rc::new(VecModel::from(sorted)).into();
        for position in [-0.2, 0.0, 0.5, 0.8, 1.2] {
            for premultiplied in [true, false] {
                assert_eq!(
                    super::gradient_stop_at_position(unsorted.clone(), position, premultiplied),
                    super::gradient_stop_at_position(sorted.clone(), position, premultiplied),
                );
            }
        }
    }

    fn make_model() -> ModelRc<ui::GradientStop> {
        Rc::new(VecModel::from(
            [
                (0., 0xff040404),
                (0., 0xff030303),
                (0.1445, 0xff060606),
                (0.5, 0xff050505),
                (1., 0xff020202),
                (1., 0xff010101),
            ]
            .into_iter()
            .map(|(position, color)| ui::GradientStop {
                position,
                color: slint::Color::from_argb_encoded(color),
            })
            .collect::<Vec<_>>(),
        ))
        .into()
    }

    #[test]
    fn add_and_remove_stops_preserves_existing_slots() {
        let model = make_model();
        let original = model.iter().collect::<Vec<_>>();
        for position in [-1., 0., 0.25, 1., 2.] {
            let stop = ui::GradientStop { position, ..Default::default() };
            let index = super::add_gradient_stop(model.clone(), stop.clone());
            assert_eq!(model.row_data(index as usize), Some(stop));
            super::remove_gradient_stop(model.clone(), index);
            assert_eq!(model.iter().collect::<Vec<_>>(), original);
        }
        super::remove_gradient_stop(model.clone(), -1);
        super::remove_gradient_stop(model.clone(), model.row_count() as i32);
        assert_eq!(model.iter().collect::<Vec<_>>(), original);
        let empty = make_empty_model();
        super::remove_gradient_stop(empty.clone(), 0);
        assert_eq!(empty.row_count(), 0);
    }

    #[test]
    fn insertion_preserves_coincident_order_after_crossing() {
        for position in [-0.2, 0.5, 1.2] {
            let red = slint::Color::from_rgb_u8(255, 0, 0);
            let blue = slint::Color::from_rgb_u8(0, 0, 255);
            let model: ModelRc<_> = Rc::new(VecModel::from(vec![
                ui::GradientStop { position: 2., color: red },
                ui::GradientStop { position, color: red },
                ui::GradientStop { position: -1., color: blue },
                ui::GradientStop { position, color: blue },
            ]))
            .into();
            let before = model.iter().collect::<Vec<_>>();
            let samples = [-0.9, position - 0.01, position, position + 0.01, 1.9];
            let expected =
                samples.map(|p| super::gradient_stop_at_position(model.clone(), p, true));
            let index = super::add_gradient_stop(
                model.clone(),
                super::gradient_stop_at_position(model.clone(), position, true),
            );
            assert_eq!(model.row_data(index as usize).unwrap().color, blue);
            assert_eq!(
                model
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index as usize)
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>(),
                before
            );
            assert_eq!(
                samples.map(|p| super::gradient_stop_at_position(model.clone(), p, true)),
                expected
            );
            let order = super::gradient_stop_order(model.clone(), index);
            let ordered: ModelRc<_> = Rc::new(VecModel::from(
                order
                    .indices
                    .iter()
                    .map(|slot| model.row_data(slot as usize).unwrap())
                    .collect::<Vec<_>>(),
            ))
            .into();
            assert_eq!(
                ordered
                    .iter()
                    .filter(|stop| stop.position == position)
                    .map(|stop| stop.color)
                    .collect::<Vec<_>>(),
                [red, blue, blue]
            );
            assert_eq!(super::gradient_stop_at_position(model.clone(), position, true).color, blue);
            for kind in [ui::BrushKind::Linear, ui::BrushKind::Radial, ui::BrushKind::Conic] {
                let fill = ui::FillData { kind, stops: model.clone(), ..Default::default() };
                let sorted = ui::FillData { stops: ordered.clone(), ..fill.clone() };
                assert_eq!(
                    super::fill_expression(fill.clone()),
                    super::fill_expression(sorted.clone())
                );
                assert_eq!(super::fill_brush(fill), super::fill_brush(sorted));
            }
        }
    }

    #[test]
    fn ordered_view_preserves_slots_and_duplicate_order() {
        let model = make_model();
        for (slot, position) in [(3, 0.4), (3, -0.1), (0, 1.2), (3, 0.), (3, 1.)] {
            let before = model.iter().map(|stop| stop.color).collect::<Vec<_>>();
            let mut stop = model.row_data(slot).unwrap();
            stop.position = position;
            model.set_row_data(slot, stop);
            let order = super::gradient_stop_order(model.clone(), slot as i32);
            let indices = order.indices.iter().collect::<Vec<_>>();
            assert_eq!(indices[order.selected as usize], slot as i32);
            assert_eq!(before, model.iter().map(|stop| stop.color).collect::<Vec<_>>());
            for pair in indices.windows(2) {
                let a = model.row_data(pair[0] as usize).unwrap().position;
                let b = model.row_data(pair[1] as usize).unwrap().position;
                assert!(a <= b);
                if a == b {
                    assert!(pair[0] < pair[1]);
                }
            }
            let ordered: ModelRc<_> = Rc::new(VecModel::from(
                indices.iter().map(|i| model.row_data(*i as usize).unwrap()).collect::<Vec<_>>(),
            ))
            .into();
            for kind in [ui::BrushKind::Linear, ui::BrushKind::Radial, ui::BrushKind::Conic] {
                let fill = ui::FillData { kind, stops: model.clone(), ..Default::default() };
                let sorted = ui::FillData { stops: ordered.clone(), ..fill.clone() };
                assert_eq!(
                    super::fill_expression(fill.clone()),
                    super::fill_expression(sorted.clone())
                );
                assert_eq!(super::fill_brush(fill), super::fill_brush(sorted));
            }
        }
        let empty = super::gradient_stop_order(make_empty_model(), 0);
        assert_eq!(empty.selected, -1);
        assert_eq!(empty.indices.row_count(), 0);
    }
}
