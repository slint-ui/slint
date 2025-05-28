// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;

use smol_str::SmolStr;

use i_slint_compiler::{expression_tree, langtype};
use slint::Model;
use slint_interpreter::{Value, ValueType};

#[derive(Default)]
struct EvalLocalContext {
    local_variables: HashMap<SmolStr, Value>,
    return_value: Option<Value>,
}

fn eval_expression(
    expression: &expression_tree::Expression,
    local_context: &mut EvalLocalContext,
) -> slint_interpreter::Value {
    use expression_tree::Expression;

    match expression {
        Expression::StringLiteral(s) => Value::String(s.as_str().into()),
        Expression::NumberLiteral(n, unit) => Value::Number(unit.normalize(*n)),
        Expression::BoolLiteral(b) => Value::Bool(*b),
        Expression::StructFieldAccess { base, name } => {
            if let Value::Struct(o) = eval_expression(base, local_context) {
                o.get_field(name).cloned().unwrap_or_default()
            } else {
                Value::Void
            }
        }
        Expression::PropertyReference(source) => {
            let elem = source.element();
            let elem = elem.borrow();
            if let Some(binding) = elem.bindings.get(source.name()) {
                let binding = binding.borrow();
                eval_expression(&binding.expression, local_context)
            } else {
                Value::Void
            }
        }
        Expression::Cast { from, to } => {
            let v = eval_expression(from, local_context);
            match (v, to) {
                (Value::Number(n), langtype::Type::Int32) => Value::Number(n.trunc()),
                (Value::Number(n), langtype::Type::String) => {
                    Value::String(i_slint_core::string::shared_string_from_number(n))
                }
                (Value::Number(n), langtype::Type::Color) => {
                    slint::Color::from_argb_encoded(n as u32).into()
                }
                (Value::Brush(brush), langtype::Type::Color) => brush.color().into(),
                (v, _) => v,
            }
        }
        Expression::CodeBlock(sub) => {
            let mut v = Value::Void;
            for e in sub {
                v = eval_expression(e, local_context);
                if let Some(r) = &local_context.return_value {
                    return r.clone();
                }
            }
            v
        }
        Expression::FunctionCall {
            function: expression_tree::Callable::Builtin(f),
            arguments,
            source_location: _,
        } => handle_builtin_function(f, arguments, local_context),
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = eval_expression(lhs, local_context);
            let rhs = eval_expression(rhs, local_context);

            match (op, lhs, rhs) {
                ('+', Value::String(mut a), Value::String(b)) => {
                    a.push_str(b.as_str());
                    Value::String(a)
                }
                ('+', Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                ('+', a @ Value::Struct(_), b @ Value::Struct(_)) => {
                    let a: Option<i_slint_core::layout::LayoutInfo> = a.try_into().ok();
                    let b: Option<i_slint_core::layout::LayoutInfo> = b.try_into().ok();
                    if let (Some(a), Some(b)) = (a, b) {
                        a.merge(&b).into()
                    } else {
                        Value::Void
                    }
                }
                ('-', Value::Number(a), Value::Number(b)) => Value::Number(a - b),
                ('/', Value::Number(a), Value::Number(b)) => Value::Number(a / b),
                ('*', Value::Number(a), Value::Number(b)) => Value::Number(a * b),
                ('<', Value::Number(a), Value::Number(b)) => Value::Bool(a < b),
                ('>', Value::Number(a), Value::Number(b)) => Value::Bool(a > b),
                ('≤', Value::Number(a), Value::Number(b)) => Value::Bool(a <= b),
                ('≥', Value::Number(a), Value::Number(b)) => Value::Bool(a >= b),
                ('<', Value::String(a), Value::String(b)) => Value::Bool(a < b),
                ('>', Value::String(a), Value::String(b)) => Value::Bool(a > b),
                ('≤', Value::String(a), Value::String(b)) => Value::Bool(a <= b),
                ('≥', Value::String(a), Value::String(b)) => Value::Bool(a >= b),
                ('=', a, b) => Value::Bool(a == b),
                ('!', a, b) => Value::Bool(a != b),
                ('&', Value::Bool(a), Value::Bool(b)) => Value::Bool(a && b),
                ('|', Value::Bool(a), Value::Bool(b)) => Value::Bool(a || b),
                (_, _, _) => Value::Void,
            }
        }
        Expression::UnaryOp { sub, op } => {
            let sub = eval_expression(sub, local_context);
            match (sub, op) {
                (Value::Number(a), '+') => Value::Number(a),
                (Value::Number(a), '-') => Value::Number(-a),
                (Value::Bool(a), '!') => Value::Bool(!a),
                (_, _) => Value::Void,
            }
        }
        Expression::Condition { true_expr, .. } => eval_expression(true_expr, local_context),
        Expression::Array { values, .. } => {
            Value::Model(slint::ModelRc::new(i_slint_core::model::SharedVectorModel::from(
                values
                    .iter()
                    .map(|e| eval_expression(e, local_context))
                    .collect::<slint::SharedVector<_>>(),
            )))
        }
        Expression::Struct { values, .. } => Value::Struct(
            values
                .iter()
                .map(|(k, v)| (k.to_string(), eval_expression(v, local_context)))
                .collect(),
        ),
        Expression::StoreLocalVariable { name, value } => {
            let value = eval_expression(value, local_context);
            local_context.local_variables.insert(name.clone(), value);
            Value::Void
        }
        Expression::ReadLocalVariable { name, .. } => {
            local_context.local_variables.get(name).unwrap().clone()
        }
        Expression::EasingCurve(curve) => Value::EasingCurve(match curve {
            expression_tree::EasingCurve::Linear => i_slint_core::animations::EasingCurve::Linear,
            expression_tree::EasingCurve::EaseInElastic => {
                i_slint_core::animations::EasingCurve::EaseInElastic
            }
            expression_tree::EasingCurve::EaseOutElastic => {
                i_slint_core::animations::EasingCurve::EaseOutElastic
            }
            expression_tree::EasingCurve::EaseInOutElastic => {
                i_slint_core::animations::EasingCurve::EaseInOutElastic
            }
            expression_tree::EasingCurve::EaseInBounce => {
                i_slint_core::animations::EasingCurve::EaseInBounce
            }
            expression_tree::EasingCurve::EaseOutBounce => {
                i_slint_core::animations::EasingCurve::EaseOutBounce
            }
            expression_tree::EasingCurve::EaseInOutBounce => {
                i_slint_core::animations::EasingCurve::EaseInOutBounce
            }
            expression_tree::EasingCurve::CubicBezier(a, b, c, d) => {
                i_slint_core::animations::EasingCurve::CubicBezier([*a, *b, *c, *d])
            }
        }),
        Expression::LinearGradient { angle, stops } => {
            let angle = eval_expression(angle, local_context);
            Value::Brush(slint::Brush::LinearGradient(
                i_slint_core::graphics::LinearGradientBrush::new(
                    angle.try_into().unwrap(),
                    stops.iter().map(|(color, stop)| {
                        let color = eval_expression(color, local_context).try_into().unwrap();
                        let position = eval_expression(stop, local_context).try_into().unwrap();
                        i_slint_core::graphics::GradientStop { color, position }
                    }),
                ),
            ))
        }
        Expression::RadialGradient { stops } => Value::Brush(slint::Brush::RadialGradient(
            i_slint_core::graphics::RadialGradientBrush::new_circle(stops.iter().map(
                |(color, stop)| {
                    let color = eval_expression(color, local_context).try_into().unwrap();
                    let position = eval_expression(stop, local_context).try_into().unwrap();
                    i_slint_core::graphics::GradientStop { color, position }
                },
            )),
        )),
        Expression::EnumerationValue(value) => {
            Value::EnumerationValue(value.enumeration.name.to_string(), value.to_string())
        }
        Expression::ReturnStatement(x) => {
            let val = x.as_ref().map_or(Value::Void, |x| eval_expression(x, local_context));
            if local_context.return_value.is_none() {
                local_context.return_value = Some(val);
            }
            local_context.return_value.clone().unwrap()
        }
        Expression::MinMax { ty: _, op, lhs, rhs } => {
            let Value::Number(lhs) = eval_expression(lhs, local_context) else {
                return local_context
                    .return_value
                    .clone()
                    .expect("minmax lhs expression did not evaluate to number");
            };
            let Value::Number(rhs) = eval_expression(rhs, local_context) else {
                return local_context
                    .return_value
                    .clone()
                    .expect("minmax rhs expression did not evaluate to number");
            };
            match op {
                expression_tree::MinMaxOp::Min => Value::Number(lhs.min(rhs)),
                expression_tree::MinMaxOp::Max => Value::Number(lhs.max(rhs)),
            }
        }
        _ => Value::Void,
    }
}

