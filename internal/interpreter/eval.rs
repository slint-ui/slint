// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::{SetPropertyError, Struct, Value};
use crate::dynamic_item_tree::{CallbackHandler, InstanceRef};
use core::pin::Pin;
use corelib::graphics::{
    ConicGradientBrush, GradientStop, LinearGradientBrush, PathElement, RadialGradientBrush,
};
use corelib::items::{ColorScheme, ItemRef, MenuEntry, PropertyAnimation};
use corelib::menus::{Menu, MenuFromItemTree, MenuVTable};
use corelib::model::{Model, ModelExt, ModelRc, VecModel};
use corelib::rtti::AnimatedBindingKind;
use corelib::window::WindowInner;
use corelib::{Brush, Color, PathData, SharedString, SharedVector};
use i_slint_compiler::expression_tree::{
    BuiltinFunction, Callable, EasingCurve, Expression, MinMaxOp, Path as ExprPath,
    PathElement as ExprPathElement,
};
use i_slint_compiler::langtype::Type;
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core as corelib;
use i_slint_core::input::FocusReason;
use i_slint_core::items::ItemRc;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::rc::Rc;

pub trait ErasedPropertyInfo {
    fn get(&self, item: Pin<ItemRef>) -> Value;
    fn set(
        &self,
        item: Pin<ItemRef>,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()>;
    fn set_binding(
        &self,
        item: Pin<ItemRef>,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    );
    fn offset(&self) -> usize;

    /// Safety: Property2 must be a (pinned) pointer to a `Property<T>`
    /// where T is the same T as the one represented by this property.
    unsafe fn link_two_ways(&self, item: Pin<ItemRef>, property2: *const ());
}

impl<Item: vtable::HasStaticVTable<corelib::items::ItemVTable>> ErasedPropertyInfo
    for &'static dyn corelib::rtti::PropertyInfo<Item, Value>
{
    fn get(&self, item: Pin<ItemRef>) -> Value {
        (*self).get(ItemRef::downcast_pin(item).unwrap()).unwrap()
    }
    fn set(
        &self,
        item: Pin<ItemRef>,
        value: Value,
        animation: Option<PropertyAnimation>,
    ) -> Result<(), ()> {
        (*self).set(ItemRef::downcast_pin(item).unwrap(), value, animation)
    }
    fn set_binding(
        &self,
        item: Pin<ItemRef>,
        binding: Box<dyn Fn() -> Value>,
        animation: AnimatedBindingKind,
    ) {
        (*self).set_binding(ItemRef::downcast_pin(item).unwrap(), binding, animation).unwrap();
    }
    fn offset(&self) -> usize {
        (*self).offset()
    }
    unsafe fn link_two_ways(&self, item: Pin<ItemRef>, property2: *const ()) {
        // Safety: ErasedPropertyInfo::link_two_ways and PropertyInfo::link_two_ways have the same safety requirement
        (*self).link_two_ways(ItemRef::downcast_pin(item).unwrap(), property2)
    }
}

pub trait ErasedCallbackInfo {
    fn call(&self, item: Pin<ItemRef>, args: &[Value]) -> Value;
    fn set_handler(&self, item: Pin<ItemRef>, handler: Box<dyn Fn(&[Value]) -> Value>);
}

impl<Item: vtable::HasStaticVTable<corelib::items::ItemVTable>> ErasedCallbackInfo
    for &'static dyn corelib::rtti::CallbackInfo<Item, Value>
{
    fn call(&self, item: Pin<ItemRef>, args: &[Value]) -> Value {
        (*self).call(ItemRef::downcast_pin(item).unwrap(), args).unwrap()
    }

    fn set_handler(&self, item: Pin<ItemRef>, handler: Box<dyn Fn(&[Value]) -> Value>) {
        (*self).set_handler(ItemRef::downcast_pin(item).unwrap(), handler).unwrap()
    }
}

impl corelib::rtti::ValueType for Value {}

#[derive(Clone)]
pub(crate) enum ComponentInstance<'a, 'id> {
    InstanceRef(InstanceRef<'a, 'id>),
    GlobalComponent(Pin<Rc<dyn crate::global_component::GlobalComponent>>),
}

/// The local variable needed for binding evaluation
pub struct EvalLocalContext<'a, 'id> {
    local_variables: HashMap<SmolStr, Value>,
    function_arguments: Vec<Value>,
    pub(crate) component_instance: InstanceRef<'a, 'id>,
    /// When Some, a return statement was executed and one must stop evaluating
    return_value: Option<Value>,
}

impl<'a, 'id> EvalLocalContext<'a, 'id> {
    pub fn from_component_instance(component: InstanceRef<'a, 'id>) -> Self {
        Self {
            local_variables: Default::default(),
            function_arguments: Default::default(),
            component_instance: component,
            return_value: None,
        }
    }

