// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![deny(clippy::missing_panics_doc)]

use std::collections::HashMap;
use std::rc::Rc;

use smol_str::SmolStr;

use i_slint_compiler::{builtin_functions, expression_tree, expression_types, langtype};
use slint::Model;
use slint_interpreter::{Value, ValueType};

#[derive(Default)]
struct EvalLocalContext {
    local_variables: HashMap<SmolStr, Value>,
    return_value: Option<Value>,
    recursion_count: usize,
    window_adapter: Option<Rc<dyn slint::platform::WindowAdapter>>,
}

impl EvalLocalContext {
    /// The context of the preview window, or the thread's if there is no window.
    fn context(&self) -> Option<i_slint_core::SlintContext> {
        self.window_adapter
            .as_ref()
            .and_then(|adapter| {
                i_slint_core::window::WindowInner::from_pub(adapter.window()).try_context().cloned()
            })
            .or_else(i_slint_core::SlintContext::current)
    }
}

/// If we only care about a specified field
///
/// For example, if we need to evaluate `<expression>.foo.bar` we would have something like
/// `FieldFilter { field: "bar", parent: Some(FieldFilter { field: "foo", parent: None }) }`
struct FieldFilter<'a> {
    field: &'a str,
    parent: Option<&'a FieldFilter<'a>>,
}

