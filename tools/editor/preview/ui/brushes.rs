// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Handle colors and brushes in the UI

use slint::{Model, VecModel};

use crate::preview::ui;

use itertools::Itertools as _;
use std::rc::Rc;

pub fn setup(api: &ui::Api<'_>) {
    api.on_fill_brush(fill_brush);
    api.on_fill_expression(fill_expression);
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
    api.on_move_gradient_stop(move_gradient_stop);
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

pub fn fill_from_expression(
    expression: &i_slint_compiler::expression_tree::Expression,
    brush: slint::Brush,
    window: Option<&Rc<dyn slint::platform::WindowAdapter>>,
) -> Option<ui::FillData> {
    use crate::preview::eval::fully_eval_expression_tree_expression as eval;
    use i_slint_compiler::expression_tree::Expression;
    if let Expression::Cast { from, .. } = expression {
        return fill_from_expression(from, brush, window);
    }
    let mut fill = fill_from_brush(brush);
    let number = |e: &Expression| -> Option<f32> {
        let value: f32 = eval(e, window)?.try_into().ok()?;
        value.is_finite().then_some(value)
    };
    let (stops, angle, center, radius) = match expression {
        Expression::LinearGradient { angle, stops } => (stops, Some(&**angle), None, None),
        Expression::RadialGradient { stops, center, radius } => {
            (stops, None, center.as_ref(), radius.as_deref())
        }
        Expression::ConicGradient { from_angle, stops, center } => {
            (stops, Some(&**from_angle), center.as_ref(), None)
        }
        _ => return Some(fill),
    };
    if let Some(angle) = angle {
        fill.angle = number(angle)?;
    }
    if let Some((x, y)) = center {
        fill.custom_center = true;
        fill.center_x = number(x)?;
        fill.center_y = number(y)?;
    }
    if let Some(radius) = radius {
        fill.custom_radius = true;
        fill.radius = number(radius)?;
    }
    let stops = stops
        .iter()
        .map(|(color, position)| {
            Some(ui::GradientStop {
                color: eval(color, window)?.try_into().ok()?,
                position: number(position)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    fill.stops = Rc::new(VecModel::from(stops)).into();
    Some(fill)
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
                .map(|s| format!(", {} {:.2}%", color_to_string(s.color), s.position * 100.))
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
                .map(|s| format!(", {} {}%", color_to_string(s.color), s.position * 100.))
                .join("")
        )
    } else {
        slint::format!(
            "@conic-gradient(from {}deg{center}, {})",
            fill.angle,
            stops
                .iter()
                .map(|s| format!("{} {}deg", color_to_string(s.color), s.position * 360.))
                .join(", ")
        )
    }
}

fn find_index_for_position(model: &slint::ModelRc<ui::GradientStop>, position: f32) -> usize {
    let position = position.clamp(0.0, 1.0);

    model
        .iter()
        .position(|gs| gs.position.total_cmp(&position) != std::cmp::Ordering::Less)
        .unwrap_or(model.row_count())
}

fn add_gradient_stop(model: slint::ModelRc<ui::GradientStop>, value: ui::GradientStop) -> i32 {
    let insert_pos = find_index_for_position(&model, value.position);
    let m = model.as_any().downcast_ref::<VecModel<_>>().unwrap();
    m.insert(insert_pos, value);
    (insert_pos) as i32
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

fn move_gradient_stop(model: slint::ModelRc<ui::GradientStop>, row: i32, new_position: f32) -> i32 {
    let mut row_usize = row as usize;
    if row < 0 || row_usize >= model.row_count() {
        return row;
    }

    let m = model.as_any().downcast_ref::<VecModel<ui::GradientStop>>().unwrap();

    let mut gs = model.row_data(row_usize).unwrap();
    gs.position = new_position;
    model.set_row_data(row_usize, gs);

    fn swap_direction(
        model: &VecModel<ui::GradientStop>,
        row: usize,
        value: f32,
    ) -> Option<(usize, usize)> {
        let previous = model.row_data(row.saturating_sub(1));
        let next = model.row_data(row + 1);
        let previous_order = previous.map(|gs| value.total_cmp(&gs.position));
        let next_order = next.map(|gs| value.total_cmp(&gs.position));

        match (previous_order, next_order) {
            (Some(std::cmp::Ordering::Less), _) => Some((row, row - 1)),
            (_, Some(std::cmp::Ordering::Greater)) => Some((row, row + 1)),
            _ => None,
        }
    }

    while let Some((old_row, new_row)) = swap_direction(m, row_usize, new_position) {
        m.swap(old_row, new_row);
        row_usize = new_row;
    }

    row_usize as i32
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
    let position = position.clamp(0.0, 1.0);

    if model.row_count() == 0 {
        return fallback_gradient_stop(position);
    }

    let mut prev = model.row_data(0).expect("Not empty");
    prev.position = 0.0;
    let mut next = model.row_data(model.row_count() - 1).expect("Not empty");
    next.position = 1.0;

    for current in model.iter() {
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
    use crate::preview::ui;

    use slint::{Model, ModelRc, VecModel};

    use std::rc::Rc;

    fn make_empty_model() -> ModelRc<ui::GradientStop> {
        Rc::new(VecModel::default()).into()
    }

    #[test]
    fn test_add_and_remove_gradient_stops() {
        let model = make_empty_model();

        super::remove_gradient_stop(model.clone(), 0);

        let mut it = model.iter();
        assert_eq!(it.next(), None);

        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff010101) },
        );

        super::remove_gradient_stop(model.clone(), 0);
        let mut it = model.iter();
        assert_eq!(it.next(), None);

        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff010101) },
        );

        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff020202) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff030303) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.5, color: slint::Color::from_argb_encoded(0xff050505) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff040404) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            },
        );

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 2);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), -1);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 42);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 0);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(it.next(), None);

        super::remove_gradient_stop(model.clone(), 3);

        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);
    }

    fn make_model() -> ModelRc<ui::GradientStop> {
        let model = make_empty_model();
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff040404) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.0, color: slint::Color::from_argb_encoded(0xff030303) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 0.5, color: slint::Color::from_argb_encoded(0xff050505) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff020202) },
        );
        super::add_gradient_stop(
            model.clone(),
            ui::GradientStop { position: 1.0, color: slint::Color::from_argb_encoded(0xff010101) },
        );

        model
    }

    #[test]
    fn test_move_gradient_stop() {
        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 0.4), 3);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.4,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 0.1), 2);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 0, 0.05), 1);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.05,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.5,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 0.0), 2);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);

        let model = make_model();

        assert_eq!(super::move_gradient_stop(model.clone(), 3, 1.0), 3);
        let mut it = model.iter();

        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff030303)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.0,
                color: slint::Color::from_argb_encoded(0xff040404)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 0.1445,
                color: slint::Color::from_argb_encoded(0xff060606),
            }),
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff050505)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff010101)
            })
        );
        assert_eq!(
            it.next(),
            Some(ui::GradientStop {
                position: 1.0,
                color: slint::Color::from_argb_encoded(0xff020202)
            })
        );
        assert_eq!(it.next(), None);
    }
}