    /// Create a context for a function and passing the arguments
    pub fn from_function_arguments(
        component: InstanceRef<'a, 'id>,
        function_arguments: Vec<Value>,
    ) -> Self {
        Self {
            component_instance: component,
            function_arguments,
            local_variables: Default::default(),
            return_value: None,
        }
    }
}

/// Evaluate an expression and return a Value as the result of this expression
pub fn eval_expression(expression: &Expression, local_context: &mut EvalLocalContext) -> Value {
    if let Some(r) = &local_context.return_value {
        return r.clone();
    }
    match expression {
        Expression::Invalid => panic!("invalid expression while evaluating"),
        Expression::Uncompiled(_) => panic!("uncompiled expression while evaluating"),
        Expression::StringLiteral(s) => Value::String(s.as_str().into()),
        Expression::NumberLiteral(n, unit) => Value::Number(unit.normalize(*n)),
        Expression::BoolLiteral(b) => Value::Bool(*b),
        Expression::ElementReference(_) => todo!("Element references are only supported in the context of built-in function calls at the moment"),
        Expression::PropertyReference(nr) => {
            load_property_helper(&ComponentInstance::InstanceRef(local_context.component_instance), &nr.element(), nr.name()).unwrap()
        }
        Expression::RepeaterIndexReference { element } => load_property_helper(&ComponentInstance::InstanceRef(local_context.component_instance),
            &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
            crate::dynamic_item_tree::SPECIAL_PROPERTY_INDEX,
        )
        .unwrap(),
        Expression::RepeaterModelReference { element } => {
            let value = load_property_helper(&ComponentInstance::InstanceRef(local_context.component_instance),
                    &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
                    crate::dynamic_item_tree::SPECIAL_PROPERTY_MODEL_DATA,
                )
                .unwrap();
            if matches!(value, Value::Void) {
                // Uninitialized model data (because the model returned None) should still be initialized to the default value of the type
                default_value_for_type(&expression.ty())
            } else {
                value
            }

        },
        Expression::FunctionParameterReference { index, .. } => {
            local_context.function_arguments[*index].clone()
        }
        Expression::StructFieldAccess { base, name } => {
            if let Value::Struct(o) = eval_expression(base, local_context) {
                o.get_field(name).cloned().unwrap_or(Value::Void)
            } else {
                Value::Void
            }
        }
        Expression::ArrayIndex { array, index } => {
            let array = eval_expression(array, local_context);
            let index = eval_expression(index, local_context);
            match (array, index) {
                (Value::Model(model), Value::Number(index)) => {
                    model.row_data_tracked(index as isize as usize).unwrap_or_else(|| default_value_for_type(&expression.ty()))
                }
                _ => {
                    Value::Void
                }
            }
        }
        Expression::Cast { from, to } => {
            let v = eval_expression(from, local_context);
            match (v, to) {
                (Value::Number(n), Type::Int32) => Value::Number(n.trunc()),
                (Value::Number(n), Type::String) => {
                    Value::String(i_slint_core::string::shared_string_from_number(n))
                }
                (Value::Number(n), Type::Color) => Color::from_argb_encoded(n as u32).into(),
                (Value::Brush(brush), Type::Color) => brush.color().into(),
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
        Expression::FunctionCall { function, arguments, source_location } => match &function {
            Callable::Function(nr) => {
                let is_item_member = nr.element().borrow().native_class().is_some_and(|n| n.properties.contains_key(nr.name()));
                if is_item_member {
                    call_item_member_function(nr, local_context)
                } else {
                    let args = arguments.iter().map(|e| eval_expression(e, local_context)).collect::<Vec<_>>();
                    call_function(&ComponentInstance::InstanceRef(local_context.component_instance), &nr.element(), nr.name(), args).unwrap()
                }
            }
            Callable::Callback(nr) => {
                let args = arguments.iter().map(|e| eval_expression(e, local_context)).collect::<Vec<_>>();
                invoke_callback(&ComponentInstance::InstanceRef(local_context.component_instance), &nr.element(), nr.name(), &args).unwrap()
            }
            Callable::Builtin(f) => call_builtin_function(f.clone(), arguments, local_context, source_location),
        }
        Expression::SelfAssignment { lhs, rhs, op, .. } => {
            let rhs = eval_expression(rhs, local_context);
            eval_assignment(lhs, *op, rhs, local_context);
            Value::Void
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = eval_expression(lhs, local_context);
            let rhs = eval_expression(rhs, local_context);

            match (op, lhs, rhs) {
                ('+', Value::String(mut a), Value::String(b)) => { a.push_str(b.as_str()); Value::String(a) },
                ('+', Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                ('+', a @ Value::Struct(_), b @ Value::Struct(_)) => {
                    let a : Option<corelib::layout::LayoutInfo> = a.try_into().ok();
                    let b : Option<corelib::layout::LayoutInfo> = b.try_into().ok();
                    if let (Some(a), Some(b)) = (a, b) {
                        a.merge(&b).into()
                    } else {
                        panic!("unsupported {a:?} {op} {b:?}");
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
                (op, lhs, rhs) => panic!("unsupported {lhs:?} {op} {rhs:?}"),
            }
        }
        Expression::UnaryOp { sub, op } => {
            let sub = eval_expression(sub, local_context);
            match (sub, op) {
                (Value::Number(a), '+') => Value::Number(a),
                (Value::Number(a), '-') => Value::Number(-a),
                (Value::Bool(a), '!') => Value::Bool(!a),
                (sub, op) => panic!("unsupported {op} {sub:?}"),
            }
        }
        Expression::ImageReference{ resource_ref, nine_slice, .. } => {
            let mut image = match resource_ref {
                i_slint_compiler::expression_tree::ImageReference::None => {
                    Ok(Default::default())
                }
                i_slint_compiler::expression_tree::ImageReference::AbsolutePath(path) => {
                    let path = std::path::Path::new(path);
                    if path.starts_with("builtin:/") {
                        i_slint_compiler::fileaccess::load_file(path).and_then(|virtual_file| virtual_file.builtin_contents).map(|virtual_file| {
                            let extension = path.extension().unwrap().to_str().unwrap();
                            corelib::graphics::load_image_from_embedded_data(
                                corelib::slice::Slice::from_slice(virtual_file),
                                corelib::slice::Slice::from_slice(extension.as_bytes())
                            )
                        }).ok_or_else(Default::default)
                    } else {
                        corelib::graphics::Image::load_from_path(path)
                    }
                }
                i_slint_compiler::expression_tree::ImageReference::EmbeddedData { .. } => {
                    todo!()
                }
                i_slint_compiler::expression_tree::ImageReference::EmbeddedTexture { .. } => {
                    todo!()
                }
            }.unwrap_or_else(|_| {
                eprintln!("Could not load image {resource_ref:?}" );
                Default::default()
            });
            if let Some(n) = nine_slice {
                image.set_nine_slice_edges(n[0], n[1], n[2], n[3]);
            }
            Value::Image(image)
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            match eval_expression(condition, local_context).try_into()
                as Result<bool, _>
            {
                Ok(true) => eval_expression(true_expr, local_context),
                Ok(false) => eval_expression(false_expr, local_context),
                _ => local_context.return_value.clone().expect("conditional expression did not evaluate to boolean"),
            }
        }
        Expression::Array { values, .. } => Value::Model(
            ModelRc::new(corelib::model::SharedVectorModel::from(
                values.iter().map(|e| eval_expression(e, local_context)).collect::<SharedVector<_>>()
            )
        )),
        Expression::Struct { values, .. } => Value::Struct(
            values
                .iter()
                .map(|(k, v)| (k.to_string(), eval_expression(v, local_context)))
                .collect(),
        ),
        Expression::PathData(data)  => {
            Value::PathData(convert_path(data, local_context))
        }
        Expression::StoreLocalVariable { name, value } => {
            let value = eval_expression(value, local_context);
            local_context.local_variables.insert(name.clone(), value);
            Value::Void
        }
        Expression::ReadLocalVariable { name, .. } => {
            local_context.local_variables.get(name).unwrap().clone()
        }
        Expression::EasingCurve(curve) => Value::EasingCurve(match curve {
            EasingCurve::Linear => corelib::animations::EasingCurve::Linear,
            EasingCurve::EaseInElastic => corelib::animations::EasingCurve::EaseInElastic,
            EasingCurve::EaseOutElastic => corelib::animations::EasingCurve::EaseOutElastic,
            EasingCurve::EaseInOutElastic => corelib::animations::EasingCurve::EaseInOutElastic,
            EasingCurve::EaseInBounce => corelib::animations::EasingCurve::EaseInBounce,
            EasingCurve::EaseOutBounce => corelib::animations::EasingCurve::EaseOutBounce,
            EasingCurve::EaseInOutBounce => corelib::animations::EasingCurve::EaseInOutBounce,
            EasingCurve::CubicBezier(a, b, c, d) => {
                corelib::animations::EasingCurve::CubicBezier([*a, *b, *c, *d])
            }
        }),
        Expression::LinearGradient{angle, stops} => {
            let angle = eval_expression(angle, local_context);
            Value::Brush(Brush::LinearGradient(LinearGradientBrush::new(angle.try_into().unwrap(), stops.iter().map(|(color, stop)| {
                let color = eval_expression(color, local_context).try_into().unwrap();
                let position = eval_expression(stop, local_context).try_into().unwrap();
                GradientStop{ color, position }
            }))))
        }
        Expression::RadialGradient{stops} => {
            Value::Brush(Brush::RadialGradient(RadialGradientBrush::new_circle(stops.iter().map(|(color, stop)| {
                let color = eval_expression(color, local_context).try_into().unwrap();
                let position = eval_expression(stop, local_context).try_into().unwrap();
                GradientStop{ color, position }
            }))))
        }
        Expression::ConicGradient{stops} => {
            Value::Brush(Brush::ConicGradient(ConicGradientBrush::new(stops.iter().map(|(color, stop)| {
                let color = eval_expression(color, local_context).try_into().unwrap();
                let position = eval_expression(stop, local_context).try_into().unwrap();
                GradientStop{ color, position }
            }))))
        }
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
        Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } => {
            let cache = load_property_helper(&ComponentInstance::InstanceRef(local_context.component_instance), &layout_cache_prop.element(), layout_cache_prop.name()).unwrap();
            if let Value::LayoutCache(cache) = cache {
                if let Some(ri) = repeater_index {
                    let offset : usize = eval_expression(ri, local_context).try_into().unwrap();
                    Value::Number(cache.get((cache[*index] as usize) + offset * 2).copied().unwrap_or(0.).into())
                } else {
                    Value::Number(cache[*index].into())
                }
            } else {
                panic!("invalid layout cache")
            }
        }
        Expression::ComputeLayoutInfo(lay, o) => crate::eval_layout::compute_layout_info(lay, *o, local_context),
        Expression::SolveLayout(lay, o) => crate::eval_layout::solve_layout(lay, *o, local_context),
        Expression::MinMax { ty: _, op, lhs, rhs } => {
            let Value::Number(lhs) = eval_expression(lhs, local_context) else {
                return local_context.return_value.clone().expect("minmax lhs expression did not evaluate to number");
            };
            let Value::Number(rhs) =  eval_expression(rhs, local_context) else {
                return local_context.return_value.clone().expect("minmax rhs expression did not evaluate to number");
            };
            match op {
                MinMaxOp::Min => Value::Number(lhs.min(rhs)),
                MinMaxOp::Max => Value::Number(lhs.max(rhs)),
            }
        }
        Expression::EmptyComponentFactory => Value::ComponentFactory(Default::default()),
        Expression::DebugHook { expression, .. } => eval_expression(expression, local_context),
    }
}

fn call_builtin_function(
    f: BuiltinFunction,
    arguments: &[Expression],
    local_context: &mut EvalLocalContext,
    source_location: &Option<i_slint_compiler::diagnostics::SourceLocation>,
) -> Value {
    match f {
        BuiltinFunction::GetWindowScaleFactor => Value::Number(
            local_context.component_instance.access_window(|window| window.scale_factor()) as _,
        ),
        BuiltinFunction::GetWindowDefaultFontSize => {
            Value::Number(local_context.component_instance.access_window(|window| {
                window.window_item().unwrap().as_pin_ref().default_font_size().get()
            }) as _)
        }
        BuiltinFunction::AnimationTick => {
            Value::Number(i_slint_core::animations::animation_tick() as f64)
        }
        BuiltinFunction::Debug => {
            let to_print: SharedString =
                eval_expression(&arguments[0], local_context).try_into().unwrap();
            local_context.component_instance.description.debug_handler.borrow()(
                source_location.as_ref(),
                &to_print,
            );
            Value::Void
        }
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
        BuiltinFunction::SetFocusItem => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to SetFocusItem")
            }
            let component = local_context.component_instance;
            if let Expression::ElementReference(focus_item) = &arguments[0] {
                generativity::make_guard!(guard);

                let focus_item = focus_item.upgrade().unwrap();
                let enclosing_component =
                    enclosing_component_for_element(&focus_item, component, guard);
                let description = enclosing_component.description;

                let item_info = &description.items[focus_item.borrow().id.as_str()];

                let focus_item_comp =
                    enclosing_component.self_weak().get().unwrap().upgrade().unwrap();

                component.access_window(|window| {
                    window.set_focus_item(
                        &corelib::items::ItemRc::new(
                            vtable::VRc::into_dyn(focus_item_comp),
                            item_info.item_index(),
                        ),
                        true,
                        FocusReason::Programmatic,
                    )
                });
                Value::Void
            } else {
                panic!("internal error: argument to SetFocusItem must be an element")
            }
        }
        BuiltinFunction::ClearFocusItem => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to SetFocusItem")
            }
            let component = local_context.component_instance;
            if let Expression::ElementReference(focus_item) = &arguments[0] {
                generativity::make_guard!(guard);

                let focus_item = focus_item.upgrade().unwrap();
                let enclosing_component =
                    enclosing_component_for_element(&focus_item, component, guard);
                let description = enclosing_component.description;

                let item_info = &description.items[focus_item.borrow().id.as_str()];

                let focus_item_comp =
                    enclosing_component.self_weak().get().unwrap().upgrade().unwrap();

                component.access_window(|window| {
                    window.set_focus_item(
                        &corelib::items::ItemRc::new(
                            vtable::VRc::into_dyn(focus_item_comp),
                            item_info.item_index(),
                        ),
                        false,
                        FocusReason::Programmatic,
                    )
                });
                Value::Void
            } else {
                panic!("internal error: argument to ClearFocusItem must be an element")
            }
        }
        BuiltinFunction::ShowPopupWindow => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to ShowPopupWindow")
            }
            let component = local_context.component_instance;
            if let Expression::ElementReference(popup_window) = &arguments[0] {
                let popup_window = popup_window.upgrade().unwrap();
                let pop_comp = popup_window.borrow().enclosing_component.upgrade().unwrap();
                let parent_component = pop_comp
                    .parent_element
                    .upgrade()
                    .unwrap()
                    .borrow()
                    .enclosing_component
                    .upgrade()
                    .unwrap();
                let popup_list = parent_component.popup_windows.borrow();
                let popup =
                    popup_list.iter().find(|p| Rc::ptr_eq(&p.component, &pop_comp)).unwrap();

                generativity::make_guard!(guard);
                let enclosing_component =
                    enclosing_component_for_element(&popup.parent_element, component, guard);
                let parent_item_info = &enclosing_component.description.items
                    [popup.parent_element.borrow().id.as_str()];
                let parent_item_comp =
                    enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
                let parent_item = corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(parent_item_comp),
                    parent_item_info.item_index(),
                );

                let close_policy = Value::EnumerationValue(
                    popup.close_policy.enumeration.name.to_string(),
                    popup.close_policy.to_string(),
                )
                .try_into()
                .expect("Invalid internal enumeration representation for close policy");

                crate::dynamic_item_tree::show_popup(
                    popup_window,
                    component,
                    popup,
                    |instance_ref| {
                        let comp = ComponentInstance::InstanceRef(instance_ref);
                        let x = load_property_helper(&comp, &popup.x.element(), popup.x.name())
                            .unwrap();
                        let y = load_property_helper(&comp, &popup.y.element(), popup.y.name())
                            .unwrap();
                        corelib::api::LogicalPosition::new(
                            x.try_into().unwrap(),
                            y.try_into().unwrap(),
                        )
                    },
                    close_policy,
                    enclosing_component.self_weak().get().unwrap().clone(),
                    component.window_adapter(),
                    &parent_item,
                );
                Value::Void
            } else {
                panic!("internal error: argument to ShowPopupWindow must be an element")
            }
        }
        BuiltinFunction::ClosePopupWindow => {
            let component = local_context.component_instance;
            if let Expression::ElementReference(popup_window) = &arguments[0] {
                let popup_window = popup_window.upgrade().unwrap();
                let pop_comp = popup_window.borrow().enclosing_component.upgrade().unwrap();
                let parent_component = pop_comp
                    .parent_element
                    .upgrade()
                    .unwrap()
                    .borrow()
                    .enclosing_component
                    .upgrade()
                    .unwrap();
                let popup_list = parent_component.popup_windows.borrow();
                let popup =
                    popup_list.iter().find(|p| Rc::ptr_eq(&p.component, &pop_comp)).unwrap();

                generativity::make_guard!(guard);
                let enclosing_component =
                    enclosing_component_for_element(&popup.parent_element, component, guard);
                crate::dynamic_item_tree::close_popup(
                    popup_window,
                    enclosing_component,
                    enclosing_component.window_adapter(),
                );

                Value::Void
            } else {
                panic!("internal error: argument to ClosePopupWindow must be an element")
            }
        }
        BuiltinFunction::ShowPopupMenu => {
            let [Expression::ElementReference(element), entries, position] = arguments else {
                panic!("internal error: incorrect argument count to ShowPopupMenu")
            };
            let position = eval_expression(position, local_context)
                .try_into()
                .expect("internal error: popup menu position argument should be a point");

            let component = local_context.component_instance;
            let elem = element.upgrade().unwrap();
            generativity::make_guard!(guard);
            let enclosing_component = enclosing_component_for_element(&elem, component, guard);
            let description = enclosing_component.description;
            let item_info = &description.items[elem.borrow().id.as_str()];
            let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
            let item_tree = vtable::VRc::into_dyn(item_comp);
            let item_rc = corelib::items::ItemRc::new(item_tree.clone(), item_info.item_index());

            let context_menu_item = vtable::VRc::new(MenuFromItemTree::new(item_tree));
            let context_menu_item = vtable::VRc::into_dyn(context_menu_item);
            if component
                .access_window(|window| window.show_native_popup_menu(context_menu_item, position))
            {
                return Value::Void;
            }

            generativity::make_guard!(guard);
            let compiled = enclosing_component.description.popup_menu_description.unerase(guard);
            let inst = crate::dynamic_item_tree::instantiate(
                compiled.clone(),
                Some(enclosing_component.self_weak().get().unwrap().clone()),
                None,
                Some(&crate::dynamic_item_tree::WindowOptions::UseExistingWindow(
                    component.window_adapter(),
                )),
                Default::default(),
            );

            generativity::make_guard!(guard);
            let inst_ref = inst.unerase(guard);
            if let Expression::ElementReference(e) = entries {
                let menu_item_tree =
                    e.upgrade().unwrap().borrow().enclosing_component.upgrade().unwrap();
                let (entries, sub_menu, activated) =
                    menu_item_tree_properties(crate::dynamic_item_tree::make_menu_item_tree(
                        &menu_item_tree,
                        &enclosing_component,
                    ));
                compiled.set_binding(inst_ref.borrow(), "entries", entries).unwrap();
                compiled.set_callback_handler(inst_ref.borrow(), "sub-menu", sub_menu).unwrap();
                compiled.set_callback_handler(inst_ref.borrow(), "activated", activated).unwrap();
            } else {
                let entries = eval_expression(entries, local_context);
                compiled.set_property(inst_ref.borrow(), "entries", entries).unwrap();
                let item_weak = item_rc.downgrade();
                compiled
                    .set_callback_handler(
                        inst_ref.borrow(),
                        "sub-menu",
                        Box::new(move |args: &[Value]| -> Value {
                            item_weak
                                .upgrade()
                                .unwrap()
                                .downcast::<corelib::items::ContextMenu>()
                                .unwrap()
                                .sub_menu
                                .call(&(args[0].clone().try_into().unwrap(),))
                                .into()
                        }),
                    )
                    .unwrap();
                let item_weak = item_rc.downgrade();
                compiled
                    .set_callback_handler(
                        inst_ref.borrow(),
                        "activated",
                        Box::new(move |args: &[Value]| -> Value {
                            item_weak
                                .upgrade()
                                .unwrap()
                                .downcast::<corelib::items::ContextMenu>()
                                .unwrap()
                                .activated
                                .call(&(args[0].clone().try_into().unwrap(),));
                            Value::Void
                        }),
                    )
                    .unwrap();
            }
            let item_weak = item_rc.downgrade();
            compiled
                .set_callback_handler(
                    inst_ref.borrow(),
                    "close",
                    Box::new(move |_args: &[Value]| -> Value {
                        let Some(item_rc) = item_weak.upgrade() else { return Value::Void };
                        if let Some(id) = item_rc
                            .downcast::<corelib::items::ContextMenu>()
                            .unwrap()
                            .popup_id
                            .take()
                        {
                            WindowInner::from_pub(item_rc.window_adapter().unwrap().window())
                                .close_popup(id);
                        }
                        Value::Void
                    }),
                )
                .unwrap();
            component.access_window(|window| {
                let context_menu_elem = item_rc.downcast::<corelib::items::ContextMenu>().unwrap();
                if let Some(old_id) = context_menu_elem.popup_id.take() {
                    window.close_popup(old_id)
                }
                let id = window.show_popup(
                    &vtable::VRc::into_dyn(inst.clone()),
                    position,
                    corelib::items::PopupClosePolicy::CloseOnClickOutside,
                    &item_rc,
                    true,
                );
                context_menu_elem.popup_id.set(Some(id));
            });
            inst.run_setup_code();
            Value::Void
        }
        BuiltinFunction::SetSelectionOffsets => {
            if arguments.len() != 3 {
                panic!("internal error: incorrect argument count to select range function call")
            }
            let component = local_context.component_instance;
            if let Expression::ElementReference(element) = &arguments[0] {
                generativity::make_guard!(guard);

                let elem = element.upgrade().unwrap();
                let enclosing_component = enclosing_component_for_element(&elem, component, guard);
                let description = enclosing_component.description;
                let item_info = &description.items[elem.borrow().id.as_str()];
                let item_ref =
                    unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };

                let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
                let item_rc = corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(item_comp),
                    item_info.item_index(),
                );

                let window_adapter = component.window_adapter();

                // TODO: Make this generic through RTTI
                if let Some(textinput) =
                    ItemRef::downcast_pin::<corelib::items::TextInput>(item_ref)
                {
                    let start: i32 =
                        eval_expression(&arguments[1], local_context).try_into().expect(
                            "internal error: second argument to set-selection-offsets must be an integer",
                        );
                    let end: i32 = eval_expression(&arguments[2], local_context).try_into().expect(
                        "internal error: third argument to set-selection-offsets must be an integer",
                    );

                    textinput.set_selection_offsets(&window_adapter, &item_rc, start, end);
                } else {
                    panic!(
                        "internal error: member function called on element that doesn't have it: {}",
                        elem.borrow().original_name()
                    )
                }

                Value::Void
            } else {
                panic!("internal error: first argument to set-selection-offsets must be an element")
            }
        }
        BuiltinFunction::ItemFontMetrics => {
            if arguments.len() != 1 {
                panic!(
                    "internal error: incorrect argument count to item font metrics function call"
                )
            }
            let component = local_context.component_instance;
            if let Expression::ElementReference(element) = &arguments[0] {
                generativity::make_guard!(guard);

                let elem = element.upgrade().unwrap();
                let enclosing_component = enclosing_component_for_element(&elem, component, guard);
                let description = enclosing_component.description;
                let item_info = &description.items[elem.borrow().id.as_str()];
                let item_ref =
                    unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
                let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
                let item_rc = corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(item_comp),
                    item_info.item_index(),
                );
                let window_adapter = component.window_adapter();
                let metrics = i_slint_core::items::slint_text_item_fontmetrics(
                    &window_adapter,
                    item_ref,
                    &item_rc,
                );
                metrics.into()
            } else {
                panic!("internal error: argument to item-font-metrics must be an element")
            }
        }
        BuiltinFunction::StringIsFloat => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to StringIsFloat")
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::Bool(<f64 as core::str::FromStr>::from_str(s.as_str()).is_ok())
            } else {
                panic!("Argument not a string");
            }
        }
        BuiltinFunction::StringToFloat => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to StringToFloat")
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::Number(core::str::FromStr::from_str(s.as_str()).unwrap_or(0.))
            } else {
                panic!("Argument not a string");
            }
        }
        BuiltinFunction::StringIsEmpty => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to StringIsEmpty")
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::Bool(s.is_empty())
            } else {
                panic!("Argument not a string");
            }
        }
        BuiltinFunction::StringCharacterCount => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to StringCharacterCount")
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::Number(
                    unicode_segmentation::UnicodeSegmentation::graphemes(s.as_str(), true).count()
                        as f64,
                )
            } else {
                panic!("Argument not a string");
            }
        }
        BuiltinFunction::StringToLowercase => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to StringToLowercase")
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::String(s.to_lowercase().into())
            } else {
                panic!("Argument not a string");
            }
        }
        BuiltinFunction::StringToUppercase => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to StringToUppercase")
            }
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                Value::String(s.to_uppercase().into())
            } else {
                panic!("Argument not a string");
            }
        }
        BuiltinFunction::ColorRgbaStruct => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to ColorRGBAComponents")
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
                panic!("First argument not a color");
            }
        }
        BuiltinFunction::ColorHsvaStruct => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to ColorHSVAComponents")
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
                panic!("First argument not a color");
            }
        }
        BuiltinFunction::ColorBrighter => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ColorBrighter")
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.brighter(factor as _).into()
                } else {
                    panic!("Second argument not a number");
                }
            } else {
                panic!("First argument not a color");
            }
        }
        BuiltinFunction::ColorDarker => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ColorDarker")
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.darker(factor as _).into()
                } else {
                    panic!("Second argument not a number");
                }
            } else {
                panic!("First argument not a color");
            }
        }
        BuiltinFunction::ColorTransparentize => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ColorFaded")
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.transparentize(factor as _).into()
                } else {
                    panic!("Second argument not a number");
                }
            } else {
                panic!("First argument not a color");
            }
        }
        BuiltinFunction::ColorMix => {
            if arguments.len() != 3 {
                panic!("internal error: incorrect argument count to ColorMix")
            }

            let arg0 = eval_expression(&arguments[0], local_context);
            let arg1 = eval_expression(&arguments[1], local_context);
            let arg2 = eval_expression(&arguments[2], local_context);

            if !matches!(arg0, Value::Brush(Brush::SolidColor(_))) {
                panic!("First argument not a color");
            }
            if !matches!(arg1, Value::Brush(Brush::SolidColor(_))) {
                panic!("Second argument not a color");
            }
            if !matches!(arg2, Value::Number(_)) {
                panic!("Third argument not a number");
            }

            let (
                Value::Brush(Brush::SolidColor(color_a)),
                Value::Brush(Brush::SolidColor(color_b)),
                Value::Number(factor),
            ) = (arg0, arg1, arg2)
            else {
                unreachable!()
            };

            color_a.mix(&color_b, factor as _).into()
        }
        BuiltinFunction::ColorWithAlpha => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ColorWithAlpha")
            }
            if let Value::Brush(brush) = eval_expression(&arguments[0], local_context) {
                if let Value::Number(factor) = eval_expression(&arguments[1], local_context) {
                    brush.with_alpha(factor as _).into()
                } else {
                    panic!("Second argument not a number");
                }
            } else {
                panic!("First argument not a color");
            }
        }
        BuiltinFunction::ImageSize => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to ImageSize")
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
                panic!("First argument not an image");
            }
        }
        BuiltinFunction::ArrayLength => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to ArrayLength")
            }
            match eval_expression(&arguments[0], local_context) {
                Value::Model(model) => {
                    model.model_tracker().track_row_count_changes();
                    Value::Number(model.row_count() as f64)
                }
                _ => {
                    panic!("First argument not an array");
                }
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
            Value::Brush(Brush::SolidColor(Color::from_argb_u8(a, r, g, b)))
        }
        BuiltinFunction::Hsv => {
            let h: f32 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let s: f32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            let v: f32 = eval_expression(&arguments[2], local_context).try_into().unwrap();
            let a: f32 = eval_expression(&arguments[3], local_context).try_into().unwrap();
            let a = (1. * a).clamp(0., 1.);
            Value::Brush(Brush::SolidColor(Color::from_hsva(h, s, v, a)))
        }
        BuiltinFunction::ColorScheme => local_context
            .component_instance
            .window_adapter()
            .internal(corelib::InternalToken)
            .map_or(ColorScheme::Unknown, |x| x.color_scheme())
            .into(),
        BuiltinFunction::SupportsNativeMenuBar => local_context
            .component_instance
            .window_adapter()
            .internal(corelib::InternalToken)
            .is_some_and(|x| x.supports_native_menu_bar())
            .into(),
        BuiltinFunction::SetupNativeMenuBar => {
            let component = local_context.component_instance;
            if let [Expression::PropertyReference(entries_nr), Expression::PropertyReference(sub_menu_nr), Expression::PropertyReference(activated_nr), Expression::ElementReference(item_tree_root), Expression::BoolLiteral(no_native)] =
                arguments
            {
                let menu_item_tree = item_tree_root
                    .upgrade()
                    .unwrap()
                    .borrow()
                    .enclosing_component
                    .upgrade()
                    .unwrap();
                let menu_item_tree =
                    crate::dynamic_item_tree::make_menu_item_tree(&menu_item_tree, &component);

                if let Some(w) = component.window_adapter().internal(i_slint_core::InternalToken) {
                    if !no_native && w.supports_native_menu_bar() {
                        let menubar = vtable::VRc::new(menu_item_tree);
                        let menubar = vtable::VRc::into_dyn(menubar);
                        w.setup_menubar(menubar);
                        return Value::Void;
                    }
                }

                let (entries, sub_menu, activated) = menu_item_tree_properties(menu_item_tree);

                assert_eq!(
                    entries_nr.element().borrow().id,
                    component.description.original.root_element.borrow().id,
                    "entries need to be in the main element"
                );
                local_context
                    .component_instance
                    .description
                    .set_binding(component.borrow(), entries_nr.name(), entries)
                    .unwrap();
                let i = &ComponentInstance::InstanceRef(local_context.component_instance);
                set_callback_handler(i, &sub_menu_nr.element(), sub_menu_nr.name(), sub_menu)
                    .unwrap();
                set_callback_handler(i, &activated_nr.element(), activated_nr.name(), activated)
                    .unwrap();

                return Value::Void;
            }
            let [entries, Expression::PropertyReference(sub_menu), Expression::PropertyReference(activated)] =
                arguments
            else {
                panic!("internal error: incorrect arguments to SetupNativeMenuBar: {arguments:?}")
            };
            if let Some(w) = component.window_adapter().internal(i_slint_core::InternalToken) {
                if w.supports_native_menu_bar() {
                    let menubar = vtable::VRc::new(MenuWrapper {
                        entries: entries.clone(),
                        sub_menu: sub_menu.clone(),
                        activated: activated.clone(),
                        item_tree: component.self_weak().get().unwrap().clone(),
                    });
                    let menubar = vtable::VRc::into_dyn(menubar);
                    w.setup_menubar(menubar);
                }
            }
            Value::Void
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
            let f: SharedString = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let d: u32 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            let m: u32 = eval_expression(&arguments[2], local_context).try_into().unwrap();
            let y: i32 = eval_expression(&arguments[3], local_context).try_into().unwrap();

            Value::String(i_slint_core::date_time::format_date(&f, d, m, y))
        }
        BuiltinFunction::DateNow => Value::Model(ModelRc::new(VecModel::from(
            i_slint_core::date_time::date_now()
                .into_iter()
                .map(|x| Value::Number(x as f64))
                .collect::<Vec<_>>(),
        ))),
        BuiltinFunction::ValidDate => {
            let d: SharedString = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let f: SharedString = eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Bool(i_slint_core::date_time::parse_date(d.as_str(), f.as_str()).is_some())
        }
        BuiltinFunction::ParseDate => {
            let d: SharedString = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let f: SharedString = eval_expression(&arguments[1], local_context).try_into().unwrap();

            Value::Model(ModelRc::new(
                i_slint_core::date_time::parse_date(d.as_str(), f.as_str())
                    .map(|x| {
                        VecModel::from(
                            x.into_iter().map(|x| Value::Number(x as f64)).collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_default(),
            ))
        }
        BuiltinFunction::TextInputFocused => Value::Bool(
            local_context.component_instance.access_window(|window| window.text_input_focused())
                as _,
        ),
        BuiltinFunction::SetTextInputFocused => {
            local_context.component_instance.access_window(|window| {
                window.set_text_input_focused(
                    eval_expression(&arguments[0], local_context).try_into().unwrap(),
                )
            });
            Value::Void
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            let component = local_context.component_instance;
            if let [Expression::ElementReference(item)] = arguments {
                generativity::make_guard!(guard);

                let item = item.upgrade().unwrap();
                let enclosing_component = enclosing_component_for_element(&item, component, guard);
                let description = enclosing_component.description;
                let item_info = &description.items[item.borrow().id.as_str()];
                let item_ref =
                    unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
                let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
                let window_adapter = component.window_adapter();
                item_ref
                    .as_ref()
                    .layout_info(
                        crate::eval_layout::to_runtime(orient),
                        &window_adapter,
                        &ItemRc::new(vtable::VRc::into_dyn(item_comp), item_info.item_index()),
                    )
                    .into()
            } else {
                panic!("internal error: incorrect arguments to ImplicitLayoutInfo {arguments:?}");
            }
        }
        BuiltinFunction::ItemAbsolutePosition => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to ItemAbsolutePosition")
            }

            let component = local_context.component_instance;

            if let Expression::ElementReference(item) = &arguments[0] {
                generativity::make_guard!(guard);

                let item = item.upgrade().unwrap();
                let enclosing_component = enclosing_component_for_element(&item, component, guard);
                let description = enclosing_component.description;

                let item_info = &description.items[item.borrow().id.as_str()];

                let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();

                let item_rc = corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(item_comp),
                    item_info.item_index(),
                );

                item_rc.map_to_window(Default::default()).to_untyped().into()
            } else {
                panic!("internal error: argument to SetFocusItem must be an element")
            }
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to RegisterCustomFontByPath")
            }
            let component = local_context.component_instance;
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                if let Some(err) = component
                    .window_adapter()
                    .renderer()
                    .register_font_from_path(&std::path::PathBuf::from(s.as_str()))
                    .err()
                {
                    corelib::debug_log!("Error loading custom font {}: {}", s.as_str(), err);
                }
                Value::Void
            } else {
                panic!("Argument not a string");
            }
        }
        BuiltinFunction::RegisterCustomFontByMemory | BuiltinFunction::RegisterBitmapFont => {
            unimplemented!()
        }
        BuiltinFunction::Translate => {
            let original: SharedString =
                eval_expression(&arguments[0], local_context).try_into().unwrap();
            let context: SharedString =
                eval_expression(&arguments[1], local_context).try_into().unwrap();
            let domain: SharedString =
                eval_expression(&arguments[2], local_context).try_into().unwrap();
            let args = eval_expression(&arguments[3], local_context);
            let Value::Model(args) = args else { panic!("Args to translate not a model {args:?}") };
            struct StringModelWrapper(ModelRc<Value>);
            impl corelib::translations::FormatArgs for StringModelWrapper {
                type Output<'a> = SharedString;
                fn from_index(&self, index: usize) -> Option<SharedString> {
                    self.0.row_data(index).map(|x| x.try_into().unwrap())
                }
            }
            Value::String(corelib::translations::translate(
                &original,
                &context,
                &domain,
                &StringModelWrapper(args),
                eval_expression(&arguments[4], local_context).try_into().unwrap(),
                &SharedString::try_from(eval_expression(&arguments[5], local_context)).unwrap(),
            ))
        }
        BuiltinFunction::Use24HourFormat => Value::Bool(corelib::date_time::use_24_hour_format()),
        BuiltinFunction::UpdateTimers => {
            crate::dynamic_item_tree::update_timers(local_context.component_instance);
            Value::Void
        }
        BuiltinFunction::DetectOperatingSystem => i_slint_core::detect_operating_system().into(),
        // start and stop are unreachable because they are lowered to simple assignment of running
        BuiltinFunction::StartTimer => unreachable!(),
        BuiltinFunction::StopTimer => unreachable!(),
        BuiltinFunction::RestartTimer => {
            if let [Expression::ElementReference(timer_element)] = arguments {
                crate::dynamic_item_tree::restart_timer(
                    timer_element.clone(),
                    local_context.component_instance,
                );

                Value::Void
            } else {
                panic!("internal error: argument to RestartTimer must be an element")
            }
        }
    }
}

