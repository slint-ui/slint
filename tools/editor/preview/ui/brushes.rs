// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::preview::ui;
use slint::VecModel;
pub use slint_editor::component_support::brushes::*;
use std::rc::Rc;

pub fn fill_from_expression(
    expression: &i_slint_compiler::expression_tree::Expression,
    mut fill: ui::FillData,
    window: Option<&Rc<dyn slint::platform::WindowAdapter>>,
) -> Option<ui::FillData> {
    use crate::preview::eval::fully_eval_expression_tree_expression as eval;
    use i_slint_compiler::expression_tree::Expression;
    if let Expression::Cast { from, .. } = expression {
        return fill_from_expression(from, fill, window);
    }
    let number = |e: &Expression| -> Option<f32> {
        let value: f32 = eval(e, window)?.try_into().ok()?;
        value.is_finite().then_some(value)
    };
    let (stops, angle, center, radius) = match expression {
        Expression::LinearGradient { angle, stops } => (stops, Some(&**angle), None, None),
        Expression::RadialGradient { stops, center, radius } => {
            (stops, None, center.as_ref(), radius.as_ref())
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
    if let Some((radius, _)) = radius {
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