/// Tries to evaluate a `syntax_nodes::BindingExpression` into an `slint_interpreter::Value`
///
/// This has no access to any runtime information, so the evaluation is an approximation to the
/// real value only.
///
/// E.g. It will always evaluate the `true` branch of any condition and takes other shortcuts as well.
/// It might also just fail to evaluate entirely, returning `None` in that case.
///
/// The purpose of this function is to be able to show some not totally useless representation of
/// property values in the UI.
pub fn fully_eval_expression_tree_expression(
    expression: &expression_tree::Expression,
) -> Option<slint_interpreter::Value> {
    let mut ctx = EvalLocalContext::default();
    let value = eval_expression(expression, &mut ctx);

    (value.value_type() != ValueType::Void).then_some(value)
}

fn handle_builtin_function(
    f: &expression_tree::BuiltinFunction,
    arguments: &[expression_tree::Expression],
    local_context: &mut EvalLocalContext,
) -> slint_interpreter::Value {
    use i_slint_compiler::expression_tree::BuiltinFunction;

    match f {
        BuiltinFunction::Mod => {
            let mut to_num = |e| -> f64 { eval_expression(e, local_context).try_into().unwrap() };
            Value::Number(to_num(&arguments[0]).rem_euclid(to_num(&arguments[1])))
        }
        BuiltinFunction::Round => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.round())
        }
        BuiltinFunction::Ceil => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.ceil())
        }
        BuiltinFunction::Floor => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.floor())
        }
        BuiltinFunction::Sqrt => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.sqrt())
        }
        BuiltinFunction::Abs => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.abs())
        }
        BuiltinFunction::Sin => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.to_radians().sin())
        }
        BuiltinFunction::Cos => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.to_radians().cos())
        }
        BuiltinFunction::Tan => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.to_radians().tan())
        }
        BuiltinFunction::ASin => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.asin().to_degrees())
        }
        BuiltinFunction::ACos => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.acos().to_degrees())
        }
        BuiltinFunction::ATan => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.atan().to_degrees())
        }
        BuiltinFunction::ATan2 => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let y: f64 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Number(x.atan2(y).to_degrees())
        }
        BuiltinFunction::Log => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let y: f64 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Number(x.log(y))
        }
        BuiltinFunction::Ln => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.ln())
        }
        BuiltinFunction::Pow => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let y: f64 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Number(x.powf(y))
        }
        BuiltinFunction::Exp => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            Value::Number(x.exp())
        }
        BuiltinFunction::ToFixed => {
            let n: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let digits: i32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            let digits: usize = digits.max(0) as usize;
            Value::String(i_slint_core::string::shared_string_from_number_fixed(n, digits))
        }
        BuiltinFunction::ToPrecision => {
            let n: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let precision: i32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            let precision: usize = precision.max(0) as usize;
            Value::String(i_slint_core::string::shared_string_from_number_precision(n, precision))
        }
        BuiltinFunction::StringIsFloat => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::Bool(<f64 as core::str::FromStr>::from_str(s.as_str()).is_ok())
            } else {
                Value::Void
            }
        }
        BuiltinFunction::StringToFloat => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::Number(core::str::FromStr::from_str(s.as_str()).unwrap_or(0.))
            } else {
                Value::Void
            }
        }
        BuiltinFunction::StringIsEmpty => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::Bool(s.is_empty())
            } else {
                Value::Void
            }
        }
        BuiltinFunction::StringToLowercase => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::String(s.to_lowercase().into())
            } else {
                Value::Void
            }
        }
        BuiltinFunction::StringToUppercase => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::String(s.to_uppercase().into())
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorRgbaStruct => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                let color = brush.color();
                let values = IntoIterator::into_iter([
                    ("red".to_string(), Value::Number(color.red().into())),
                    ("green".to_string(), Value::Number(color.green().into())),
                    ("blue".to_string(), Value::Number(color.blue().into())),
                    ("alpha".to_string(), Value::Number(color.alpha().into())),
                ])
                .collect();
                Value::Struct(values)
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorHsvaStruct => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                let color = brush.color().to_hsva();
                let values = IntoIterator::into_iter([
                    ("hue".to_string(), Value::Number(color.hue.into())),
                    ("saturation".to_string(), Value::Number(color.saturation.into())),
                    ("value".to_string(), Value::Number(color.value.into())),
                    ("alpha".to_string(), Value::Number(color.alpha.into())),
                ])
                .collect();
                Value::Struct(values)
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorBrighter => {
            if arguments.len() != 2 {
                return Value::Void;
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.brighter(factor as _).into()
                } else {
                    Value::Void
                }
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorDarker => {
            if arguments.len() != 2 {
                return Value::Void;
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.darker(factor as _).into()
                } else {
                    Value::Void
                }
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorTransparentize => {
            if arguments.len() != 2 {
                return Value::Void;
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.transparentize(factor as _).into()
                } else {
                    Value::Void
                }
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorMix => {
            if arguments.len() != 3 {
                return Value::Void;
            }

            let arg0 = eval_expression(&arguments[0], local_context);
            let arg1 = eval_expression(&arguments[1], local_context);
            let arg2 = eval_expression(&arguments[2], local_context);

            if !matches!(arg0, Value::Brush(slint::Brush::SolidColor(_))) {
                return Value::Void;
            }
            if !matches!(arg1, Value::Brush(slint::Brush::SolidColor(_))) {
                return Value::Void;
            }
            if !matches!(arg2, Value::Number(_)) {
                return Value::Void;
            }

            let (
                Value::Brush(slint::Brush::SolidColor(color_a)),
                Value::Brush(slint::Brush::SolidColor(color_b)),
                Value::Number(factor),
            ) = (arg0, arg1, arg2)
            else {
                unreachable!()
            };

            color_a.mix(&color_b, factor as _).into()
        }
        BuiltinFunction::ColorWithAlpha => {
            if arguments.len() != 2 {
                return Value::Void;
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.with_alpha(factor as _).into()
                } else {
                    Value::Void
                }
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ImageSize => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::Image(img) = eval_expression(&arguments[0], local_context) {
                let size = img.size();
                let values = IntoIterator::into_iter([
                    ("width".to_string(), Value::Number(size.width as f64)),
                    ("height".to_string(), Value::Number(size.height as f64)),
                ])
                .collect();
                Value::Struct(values)
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ArrayLength => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            match eval_expression(&arguments[0], local_context) {
                Value::Model(model) => Value::Number(model.row_count() as f64),
                _ => Value::Void,
            }
        }
        BuiltinFunction::Rgb => {
            let r: i32 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let g: i32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            let b: i32 = eval_expression(&arguments[2], local_context).try_into().unwrap();
            let a: f32 = eval_expression(&arguments[3], local_context).try_into().unwrap();
            let r: u8 = r.clamp(0, 255) as u8;
            let g: u8 = g.clamp(0, 255) as u8;
            let b: u8 = b.clamp(0, 255) as u8;
            let a: u8 = (255. * a).clamp(0., 255.) as u8;
            Value::Brush(slint::Brush::SolidColor(slint::Color::from_argb_u8(a, r, g, b)))
        }
        BuiltinFunction::Hsv => {
            let h: f32 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let s: f32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            let v: f32 = eval_expression(&arguments[2], local_context).try_into().unwrap();
            let a: f32 = eval_expression(&arguments[3], local_context).try_into().unwrap();
            let a = (1. * a).clamp(0., 1.);
            Value::Brush(slint::Brush::SolidColor(slint::Color::from_hsva(h, s, v, a)))
        }
        BuiltinFunction::MonthDayCount => {
            let m: u32 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let y: i32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Number(i_slint_core::date_time::month_day_count(m, y).unwrap_or(0) as f64)
        }
        BuiltinFunction::MonthOffset => {
            let m: u32 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let y: i32 = eval_expression(&arguments[1], local_context).try_into().unwrap();

            Value::Number(i_slint_core::date_time::month_offset(m, y) as f64)
        }
        BuiltinFunction::FormatDate => {
            let f: slint::SharedString =
                eval_expression(&arguments[0], local_context).try_into().unwrap();
            let d: u32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            let m: u32 = eval_expression(&arguments[2], local_context).try_into().unwrap();
            let y: i32 = eval_expression(&arguments[3], local_context).try_into().unwrap();

            Value::String(i_slint_core::date_time::format_date(&f, d, m, y))
        }
        BuiltinFunction::DateNow => Value::Model(slint::ModelRc::new(slint::VecModel::from(
            i_slint_core::date_time::date_now()
                .into_iter()
                .map(|x| Value::Number(x as f64))
                .collect::<Vec<_>>(),
        ))),
        BuiltinFunction::ValidDate => {
            let d: slint::SharedString =
                eval_expression(&arguments[0], local_context).try_into().unwrap();
            let f: slint::SharedString =
                eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Bool(i_slint_core::date_time::parse_date(d.as_str(), f.as_str()).is_some())
        }
        BuiltinFunction::ParseDate => {
            let d: slint::SharedString =
                eval_expression(&arguments[0], local_context).try_into().unwrap();
            let f: slint::SharedString =
                eval_expression(&arguments[1], local_context).try_into().unwrap();

            Value::Model(slint::ModelRc::new(
                i_slint_core::date_time::parse_date(d.as_str(), f.as_str())
                    .map(|x| {
                        slint::VecModel::from(
                            x.into_iter().map(|x| Value::Number(x as f64)).collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_default(),
            ))
        }
        BuiltinFunction::Translate => {
            let original: slint::SharedString =
                eval_expression(&arguments[0], local_context).try_into().unwrap();
            let context: slint::SharedString =
                eval_expression(&arguments[1], local_context).try_into().unwrap();
            let domain: slint::SharedString =
                eval_expression(&arguments[2], local_context).try_into().unwrap();
            let args = eval_expression(&arguments[3], local_context);
            let Value::Model(args) = args else {
                return Value::Void;
            };
            struct StringModelWrapper(slint::ModelRc<Value>);
            impl i_slint_core::translations::FormatArgs for StringModelWrapper {
                type Output<'a> = slint::SharedString;
                fn from_index(&self, index: usize) -> Option<slint::SharedString> {
                    self.0.row_data(index).map(|x| x.try_into().unwrap_or_default())
                }
            }
            Value::String(i_slint_core::translations::translate(
                &original,
                &context,
                &domain,
                &StringModelWrapper(args),
                eval_expression(&arguments[4], local_context).try_into().unwrap(),
                &slint::SharedString::try_from(eval_expression(&arguments[5], local_context))
                    .unwrap(),
            ))
        }
        BuiltinFunction::Use24HourFormat => {
            Value::Bool(i_slint_core::date_time::use_24_hour_format())
        }
        BuiltinFunction::DetectOperatingSystem => i_slint_core::detect_operating_system().into(),
        _ => Value::Void,
    }
}

#[cfg(test)]
mod tests {
    // ui/palette.rs covers large parts of this functionality in its tests
}