fn call_item_member_function(nr: &NamedReference, local_context: &mut EvalLocalContext) -> Value {
    let component = local_context.component_instance;
    let elem = nr.element();
    let name = nr.name().as_str();
    generativity::make_guard!(guard);
    let enclosing_component = enclosing_component_for_element(&elem, component, guard);
    let description = enclosing_component.description;
    let item_info = &description.items[elem.borrow().id.as_str()];
    let item_ref = unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };

    let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
    let item_rc =
        corelib::items::ItemRc::new(vtable::VRc::into_dyn(item_comp), item_info.item_index());

    let window_adapter = component.window_adapter();

    // TODO: Make this generic through RTTI
    if let Some(textinput) = ItemRef::downcast_pin::<corelib::items::TextInput>(item_ref) {
        match name {
            "select-all" => textinput.select_all(&window_adapter, &item_rc),
            "clear-selection" => textinput.clear_selection(&window_adapter, &item_rc),
            "cut" => textinput.cut(&window_adapter, &item_rc),
            "copy" => textinput.copy(&window_adapter, &item_rc),
            "paste" => textinput.paste(&window_adapter, &item_rc),
            _ => panic!("internal: Unknown member function {name} called on TextInput"),
        }
    } else if let Some(s) = ItemRef::downcast_pin::<corelib::items::SwipeGestureHandler>(item_ref) {
        match name {
            "cancel" => s.cancel(&window_adapter, &item_rc),
            _ => panic!("internal: Unknown member function {name} called on SwipeGestureHandler"),
        }
    } else if let Some(s) = ItemRef::downcast_pin::<corelib::items::ContextMenu>(item_ref) {
        match name {
            "close" => s.close(&window_adapter, &item_rc),
            "is-open" => return Value::Bool(s.is_open(&window_adapter, &item_rc)),
            _ => {
                panic!("internal: Unknown member function {name} called on ContextMenu")
            }
        }
    } else {
        panic!(
            "internal error: member function {name} called on element that doesn't have it: {}",
            elem.borrow().original_name()
        )
    }

    Value::Void
}