fn eval_expression(
    expression: &expression_tree::Expression,
    local_context: &mut EvalLocalContext,
    field_filter: Option<&FieldFilter>,
) -> slint_interpreter::Value {
    use expression_tree::Expression;

    match expression {
        Expression::StringLiteral(s) => Value::String(s.as_str().into()),
        Expression::NumberLiteral(n, _unit) => Value::Number(*n),
        Expression::BoolLiteral(b) => Value::Bool(*b),
        Expression::StructFieldAccess { base, name } => {
            if let Value::Struct(o) = eval_expression(
                base,
                local_context,
                Some(&FieldFilter { field: name, parent: field_filter }),
            ) {
                o.get_field(name).cloned().unwrap_or_default()
            } else {
                Value::Void
            }
        }
        Expression::PropertyReference(source) => {
            let elem = source.element();
            let elem = elem.borrow();
            if let Some(binding) = elem.binding_cell_including_synthetic(source.name()) {
                let binding = binding.borrow();
                let mut ctx = EvalLocalContext {
                    recursion_count: local_context.recursion_count + 1,
                    window_adapter: local_context.window_adapter.clone(),
                    ..Default::default()
                };
                if ctx.recursion_count > 20 {
                    return Value::Void;
                }
                eval_expression(&binding.expression, &mut ctx, field_filter)
            } else {
                Value::Void
            }
        }
        Expression::Cast { from, to } => {
            let v = eval_expression(from, local_context, field_filter);
            match (v, to) {
                (Value::Number(n), langtype::Type::Int32) => Value::Number(n.trunc()),
                (Value::Number(n), langtype::Type::String) => {
                    Value::String(match local_context.context() {
                        Some(context) => context.format_number(n),
                        None => i_slint_core::string::shared_string_from_number_unlocalized(n),
                    })
                }
                (Value::Number(n), langtype::Type::Color) => {
                    slint::Color::from_argb_encoded(n as u32).into()
                }
                (Value::Brush(brush), langtype::Type::Color) => brush.color().into(),
                (Value::EnumerationValue(_, val), langtype::Type::String) => {
                    Value::String(val.into())
                }
                (v, _) => v,
            }
        }
        Expression::CodeBlock(sub) => {
            let mut v = Value::Void;
            for e in sub.iter() {
                v = eval_expression(e, local_context, field_filter);
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
        Expression::BinaryExpression { lhs, rhs, op, .. } => {
            let lhs = eval_expression(lhs, local_context, None);
            let rhs = eval_expression(rhs, local_context, None);

            match (op, lhs, rhs) {
                (_, Value::Void, _) => Value::Void,
                (_, _, Value::Void) => Value::Void,
                ('+', Value::String(mut a), Value::String(b)) => {
                    a.push_str(b.as_str());
                    Value::String(a)
                }
                ('+', Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                ('+', a @ Value::Struct(_), b @ Value::Struct(_)) => {
                    let a: Option<i_slint_core::layout::LayoutInfo> = a.try_into().ok();
                    let b: Option<i_slint_core::layout::LayoutInfo> = b.try_into().ok();
                    if let (Some(a), Some(b)) = (a, b) { a.merge(&b).into() } else { Value::Void }
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
            let sub = eval_expression(sub, local_context, None);
            match (sub, op) {
                (Value::Number(a), '+') => Value::Number(a),
                (Value::Number(a), '-') => Value::Number(-a),
                (Value::Bool(a), '!') => Value::Bool(!a),
                (_, _) => Value::Void,
            }
        }
        Expression::Condition { true_expr, false_expr, condition, .. } => {
            let condition = eval_expression(condition, local_context, None);
            if condition.try_into().unwrap_or(true) {
                eval_expression(true_expr, local_context, field_filter)
            } else {
                eval_expression(false_expr, local_context, field_filter)
            }
        }
        Expression::Array { values, .. } => {
            Value::Model(slint::ModelRc::new(i_slint_core::model::SharedVectorModel::from(
                values
                    .iter()
                    .map(|e| eval_expression(e, local_context, None))
                    .collect::<slint::SharedVector<_>>(),
            )))
        }
        Expression::Struct { values, .. } => {
            Value::Struct(if let Some(field_filter) = field_filter {
                values
                    .get(field_filter.field)
                    .map(|v| {
                        (
                            field_filter.field.to_string(),
                            eval_expression(v, local_context, field_filter.parent),
                        )
                    })
                    .into_iter()
                    .collect()
            } else {
                values
                    .iter()
                    .map(|(k, v)| (k.to_string(), eval_expression(v, local_context, None)))
                    .collect()
            })
        }
        Expression::StoreLocalVariable { name, value } => {
            let value = eval_expression(value, local_context, None);
            local_context.local_variables.insert(name.clone(), value);
            Value::Void
        }
        Expression::ReadLocalVariable { name, .. } => {
            local_context.local_variables.get(name).cloned().unwrap_or_default()
        }
        Expression::EasingCurve(curve) => Value::EasingCurve(match curve {
            expression_types::EasingCurve::Linear => i_slint_core::animations::EasingCurve::Linear,
            expression_types::EasingCurve::EaseInElastic => {
                i_slint_core::animations::EasingCurve::EaseInElastic
            }
            expression_types::EasingCurve::EaseOutElastic => {
                i_slint_core::animations::EasingCurve::EaseOutElastic
            }
            expression_types::EasingCurve::EaseInOutElastic => {
                i_slint_core::animations::EasingCurve::EaseInOutElastic
            }
            expression_types::EasingCurve::EaseInBounce => {
                i_slint_core::animations::EasingCurve::EaseInBounce
            }
            expression_types::EasingCurve::EaseOutBounce => {
                i_slint_core::animations::EasingCurve::EaseOutBounce
            }
            expression_types::EasingCurve::EaseInOutBounce => {
                i_slint_core::animations::EasingCurve::EaseInOutBounce
            }
            expression_types::EasingCurve::CubicBezier(a, b, c, d) => {
                i_slint_core::animations::EasingCurve::CubicBezier([*a, *b, *c, *d])
            }
            expression_types::EasingCurve::Spring(a) => {
                i_slint_core::animations::EasingCurve::Spring(*a)
            }
        }),
        Expression::LinearGradient { angle, stops } => {
            let angle = eval_expression(angle, local_context, None);
            Value::Brush(slint::Brush::LinearGradient(
                i_slint_core::graphics::LinearGradientBrush::new(
                    angle.try_into().unwrap_or_default(),
                    stops.iter().map(|(color, stop)| {
                        let color = eval_expression(color, local_context, None)
                            .try_into()
                            .unwrap_or_default();
                        let position = eval_expression(stop, local_context, None)
                            .try_into()
                            .unwrap_or_default();
                        i_slint_core::graphics::GradientStop { color, position }
                    }),
                ),
            ))
        }
        Expression::RadialGradient { stops, center, radius } => {
            let mut gradient = i_slint_core::graphics::RadialGradientBrush::new_circle(
                stops.iter().map(|(color, stop)| {
                    let color =
                        eval_expression(color, local_context, None).try_into().unwrap_or_default();
                    let position =
                        eval_expression(stop, local_context, None).try_into().unwrap_or_default();
                    i_slint_core::graphics::GradientStop { color, position }
                }),
            );
            if let Some((cx, cy)) = center {
                let cx: f32 =
                    eval_expression(cx, local_context, None).try_into().unwrap_or_default();
                let cy: f32 =
                    eval_expression(cy, local_context, None).try_into().unwrap_or_default();
                gradient = gradient.with_center(cx, cy);
            }
            if let Some(radius) = radius {
                let r: f32 =
                    eval_expression(radius, local_context, None).try_into().unwrap_or_default();
                gradient = gradient.with_radius(r);
            }
            Value::Brush(slint::Brush::RadialGradient(gradient))
        }
        Expression::ConicGradient { from_angle, stops, center } => {
            let mut gradient = i_slint_core::graphics::ConicGradientBrush::new(
                eval_expression(from_angle, local_context, None).try_into().unwrap_or_default(),
                stops.iter().map(|(color, stop)| {
                    let color =
                        eval_expression(color, local_context, None).try_into().unwrap_or_default();
                    let position =
                        eval_expression(stop, local_context, None).try_into().unwrap_or_default();
                    i_slint_core::graphics::GradientStop { color, position }
                }),
            );
            if let Some((cx, cy)) = center {
                let cx: f32 =
                    eval_expression(cx, local_context, None).try_into().unwrap_or_default();
                let cy: f32 =
                    eval_expression(cy, local_context, None).try_into().unwrap_or_default();
                gradient = gradient.with_center(cx, cy);
            }
            Value::Brush(slint::Brush::ConicGradient(gradient))
        }
        Expression::EnumerationValue(value) => {
            Value::EnumerationValue(value.enumeration.name.to_string(), value.to_string())
        }
        Expression::ReturnStatement(x) => {
            let val =
                x.as_ref().map_or(Value::Void, |x| eval_expression(x, local_context, field_filter));
            if local_context.return_value.is_none() {
                local_context.return_value = Some(val);
            }
            local_context.return_value.clone().unwrap_or_default()
        }
        Expression::MinMax { ty: _, op, lhs, rhs } => {
            let Value::Number(lhs) = eval_expression(lhs, local_context, None) else {
                return local_context.return_value.clone().unwrap_or_default();
            };
            let Value::Number(rhs) = eval_expression(rhs, local_context, None) else {
                return local_context.return_value.clone().unwrap_or_default();
            };
            match op {
                expression_types::MinMaxOp::Min => Value::Number(lhs.min(rhs)),
                expression_types::MinMaxOp::Max => Value::Number(lhs.max(rhs)),
            }
        }
        Expression::CornerShape(shape) => {
            Value::CornerShape(slint_interpreter::corner_shape(shape))
        }
        _ => Value::Void,
    }
}

/// Tries to evaluate a `syntax_nodes::Expression` into an `slint_interpreter::Value`
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
    window_adapter: Option<&Rc<dyn slint::platform::WindowAdapter>>,
) -> Option<slint_interpreter::Value> {
    let mut ctx =
        EvalLocalContext { window_adapter: window_adapter.cloned(), ..Default::default() };
    let value = eval_expression(expression, &mut ctx, None);

    (value.value_type() != ValueType::Void).then_some(value)
}

fn handle_builtin_function(
    f: &builtin_functions::BuiltinFunction,
    arguments: &[expression_tree::Expression],
    local_context: &mut EvalLocalContext,
) -> slint_interpreter::Value {
    use i_slint_compiler::builtin_functions::BuiltinFunction;

    if f.is_pure() && f.runtime_function().is_some() {
        let context = f.runtime_function_takes_context().then(|| local_context.context()).flatten();
        let args = arguments.iter().map(|a| eval_expression(a, local_context, None)).collect();
        if let Some(value) =
            slint_interpreter::call_runtime_builtin_function(f, context.as_ref(), None, args)
        {
            return value;
        }
    }
    match f {
        BuiltinFunction::ImageSize => {
            if arguments.len() != 1 {
                return Value::Void;
            }
            if let Value::Image(img) = eval_expression(&arguments[0], local_context, None) {
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
            match eval_expression(&arguments[0], local_context, None) {
                Value::Model(model) => Value::Number(model.row_count() as f64),
                _ => Value::Void,
            }
        }
        BuiltinFunction::ArrayPush => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ArrayPush")
            }

            let model = match eval_expression(&arguments[0], local_context, None) {
                Value::Model(m) => m,
                _ => panic!("First argument not an array: {:?}", arguments[0]),
            };
            let value = eval_expression(&arguments[1], local_context, None);

            let _ = model.push_row(value);

            Value::Void
        }
        BuiltinFunction::ArrayRemove => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ArrayRemove")
            }

            let model = match eval_expression(&arguments[0], local_context, None) {
                Value::Model(m) => m,
                _ => panic!("First argument not an array: {:?}", arguments[0]),
            };

            let index = match eval_expression(&arguments[1], local_context, None) {
                Value::Number(i) => i,
                _ => panic!("Second argument not an integer: {:?}", arguments[1]),
            };

            if let Ok(index) = usize::try_from(index as i64) {
                let _ = model.remove_row(index);
            }

            Value::Void
        }
        BuiltinFunction::ArrayInsert => {
            if arguments.len() != 3 {
                panic!("internal error: incorrect argument count to ArrayInsert")
            }

            let model = match eval_expression(&arguments[0], local_context, None) {
                Value::Model(m) => m,
                _ => panic!("First argument not an array: {:?}", arguments[0]),
            };
            let index = match eval_expression(&arguments[1], local_context, None) {
                Value::Number(i) => i,
                _ => panic!("Second argument not an integer: {:?}", arguments[1]),
            };

            let value = eval_expression(&arguments[2], local_context, None);
            if let Ok(index) = usize::try_from(index as i64) {
                let _ = model.insert_row(index, value);
            }

            Value::Void
        }
        BuiltinFunction::ColorScheme => {
            local_context.window_adapter.as_ref().map_or(Value::Void, |win| {
                let inner = i_slint_core::window::WindowInner::from_pub(win.window());
                let root = inner.component();
                inner.context().color_scheme(Some(&root)).into()
            })
        }
        BuiltinFunction::DateNow => Value::Model(slint::ModelRc::new(slint::VecModel::from(
            i_slint_core::date_time::date_now()
                .into_iter()
                .map(|x| Value::Number(x as f64))
                .collect::<Vec<_>>(),
        ))),
        BuiltinFunction::ParseDate => {
            let d: slint::SharedString =
                eval_expression(&arguments[0], local_context, None).try_into().unwrap_or_default();
            let f: slint::SharedString =
                eval_expression(&arguments[1], local_context, None).try_into().unwrap_or_default();

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
                eval_expression(&arguments[0], local_context, None).try_into().unwrap_or_default();
            let context: slint::SharedString =
                eval_expression(&arguments[1], local_context, None).try_into().unwrap_or_default();
            let domain: slint::SharedString =
                eval_expression(&arguments[2], local_context, None).try_into().unwrap_or_default();
            let args = eval_expression(&arguments[3], local_context, None);
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
                eval_expression(&arguments[4], local_context, None).try_into().unwrap_or_default(),
                &slint::SharedString::try_from(eval_expression(&arguments[5], local_context, None))
                    .unwrap_or_default(),
            ))
        }
        _ => Value::Void,
    }
}

#[cfg(test)]
mod tests {
    // ui/palette.rs covers large parts of this functionality in its tests
}