fn eval_assignment(lhs: &Expression, op: char, rhs: Value, local_context: &mut EvalLocalContext) {
    let eval = |lhs| match (lhs, &rhs, op) {
        (Value::String(ref mut a), Value::String(b), '+') => {
            a.push_str(b.as_str());
            Value::String(a.clone())
        }
        (Value::Number(a), Value::Number(b), '+') => Value::Number(a + b),
        (Value::Number(a), Value::Number(b), '-') => Value::Number(a - b),
        (Value::Number(a), Value::Number(b), '/') => Value::Number(a / b),
        (Value::Number(a), Value::Number(b), '*') => Value::Number(a * b),
        (lhs, rhs, op) => panic!("unsupported {lhs:?} {op} {rhs:?}"),
    };
    match lhs {
        Expression::PropertyReference(nr) => {
            let element = nr.element();
            generativity::make_guard!(guard);
            let enclosing_component = enclosing_component_instance_for_element(
                &element,
                &ComponentInstance::InstanceRef(local_context.component_instance),
                guard,
            );

            match enclosing_component {
                ComponentInstance::InstanceRef(enclosing_component) => {
                    if op == '=' {
                        store_property(enclosing_component, &element, nr.name(), rhs).unwrap();
                        return;
                    }

                    let component = element.borrow().enclosing_component.upgrade().unwrap();
                    if element.borrow().id == component.root_element.borrow().id {
                        if let Some(x) =
                            enclosing_component.description.custom_properties.get(nr.name())
                        {
                            unsafe {
                                let p = Pin::new_unchecked(
                                    &*enclosing_component.as_ptr().add(x.offset),
                                );
                                x.prop.set(p, eval(x.prop.get(p).unwrap()), None).unwrap();
                            }
                            return;
                        }
                    };
                    let item_info =
                        &enclosing_component.description.items[element.borrow().id.as_str()];
                    let item =
                        unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
                    let p = &item_info.rtti.properties[nr.name().as_str()];
                    p.set(item, eval(p.get(item)), None).unwrap();
                }
                ComponentInstance::GlobalComponent(global) => {
                    let val = if op == '=' {
                        rhs
                    } else {
                        eval(global.as_ref().get_property(nr.name()).unwrap())
                    };
                    global.as_ref().set_property(nr.name(), val).unwrap();
                }
            }
        }
        Expression::StructFieldAccess { base, name } => {
            if let Value::Struct(mut o) = eval_expression(base, local_context) {
                let mut r = o.get_field(name).unwrap().clone();
                r = if op == '=' { rhs } else { eval(std::mem::take(&mut r)) };
                o.set_field(name.to_string(), r);
                eval_assignment(base, '=', Value::Struct(o), local_context)
            }
        }
        Expression::RepeaterModelReference { element } => {
            let element = element.upgrade().unwrap();
            let component_instance = local_context.component_instance;
            generativity::make_guard!(g1);
            let enclosing_component =
                enclosing_component_for_element(&element, component_instance, g1);
            // we need a 'static Repeater component in order to call model_set_row_data, so get it.
            // Safety: This is the only 'static Id in scope.
            let static_guard =
                unsafe { generativity::Guard::new(generativity::Id::<'static>::new()) };
            let repeater = crate::dynamic_item_tree::get_repeater_by_name(
                enclosing_component,
                element.borrow().id.as_str(),
                static_guard,
            );
            repeater.0.model_set_row_data(
                eval_expression(
                    &Expression::RepeaterIndexReference { element: Rc::downgrade(&element) },
                    local_context,
                )
                .try_into()
                .unwrap(),
                if op == '=' {
                    rhs
                } else {
                    eval(eval_expression(
                        &Expression::RepeaterModelReference { element: Rc::downgrade(&element) },
                        local_context,
                    ))
                },
            )
        }
        Expression::ArrayIndex { array, index } => {
            let array = eval_expression(array, local_context);
            let index = eval_expression(index, local_context);
            match (array, index) {
                (Value::Model(model), Value::Number(index)) => {
                    if index >= 0. && (index as usize) < model.row_count() {
                        let index = index as usize;
                        if op == '=' {
                            model.set_row_data(index, rhs);
                        } else {
                            model.set_row_data(
                                index,
                                eval(
                                    model
                                        .row_data(index)
                                        .unwrap_or_else(|| default_value_for_type(&lhs.ty())),
                                ),
                            );
                        }
                    }
                }
                _ => {
                    eprintln!("Attempting to write into an array that cannot be written");
                }
            }
        }
        _ => panic!("typechecking should make sure this was a PropertyReference"),
    }
}

pub fn load_property(component: InstanceRef, element: &ElementRc, name: &str) -> Result<Value, ()> {
    load_property_helper(&ComponentInstance::InstanceRef(component), element, name)
}

fn load_property_helper(
    component_instance: &ComponentInstance,
    element: &ElementRc,
    name: &str,
) -> Result<Value, ()> {
    generativity::make_guard!(guard);
    match enclosing_component_instance_for_element(element, component_instance, guard) {
        ComponentInstance::InstanceRef(enclosing_component) => {
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(x) = enclosing_component.description.custom_properties.get(name) {
                    return unsafe {
                        x.prop.get(Pin::new_unchecked(&*enclosing_component.as_ptr().add(x.offset)))
                    };
                } else if enclosing_component.description.original.is_global() {
                    return Err(());
                }
            };
            let item_info = enclosing_component
                .description
                .items
                .get(element.id.as_str())
                .unwrap_or_else(|| panic!("Unknown element for {}.{}", element.id, name));
            core::mem::drop(element);
            let item = unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
            Ok(item_info.rtti.properties.get(name).ok_or(())?.get(item))
        }
        ComponentInstance::GlobalComponent(glob) => glob.as_ref().get_property(name),
    }
}

pub fn store_property(
    component_instance: InstanceRef,
    element: &ElementRc,
    name: &str,
    value: Value,
) -> Result<(), SetPropertyError> {
    generativity::make_guard!(guard);
    match enclosing_component_instance_for_element(
        element,
        &ComponentInstance::InstanceRef(component_instance),
        guard,
    ) {
        ComponentInstance::InstanceRef(enclosing_component) => {
            let maybe_animation = match element.borrow().bindings.get(name) {
                Some(b) => crate::dynamic_item_tree::animation_for_property(
                    enclosing_component,
                    &b.borrow().animation,
                ),
                None => {
                    crate::dynamic_item_tree::animation_for_property(enclosing_component, &None)
                }
            };

            let component = element.borrow().enclosing_component.upgrade().unwrap();
            if element.borrow().id == component.root_element.borrow().id {
                if let Some(x) = enclosing_component.description.custom_properties.get(name) {
                    if let Some(orig_decl) = enclosing_component
                        .description
                        .original
                        .root_element
                        .borrow()
                        .property_declarations
                        .get(name)
                    {
                        // Do an extra type checking because PropertyInfo::set won't do it for custom structures or array
                        if !check_value_type(&value, &orig_decl.property_type) {
                            return Err(SetPropertyError::WrongType);
                        }
                    }
                    unsafe {
                        let p = Pin::new_unchecked(&*enclosing_component.as_ptr().add(x.offset));
                        return x
                            .prop
                            .set(p, value, maybe_animation.as_animation())
                            .map_err(|()| SetPropertyError::WrongType);
                    }
                } else if enclosing_component.description.original.is_global() {
                    return Err(SetPropertyError::NoSuchProperty);
                }
            };
            let item_info = &enclosing_component.description.items[element.borrow().id.as_str()];
            let item = unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
            let p = &item_info.rtti.properties.get(name).ok_or(SetPropertyError::NoSuchProperty)?;
            p.set(item, value, maybe_animation.as_animation())
                .map_err(|()| SetPropertyError::WrongType)?;
        }
        ComponentInstance::GlobalComponent(glob) => {
            glob.as_ref().set_property(name, value)?;
        }
    }
    Ok(())
}

/// Return true if the Value can be used for a property of the given type
fn check_value_type(value: &Value, ty: &Type) -> bool {
    match ty {
        Type::Void => true,
        Type::Invalid
        | Type::InferredProperty
        | Type::InferredCallback
        | Type::Callback { .. }
        | Type::Function { .. }
        | Type::ElementReference => panic!("not valid property type"),
        Type::Float32 => matches!(value, Value::Number(_)),
        Type::Int32 => matches!(value, Value::Number(_)),
        Type::String => matches!(value, Value::String(_)),
        Type::Color => matches!(value, Value::Brush(_)),
        Type::UnitProduct(_)
        | Type::Duration
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Angle
        | Type::Percent => matches!(value, Value::Number(_)),
        Type::Image => matches!(value, Value::Image(_)),
        Type::Bool => matches!(value, Value::Bool(_)),
        Type::Model => {
            matches!(value, Value::Model(_) | Value::Bool(_) | Value::Number(_))
        }
        Type::PathData => matches!(value, Value::PathData(_)),
        Type::Easing => matches!(value, Value::EasingCurve(_)),
        Type::Brush => matches!(value, Value::Brush(_)),
        Type::Array(inner) => {
            matches!(value, Value::Model(m) if m.iter().all(|v| check_value_type(&v, inner)))
        }
        Type::Struct(s) => {
            matches!(value, Value::Struct(str) if str.iter().all(|(k, v)| s.fields.get(k).is_some_and(|ty| check_value_type(v, ty))))
        }
        Type::Enumeration(en) => {
            matches!(value, Value::EnumerationValue(name, _) if name == en.name.as_str())
        }
        Type::LayoutCache => matches!(value, Value::LayoutCache(_)),
        Type::ComponentFactory => matches!(value, Value::ComponentFactory(_)),
    }
}

pub(crate) fn invoke_callback(
    component_instance: &ComponentInstance,
    element: &ElementRc,
    callback_name: &SmolStr,
    args: &[Value],
) -> Option<Value> {
    generativity::make_guard!(guard);
    match enclosing_component_instance_for_element(element, component_instance, guard) {
        ComponentInstance::InstanceRef(enclosing_component) => {
            let description = enclosing_component.description;
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(callback_offset) = description.custom_callbacks.get(callback_name) {
                    let callback = callback_offset.apply(&*enclosing_component.instance);
                    let res = callback.call(args);
                    return Some(if res != Value::Void {
                        res
                    } else if let Some(Type::Callback(callback)) = description
                        .original
                        .root_element
                        .borrow()
                        .property_declarations
                        .get(callback_name)
                        .map(|d| &d.property_type)
                    {
                        // If the callback was not set, the return value will be Value::Void, but we need
                        // to make sure that the value is actually of the right type as returned by the
                        // callback, otherwise we will get panics later
                        default_value_for_type(&callback.return_type)
                    } else {
                        res
                    });
                } else if enclosing_component.description.original.is_global() {
                    return None;
                }
            };
            let item_info = &description.items[element.id.as_str()];
            let item = unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
            item_info
                .rtti
                .callbacks
                .get(callback_name.as_str())
                .map(|callback| callback.call(item, args))
        }
        ComponentInstance::GlobalComponent(global) => {
            Some(global.as_ref().invoke_callback(callback_name, args).unwrap())
        }
    }
}

pub(crate) fn set_callback_handler(
    component_instance: &ComponentInstance,
    element: &ElementRc,
    callback_name: &str,
    handler: CallbackHandler,
) -> Result<(), ()> {
    generativity::make_guard!(guard);
    match enclosing_component_instance_for_element(element, component_instance, guard) {
        ComponentInstance::InstanceRef(enclosing_component) => {
            let description = enclosing_component.description;
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(callback_offset) = description.custom_callbacks.get(callback_name) {
                    let callback = callback_offset.apply(&*enclosing_component.instance);
                    callback.set_handler(handler);
                    return Ok(());
                } else if enclosing_component.description.original.is_global() {
                    return Err(());
                }
            };
            let item_info = &description.items[element.id.as_str()];
            let item = unsafe { item_info.item_from_item_tree(enclosing_component.as_ptr()) };
            if let Some(callback) = item_info.rtti.callbacks.get(callback_name) {
                callback.set_handler(item, handler);
                Ok(())
            } else {
                Err(())
            }
        }
        ComponentInstance::GlobalComponent(global) => {
            global.as_ref().set_callback_handler(callback_name, handler)
        }
    }
}

/// Invoke the function.
///
/// Return None if the function don't exist
pub(crate) fn call_function(
    component_instance: &ComponentInstance,
    element: &ElementRc,
    function_name: &str,
    args: Vec<Value>,
) -> Option<Value> {
    generativity::make_guard!(guard);
    match enclosing_component_instance_for_element(element, component_instance, guard) {
        ComponentInstance::InstanceRef(c) => {
            let mut ctx = EvalLocalContext::from_function_arguments(c, args);
            eval_expression(
                &element.borrow().bindings.get(function_name)?.borrow().expression,
                &mut ctx,
            )
            .into()
        }
        ComponentInstance::GlobalComponent(g) => g.as_ref().eval_function(function_name, args).ok(),
    }
}

/// Return the component instance which hold the given element.
/// Does not take in account the global component.
pub fn enclosing_component_for_element<'a, 'old_id, 'new_id>(
    element: &'a ElementRc,
    component: InstanceRef<'a, 'old_id>,
    _guard: generativity::Guard<'new_id>,
) -> InstanceRef<'a, 'new_id> {
    let enclosing = &element.borrow().enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(enclosing, &component.description.original) {
        // Safety: new_id is an unique id
        unsafe {
            std::mem::transmute::<InstanceRef<'a, 'old_id>, InstanceRef<'a, 'new_id>>(component)
        }
    } else {
        assert!(!enclosing.is_global());
        // Safety: this is the only place we use this 'static lifetime in this function and nothing is returned with it
        // For some reason we can't make a new guard here because the compiler thinks we are returning that
        // (it assumes that the 'id must outlive 'a , which is not true)
        let static_guard = unsafe { generativity::Guard::new(generativity::Id::<'static>::new()) };

        let parent_instance = component.parent_instance(static_guard).unwrap();
        enclosing_component_for_element(element, parent_instance, _guard)
    }
}

/// Return the component instance which hold the given element.
/// The difference with enclosing_component_for_element is that it takes the GlobalComponent into account.
pub(crate) fn enclosing_component_instance_for_element<'a, 'new_id>(
    element: &'a ElementRc,
    component_instance: &ComponentInstance<'a, '_>,
    guard: generativity::Guard<'new_id>,
) -> ComponentInstance<'a, 'new_id> {
    let enclosing = &element.borrow().enclosing_component.upgrade().unwrap();
    match component_instance {
        ComponentInstance::InstanceRef(component) => {
            if enclosing.is_global() && !Rc::ptr_eq(enclosing, &component.description.original) {
                let root = component.toplevel_instance(guard);
                ComponentInstance::GlobalComponent(
                    root.description
                        .extra_data_offset
                        .apply(root.instance.get_ref())
                        .globals
                        .get()
                        .unwrap()
                        .get(enclosing.root_element.borrow().id.as_str())
                        .unwrap(),
                )
            } else {
                ComponentInstance::InstanceRef(enclosing_component_for_element(
                    element, *component, guard,
                ))
            }
        }
        ComponentInstance::GlobalComponent(global) => {
            //assert!(Rc::ptr_eq(enclosing, &global.component));
            ComponentInstance::GlobalComponent(global.clone())
        }
    }
}

pub fn new_struct_with_bindings<ElementType: 'static + Default + corelib::rtti::BuiltinItem>(
    bindings: &i_slint_compiler::object_tree::BindingsMap,
    local_context: &mut EvalLocalContext,
) -> ElementType {
    let mut element = ElementType::default();
    for (prop, info) in ElementType::fields::<Value>().into_iter() {
        if let Some(binding) = &bindings.get(prop) {
            let value = eval_expression(&binding.borrow(), local_context);
            info.set_field(&mut element, value).unwrap();
        }
    }
    element
}

fn convert_from_lyon_path<'a>(
    events_it: impl IntoIterator<Item = &'a i_slint_compiler::expression_tree::Expression>,
    points_it: impl IntoIterator<Item = &'a i_slint_compiler::expression_tree::Expression>,
    local_context: &mut EvalLocalContext,
) -> PathData {
    let events = events_it
        .into_iter()
        .map(|event_expr| eval_expression(event_expr, local_context).try_into().unwrap())
        .collect::<SharedVector<_>>();

    let points = points_it
        .into_iter()
        .map(|point_expr| {
            let point_value = eval_expression(point_expr, local_context);
            let point_struct: Struct = point_value.try_into().unwrap();
            let mut point = i_slint_core::graphics::Point::default();
            let x: f64 = point_struct.get_field("x").unwrap().clone().try_into().unwrap();
            let y: f64 = point_struct.get_field("y").unwrap().clone().try_into().unwrap();
            point.x = x as _;
            point.y = y as _;
            point
        })
        .collect::<SharedVector<_>>();

    PathData::Events(events, points)
}

pub fn convert_path(path: &ExprPath, local_context: &mut EvalLocalContext) -> PathData {
    match path {
        ExprPath::Elements(elements) => PathData::Elements(
            elements
                .iter()
                .map(|element| convert_path_element(element, local_context))
                .collect::<SharedVector<PathElement>>(),
        ),
        ExprPath::Events(events, points) => {
            convert_from_lyon_path(events.iter(), points.iter(), local_context)
        }
        ExprPath::Commands(commands) => {
            if let Value::String(commands) = eval_expression(commands, local_context) {
                PathData::Commands(commands)
            } else {
                panic!("binding to path commands does not evaluate to string");
            }
        }
    }
}

fn convert_path_element(
    expr_element: &ExprPathElement,
    local_context: &mut EvalLocalContext,
) -> PathElement {
    match expr_element.element_type.native_class.class_name.as_str() {
        "MoveTo" => {
            PathElement::MoveTo(new_struct_with_bindings(&expr_element.bindings, local_context))
        }
        "LineTo" => {
            PathElement::LineTo(new_struct_with_bindings(&expr_element.bindings, local_context))
        }
        "ArcTo" => {
            PathElement::ArcTo(new_struct_with_bindings(&expr_element.bindings, local_context))
        }
        "CubicTo" => {
            PathElement::CubicTo(new_struct_with_bindings(&expr_element.bindings, local_context))
        }
        "QuadraticTo" => PathElement::QuadraticTo(new_struct_with_bindings(
            &expr_element.bindings,
            local_context,
        )),
        "Close" => PathElement::Close,
        _ => panic!(
            "Cannot create unsupported path element {}",
            expr_element.element_type.native_class.class_name
        ),
    }
}

/// Create a value suitable as the default value of a given type
pub fn default_value_for_type(ty: &Type) -> Value {
    match ty {
        Type::Float32 | Type::Int32 => Value::Number(0.),
        Type::String => Value::String(Default::default()),
        Type::Color | Type::Brush => Value::Brush(Default::default()),
        Type::Duration | Type::Angle | Type::PhysicalLength | Type::LogicalLength | Type::Rem => {
            Value::Number(0.)
        }
        Type::Image => Value::Image(Default::default()),
        Type::Bool => Value::Bool(false),
        Type::Callback { .. } => Value::Void,
        Type::Struct(s) => Value::Struct(
            s.fields
                .iter()
                .map(|(n, t)| (n.to_string(), default_value_for_type(t)))
                .collect::<Struct>(),
        ),
        Type::Array(_) | Type::Model => Value::Model(Default::default()),
        Type::Percent => Value::Number(0.),
        Type::Enumeration(e) => Value::EnumerationValue(
            e.name.to_string(),
            e.values.get(e.default_value).unwrap().to_string(),
        ),
        Type::Easing => Value::EasingCurve(Default::default()),
        Type::Void | Type::Invalid => Value::Void,
        Type::UnitProduct(_) => Value::Number(0.),
        Type::PathData => Value::PathData(Default::default()),
        Type::LayoutCache => Value::LayoutCache(Default::default()),
        Type::ComponentFactory => Value::ComponentFactory(Default::default()),
        Type::InferredProperty
        | Type::InferredCallback
        | Type::ElementReference
        | Type::Function { .. } => {
            panic!("There can't be such property")
        }
    }
}

pub struct MenuWrapper {
    entries: Expression,
    sub_menu: NamedReference,
    activated: NamedReference,
    item_tree: crate::dynamic_item_tree::ErasedItemTreeBoxWeak,
}
i_slint_core::MenuVTable_static!(static MENU_WRAPPER_VTABLE for MenuWrapper);
impl Menu for MenuWrapper {
    fn sub_menu(&self, parent: Option<&MenuEntry>, result: &mut SharedVector<MenuEntry>) {
        let Some(s) = self.item_tree.upgrade() else { return };
        generativity::make_guard!(guard);
        let compo_box = s.unerase(guard);
        let instance_ref = compo_box.borrow_instance();
        let res = match parent {
            None => eval_expression(
                &self.entries,
                &mut EvalLocalContext::from_component_instance(instance_ref),
            ),
            Some(parent) => {
                let instance_ref = ComponentInstance::InstanceRef(instance_ref);
                invoke_callback(
                    &instance_ref,
                    &self.sub_menu.element(),
                    self.sub_menu.name(),
                    &[parent.clone().into()],
                )
                .unwrap()
            }
        };
        let Value::Model(model) = res else { panic!("Not a model of menu entries {res:?}") };
        *result = model.iter().map(|v| v.try_into().unwrap()).collect();
    }
    fn activate(&self, entry: &MenuEntry) {
        let Some(s) = self.item_tree.upgrade() else { return };
        generativity::make_guard!(guard);
        let compo_box = s.unerase(guard);
        let instance_ref = compo_box.borrow_instance();
        let instance_ref = ComponentInstance::InstanceRef(instance_ref);
        invoke_callback(
            &instance_ref,
            &self.activated.element(),
            self.activated.name(),
            &[entry.clone().into()],
        )
        .unwrap();
    }
}

fn menu_item_tree_properties(
    menu: MenuFromItemTree,
) -> (Box<dyn Fn() -> Value>, CallbackHandler, CallbackHandler) {
    let context_menu_item_tree = Rc::new(menu);
    let context_menu_item_tree_ = context_menu_item_tree.clone();
    let entries = Box::new(move || {
        let mut entries = SharedVector::default();
        context_menu_item_tree_.sub_menu(None, &mut entries);
        Value::Model(ModelRc::new(VecModel::from(
            entries.into_iter().map(Value::from).collect::<Vec<_>>(),
        )))
    });
    let context_menu_item_tree_ = context_menu_item_tree.clone();
    let sub_menu = Box::new(move |args: &[Value]| -> Value {
        let mut entries = SharedVector::default();
        context_menu_item_tree_.sub_menu(Some(&args[0].clone().try_into().unwrap()), &mut entries);
        Value::Model(ModelRc::new(VecModel::from(
            entries.into_iter().map(Value::from).collect::<Vec<_>>(),
        )))
    });
    let activated = Box::new(move |args: &[Value]| -> Value {
        context_menu_item_tree.activate(&args[0].clone().try_into().unwrap());
        Value::Void
    });
    (entries, sub_menu, activated)
}
