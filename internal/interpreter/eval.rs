// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::api::{SetPropertyError, Struct, Value};
use crate::dynamic_component::InstanceRef;
use core::convert::TryInto;
use core::pin::Pin;
use corelib::graphics::{GradientStop, LinearGradientBrush, PathElement, RadialGradientBrush};
use corelib::items::{ItemRef, PropertyAnimation};
use corelib::model::{Model, ModelRc};
use corelib::rtti::AnimatedBindingKind;
use corelib::window::WindowInner;
use corelib::{Brush, Color, PathData, SharedString, SharedVector};
use i_slint_compiler::expression_tree::{
    BuiltinFunction, EasingCurve, Expression, Path as ExprPath, PathElement as ExprPathElement,
};
use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core as corelib;
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

#[derive(Copy, Clone)]
pub(crate) enum ComponentInstance<'a, 'id> {
    InstanceRef(InstanceRef<'a, 'id>),
    GlobalComponent(&'a Pin<Rc<dyn crate::global_component::GlobalComponent>>),
}

/// The local variable needed for binding evaluation
pub struct EvalLocalContext<'a, 'id> {
    local_variables: HashMap<String, Value>,
    function_arguments: Vec<Value>,
    pub(crate) component_instance: ComponentInstance<'a, 'id>,
    /// When Some, a return statement was executed and one must stop evaluating
    return_value: Option<Value>,
}

impl<'a, 'id> EvalLocalContext<'a, 'id> {
    pub fn from_component_instance(component: InstanceRef<'a, 'id>) -> Self {
        Self {
            local_variables: Default::default(),
            function_arguments: Default::default(),
            component_instance: ComponentInstance::InstanceRef(component),
            return_value: None,
        }
    }

    /// Create a context for a function and passing the arguments
    pub fn from_function_arguments(
        component: InstanceRef<'a, 'id>,
        function_arguments: Vec<Value>,
    ) -> Self {
        Self {
            component_instance: ComponentInstance::InstanceRef(component),
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
        Expression::StringLiteral(s) => Value::String(s.into()),
        Expression::NumberLiteral(n, unit) => Value::Number(unit.normalize(*n)),
        Expression::BoolLiteral(b) => Value::Bool(*b),
        Expression::CallbackReference { .. } => panic!("callback in expression"),
        Expression::FunctionReference { .. } => panic!("function in expression"),
        Expression::BuiltinFunctionReference(..) => panic!(
            "naked builtin function reference not allowed, should be handled by function call"
        ),
        Expression::ElementReference(_) => todo!("Element references are only supported in the context of built-in function calls at the moment"),
        Expression::MemberFunction { .. } => panic!("member function expressions must not appear in the code generator anymore"),
        Expression::BuiltinMacroReference { .. } => panic!("macro expressions must not appear in the code generator anymore"),
        Expression::PropertyReference(nr) => {
            load_property_helper(local_context.component_instance, &nr.element(), nr.name()).unwrap()
        }
        Expression::RepeaterIndexReference { element } => load_property_helper(local_context.component_instance,
            &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
            "index",
        )
        .unwrap(),
        Expression::RepeaterModelReference { element } => load_property_helper(local_context.component_instance,
            &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
            "model_data",
        )
        .unwrap(),
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
                    if (index as usize) < model.row_count() {
                        model.model_tracker().track_row_data_changes(index as usize);
                        model.row_data(index as usize).unwrap_or_else(|| default_value_for_type(&expression.ty()))
                    } else {
                        default_value_for_type(&expression.ty())
                    }
                }
                _ => {
                    Value::Void
                }
            }
        }
        Expression::Cast { from, to } => {
            let v = eval_expression(&*from, local_context);
            match (v, to) {
                (Value::Number(n), Type::Int32) => Value::Number(n.round()),
                (Value::Number(n), Type::String) => {
                    Value::String(i_slint_core::format!("{}", n))
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
        Expression::FunctionCall { function, arguments, source_location: _ } => match &**function {
            Expression::FunctionReference(nr, _) => {
                let args = arguments.iter().map(|e| eval_expression(e, local_context)).collect::<Vec<_>>();
                call_function(local_context.component_instance, &nr.element(), nr.name(), args).unwrap()
            }
            Expression::CallbackReference(nr, _) => {
                let args = arguments.iter().map(|e| eval_expression(e, local_context)).collect::<Vec<_>>();
                invoke_callback(local_context.component_instance, &nr.element(), nr.name(), &args).unwrap()
            }
            Expression::BuiltinFunctionReference(f, _) => call_builtin_function(f.clone(), arguments, local_context),
            _ => panic!("call of something not a callback: {function:?}"),
        }
        Expression::SelfAssignment { lhs, rhs, op, .. } => {
            let rhs = eval_expression(&**rhs, local_context);
            eval_assignment(lhs, *op, rhs, local_context);
            Value::Void
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = eval_expression(&**lhs, local_context);
            let rhs = eval_expression(&**rhs, local_context);

            match (op, lhs, rhs) {
                ('+', Value::String(mut a), Value::String(b)) => { a.push_str(b.as_str()); Value::String(a) },
                ('+', Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                ('+', a @ Value::Struct(_), b @ Value::Struct(_)) => {
                    let a : Option<corelib::layout::LayoutInfo> = a.try_into().ok();
                    let b : Option<corelib::layout::LayoutInfo> = b.try_into().ok();
                    if let (Some(a), Some(b)) = (a, b) {
                        a.merge(&b).into()
                    } else {
                        panic!("unsupported {:?} {} {:?}", a, op, b);
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
                (op, lhs, rhs) => panic!("unsupported {:?} {} {:?}", lhs, op, rhs),
            }
        }
        Expression::UnaryOp { sub, op } => {
            let sub = eval_expression(&**sub, local_context);
            match (sub, op) {
                (Value::Number(a), '+') => Value::Number(a),
                (Value::Number(a), '-') => Value::Number(-a),
                (Value::Bool(a), '!') => Value::Bool(!a),
                (sub, op) => panic!("unsupported {} {:?}", op, sub),
            }
        }
        Expression::ImageReference{ resource_ref, .. } => {
            Value::Image(match resource_ref {
                i_slint_compiler::expression_tree::ImageReference::None => {
                    Ok(Default::default())
                }
                i_slint_compiler::expression_tree::ImageReference::AbsolutePath(path) => {
                    corelib::graphics::Image::load_from_path(std::path::Path::new(path))
                }
                i_slint_compiler::expression_tree::ImageReference::EmbeddedData { resource_id, extension } => {
                    let toplevel_instance = match local_context.component_instance {
                        ComponentInstance::InstanceRef(instance) => instance.toplevel_instance(),
                        ComponentInstance::GlobalComponent(_) => unimplemented!(),
                    };
                    let extra_data = toplevel_instance.component_type.extra_data_offset.apply(toplevel_instance.as_ref());
                    let path = extra_data.embedded_file_resources.get(resource_id).expect("internal error: invalid resource id");

                    let virtual_file = i_slint_compiler::fileaccess::load_file(std::path::Path::new(path)).unwrap();  // embedding pass ensured that the file exists

                    if let (static_path, Some(static_data)) = (virtual_file.canon_path, virtual_file.builtin_contents) {
                        let virtual_file_extension = static_path.extension().unwrap().to_str().unwrap();
                        debug_assert_eq!(virtual_file_extension, extension);
                        Ok(corelib::graphics::load_image_from_embedded_data(
                            corelib::slice::Slice::from_slice(static_data),
                            corelib::slice::Slice::from_slice(virtual_file_extension.as_bytes())
                        ))
                    } else {
                        corelib::debug_log!("Cannot embed images from disk {}", path);
                        Ok(corelib::graphics::Image::default())

                    }
                }
                i_slint_compiler::expression_tree::ImageReference::EmbeddedTexture { .. } => {
                    todo!()
                }
            }.unwrap_or_else(|_| {
                eprintln!("Could not load image {:?}",resource_ref );
                Default::default()
            }))
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            match eval_expression(&**condition, local_context).try_into()
                as Result<bool, _>
            {
                Ok(true) => eval_expression(&**true_expr, local_context),
                Ok(false) => eval_expression(&**false_expr, local_context),
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
                .map(|(k, v)| (k.clone(), eval_expression(v, local_context)))
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
        Expression::EnumerationValue(value) => {
            Value::EnumerationValue(value.enumeration.name.clone(), value.to_string())
        }
        Expression::ReturnStatement(x) => {
            let val = x.as_ref().map_or(Value::Void, |x| eval_expression(x, local_context));
            if local_context.return_value.is_none() {
                local_context.return_value = Some(val);
            }
            local_context.return_value.clone().unwrap()
        }
        Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } => {
            let cache = load_property_helper(local_context.component_instance, &layout_cache_prop.element(), layout_cache_prop.name()).unwrap();
            if let Value::LayoutCache(cache) = cache {
                if let Some(ri) = repeater_index {
                    let offset : usize = eval_expression(ri, local_context).try_into().unwrap();
                    Value::Number(cache[(cache[*index] as usize) + offset * 2].into())
                } else {
                    Value::Number(cache[*index].into())
                }
            } else {
                panic!("invalid layout cache")
            }
        }
        Expression::ComputeLayoutInfo(lay, o) => crate::eval_layout::compute_layout_info(lay, *o, local_context),
        Expression::SolveLayout(lay, o) => crate::eval_layout::solve_layout(lay, *o, local_context),
    }
}

fn call_builtin_function(
    f: BuiltinFunction,
    arguments: &[Expression],
    local_context: &mut EvalLocalContext,
) -> Value {
    match f {
        BuiltinFunction::GetWindowScaleFactor => match local_context.component_instance {
            ComponentInstance::InstanceRef(component) => {
                Value::Number(window_ref(component).unwrap().scale_factor() as _)
            }
            ComponentInstance::GlobalComponent(_) => {
                panic!("Cannot get the window from a global component")
            }
        },
        BuiltinFunction::GetWindowDefaultFontSize => match local_context.component_instance {
            ComponentInstance::InstanceRef(component) => Value::Number(
                window_ref(component)
                    .unwrap()
                    .window_item()
                    .unwrap()
                    .as_pin_ref()
                    .default_font_size()
                    .get() as _,
            ),
            ComponentInstance::GlobalComponent(_) => {
                panic!("Cannot get the window from a global component")
            }
        },
        BuiltinFunction::AnimationTick => {
            Value::Number(i_slint_core::animations::animation_tick() as f64)
        }
        BuiltinFunction::Debug => {
            let to_print: SharedString =
                eval_expression(&arguments[0], local_context).try_into().unwrap();
            corelib::debug_log!("{}", to_print);
            Value::Void
        }
        BuiltinFunction::Mod => {
            let mut to_num = |e| -> f64 { eval_expression(e, local_context).try_into().unwrap() };
            Value::Number(to_num(&arguments[0]) % to_num(&arguments[1]))
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
        BuiltinFunction::Log => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let y: f64 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Number(x.log(y))
        }
        BuiltinFunction::Pow => {
            let x: f64 = eval_expression(&arguments[0], local_context).try_into().unwrap();
            let y: f64 = eval_expression(&arguments[1], local_context).try_into().unwrap();
            Value::Number(x.powf(y))
        }
        BuiltinFunction::SetFocusItem => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to SetFocusItem")
            }
            let component = match local_context.component_instance {
                ComponentInstance::InstanceRef(c) => c,
                ComponentInstance::GlobalComponent(_) => {
                    panic!("Cannot access the focus item from a global component")
                }
            };
            if let Expression::ElementReference(focus_item) = &arguments[0] {
                generativity::make_guard!(guard);

                let focus_item = focus_item.upgrade().unwrap();
                let enclosing_component =
                    enclosing_component_for_element(&focus_item, component, guard);
                let component_type = enclosing_component.component_type;

                let item_info = &component_type.items[focus_item.borrow().id.as_str()];

                let focus_item_comp =
                    enclosing_component.self_weak().get().unwrap().upgrade().unwrap();

                window_ref(component).unwrap().set_focus_item(&corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(focus_item_comp),
                    item_info.item_index(),
                ));
                Value::Void
            } else {
                panic!("internal error: argument to SetFocusItem must be an element")
            }
        }
        BuiltinFunction::ShowPopupWindow => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to ShowPopupWindow")
            }
            let component = match local_context.component_instance {
                ComponentInstance::InstanceRef(c) => c,
                ComponentInstance::GlobalComponent(_) => {
                    panic!("Cannot show popup from a global component")
                }
            };
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
                let x = load_property_helper(
                    local_context.component_instance,
                    &popup.x.element(),
                    popup.x.name(),
                )
                .unwrap();
                let y = load_property_helper(
                    local_context.component_instance,
                    &popup.y.element(),
                    popup.y.name(),
                )
                .unwrap();

                generativity::make_guard!(guard);
                let enclosing_component =
                    enclosing_component_for_element(&popup.parent_element, component, guard);
                let parent_item_info = &enclosing_component.component_type.items
                    [popup.parent_element.borrow().id.as_str()];
                let parent_item_comp =
                    enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
                let parent_item = corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(parent_item_comp),
                    parent_item_info.item_index(),
                );

                crate::dynamic_component::show_popup(
                    popup,
                    i_slint_core::graphics::Point::new(
                        x.try_into().unwrap(),
                        y.try_into().unwrap(),
                    ),
                    popup.close_on_click,
                    component.borrow(),
                    window_adapter_ref(component).unwrap(),
                    &parent_item,
                );
                Value::Void
            } else {
                panic!("internal error: argument to SetFocusItem must be an element")
            }
        }
        BuiltinFunction::ClosePopupWindow => {
            let component = match local_context.component_instance {
                ComponentInstance::InstanceRef(c) => c,
                ComponentInstance::GlobalComponent(_) => {
                    panic!("Cannot show popup from a global component")
                }
            };

            window_ref(component).unwrap().close_popup();

            Value::Void
        }
        BuiltinFunction::ItemMemberFunction(name) => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to item member function call")
            }
            let component = match local_context.component_instance {
                ComponentInstance::InstanceRef(c) => c,
                ComponentInstance::GlobalComponent(_) => {
                    panic!("Cannot invoke member function on item from a global component")
                }
            };
            if let Expression::ElementReference(element) = &arguments[0] {
                generativity::make_guard!(guard);

                let elem = element.upgrade().unwrap();
                let enclosing_component = enclosing_component_for_element(&elem, component, guard);
                let component_type = enclosing_component.component_type;
                let item_info = &component_type.items[elem.borrow().id.as_str()];
                let item_ref =
                    unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };

                let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();
                let item_rc = corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(item_comp),
                    item_info.item_index(),
                );

                let window_adapter = window_adapter_ref(component).unwrap();

                // TODO: Make this generic through RTTI
                if let Some(textinput) =
                    ItemRef::downcast_pin::<corelib::items::TextInput>(item_ref)
                {
                    match &*name {
                        "select_all" => textinput.select_all(window_adapter, &item_rc),
                        "cut" => textinput.cut(window_adapter, &item_rc),
                        "copy" => textinput.copy(window_adapter, &item_rc),
                        "paste" => textinput.paste(window_adapter, &item_rc),
                        _ => panic!("internal: Unknown member function {name} called on TextInput"),
                    }
                } else {
                    panic!(
                        "internal error: member function called on element that doesn't have it: {}",
                        elem.borrow().original_name()
                    )
                }

                Value::Void
            } else {
                panic!("internal error: argument to TextInputSelectAll must be an element")
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

            let (Value::Brush(Brush::SolidColor(color_a)), Value::Brush(Brush::SolidColor(color_b)), Value::Number(factor)) = (arg0, arg1, arg2) else {
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
            let r: u8 = r.max(0).min(255) as u8;
            let g: u8 = g.max(0).min(255) as u8;
            let b: u8 = b.max(0).min(255) as u8;
            let a: u8 = (255. * a).max(0.).min(255.) as u8;
            Value::Brush(Brush::SolidColor(Color::from_argb_u8(a, r, g, b)))
        }
        BuiltinFunction::DarkColorScheme => match local_context.component_instance {
            ComponentInstance::InstanceRef(component) => {
                Value::Bool(window_adapter_ref(component).unwrap().dark_color_scheme())
            }
            ComponentInstance::GlobalComponent(_) => {
                panic!("Cannot get the window from a global component")
            }
        },
        BuiltinFunction::TextInputFocused => match local_context.component_instance {
            ComponentInstance::InstanceRef(component) => {
                Value::Bool(window_ref(component).unwrap().text_input_focused() as _)
            }
            ComponentInstance::GlobalComponent(_) => {
                panic!("Cannot get the window from a global component")
            }
        },
        BuiltinFunction::SetTextInputFocused => match local_context.component_instance {
            ComponentInstance::InstanceRef(component) => {
                window_ref(component).unwrap().set_text_input_focused(
                    eval_expression(&arguments[0], local_context).try_into().unwrap(),
                );
                Value::Void
            }
            ComponentInstance::GlobalComponent(_) => {
                panic!("Cannot get the window from a global component")
            }
        },
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            let component = match local_context.component_instance {
                ComponentInstance::InstanceRef(c) => c,
                ComponentInstance::GlobalComponent(_) => {
                    panic!("Cannot access the implicit item size from a global component")
                }
            };
            if let [Expression::ElementReference(item)] = arguments {
                generativity::make_guard!(guard);

                let item = item.upgrade().unwrap();
                let enclosing_component = enclosing_component_for_element(&item, component, guard);
                let component_type = enclosing_component.component_type;
                let item_info = &component_type.items[item.borrow().id.as_str()];
                let item_ref =
                    unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };

                let window_adapter = window_adapter_ref(component).unwrap();
                item_ref
                    .as_ref()
                    .layout_info(crate::eval_layout::to_runtime(orient), window_adapter)
                    .into()
            } else {
                panic!("internal error: incorrect arguments to ImplicitLayoutInfo {:?}", arguments);
            }
        }
        BuiltinFunction::ItemAbsolutePosition => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ItemAbsolutePosition")
            }

            let component = match local_context.component_instance {
                ComponentInstance::InstanceRef(c) => c,
                ComponentInstance::GlobalComponent(_) => {
                    panic!("Cannot access the implicit item size from a global component")
                }
            };

            if let Expression::ElementReference(item) = &arguments[0] {
                generativity::make_guard!(guard);

                let item = item.upgrade().unwrap();
                let enclosing_component = enclosing_component_for_element(&item, component, guard);
                let component_type = enclosing_component.component_type;

                let item_info = &component_type.items[item.borrow().id.as_str()];

                let item_comp = enclosing_component.self_weak().get().unwrap().upgrade().unwrap();

                let item_rc = corelib::items::ItemRc::new(
                    vtable::VRc::into_dyn(item_comp),
                    item_info.item_index(),
                );

                let absolute_point = item_rc.map_to_window(Default::default());

                Value::Struct(
                    [
                        ("x".to_string(), absolute_point.x.try_into().unwrap()),
                        ("y".to_string(), absolute_point.y.try_into().unwrap()),
                    ]
                    .into_iter()
                    .collect(),
                )
            } else {
                panic!("internal error: argument to SetFocusItem must be an element")
            }
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            if arguments.len() != 1 {
                panic!("internal error: incorrect argument count to RegisterCustomFontByPath")
            }
            let component = match local_context.component_instance {
                ComponentInstance::InstanceRef(c) => c,
                ComponentInstance::GlobalComponent(_) => {
                    panic!("Cannot access the implicit item size from a global component")
                }
            };
            if let Value::String(s) = eval_expression(&arguments[0], local_context) {
                if let Some(err) = window_adapter_ref(component)
                    .unwrap()
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
            let arguments = eval_expression(&arguments[3], local_context);
            let Value::Model(arguments) = arguments else { panic!("Args to translate not a model {arguments:?}") };
            struct StringModelWrapper(ModelRc<Value>);
            impl corelib::translations::FormatArgs for StringModelWrapper {
                type Output<'a> = SharedString;
                fn from_index<'a>(&'a self, index: usize) -> Option<SharedString> {
                    self.0.row_data(index).map(|x| x.try_into().unwrap())
                }
            }
            Value::String(corelib::translations::translate(
                &original,
                &context,
                &domain,
                &StringModelWrapper(arguments),
            ))
        }
    }
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
        (lhs, rhs, op) => panic!("unsupported {:?} {} {:?}", lhs, op, rhs),
    };
    match lhs {
        Expression::PropertyReference(nr) => {
            let element = nr.element();
            generativity::make_guard!(guard);
            let enclosing_component = enclosing_component_instance_for_element(
                &element,
                local_context.component_instance,
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
                            enclosing_component.component_type.custom_properties.get(nr.name())
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
                        &enclosing_component.component_type.items[element.borrow().id.as_str()];
                    let item =
                        unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
                    let p = &item_info.rtti.properties[nr.name()];
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
                o.set_field(name.to_owned(), r);
                eval_assignment(base, '=', Value::Struct(o), local_context)
            }
        }
        Expression::RepeaterModelReference { element } => {
            let element = element.upgrade().unwrap();
            let component_instance = match local_context.component_instance {
                ComponentInstance::InstanceRef(i) => i,
                ComponentInstance::GlobalComponent(_) => panic!("can't have repeater in global"),
            };
            generativity::make_guard!(g1);
            let enclosing_component =
                enclosing_component_for_element(&element, component_instance, g1);
            // we need a 'static Repeater component in order to call model_set_row_data, so get it.
            // Safety: This is the only 'static Id in scope.
            let static_guard =
                unsafe { generativity::Guard::new(generativity::Id::<'static>::new()) };
            let repeater = crate::dynamic_component::get_repeater_by_name(
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
                    let index = index as usize;
                    if (index) < model.row_count() {
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
    load_property_helper(ComponentInstance::InstanceRef(component), element, name)
}

fn load_property_helper(
    component_instance: ComponentInstance,
    element: &ElementRc,
    name: &str,
) -> Result<Value, ()> {
    generativity::make_guard!(guard);
    match enclosing_component_instance_for_element(element, component_instance, guard) {
        ComponentInstance::InstanceRef(enclosing_component) => {
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(x) = enclosing_component.component_type.custom_properties.get(name) {
                    return unsafe {
                        x.prop.get(Pin::new_unchecked(&*enclosing_component.as_ptr().add(x.offset)))
                    };
                } else if enclosing_component.component_type.original.is_global() {
                    return Err(());
                }
            };
            let item_info = enclosing_component
                .component_type
                .items
                .get(element.id.as_str())
                .unwrap_or_else(|| panic!("Unknown element for {}.{}", element.id, name));
            core::mem::drop(element);
            let item = unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
            Ok(item_info.rtti.properties.get(name).ok_or(())?.get(item))
        }
        ComponentInstance::GlobalComponent(glob) => Ok(glob.as_ref().get_property(name).unwrap()),
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
        ComponentInstance::InstanceRef(component_instance),
        guard,
    ) {
        ComponentInstance::InstanceRef(enclosing_component) => {
            let maybe_animation = match element.borrow().bindings.get(name) {
                Some(b) => crate::dynamic_component::animation_for_property(
                    enclosing_component,
                    &b.borrow().animation,
                ),
                None => {
                    crate::dynamic_component::animation_for_property(enclosing_component, &None)
                }
            };

            let component = element.borrow().enclosing_component.upgrade().unwrap();
            if element.borrow().id == component.root_element.borrow().id {
                if let Some(x) = enclosing_component.component_type.custom_properties.get(name) {
                    if let Some(orig_decl) = enclosing_component
                        .component_type
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
                } else if enclosing_component.component_type.original.is_global() {
                    return Err(SetPropertyError::NoSuchProperty);
                }
            };
            let item_info = &enclosing_component.component_type.items[element.borrow().id.as_str()];
            let item = unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
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
        Type::Struct { fields, .. } => {
            matches!(value, Value::Struct(str) if str.iter().all(|(k, v)| fields.get(k).map_or(false, |ty| check_value_type(v, ty))))
        }
        Type::Enumeration(en) => {
            matches!(value, Value::EnumerationValue(name, _) if name == en.name.as_str())
        }
        Type::LayoutCache => matches!(value, Value::LayoutCache(_)),
    }
}

pub(crate) fn invoke_callback(
    component_instance: ComponentInstance,
    element: &ElementRc,
    callback_name: &str,
    args: &[Value],
) -> Option<Value> {
    generativity::make_guard!(guard);
    match enclosing_component_instance_for_element(element, component_instance, guard) {
        ComponentInstance::InstanceRef(enclosing_component) => {
            let component_type = enclosing_component.component_type;
            let element = element.borrow();
            if element.id == element.enclosing_component.upgrade().unwrap().root_element.borrow().id
            {
                if let Some(callback_offset) = component_type.custom_callbacks.get(callback_name) {
                    let callback = callback_offset.apply(&*enclosing_component.instance);
                    let res = callback.call(args);
                    return Some(if res != Value::Void {
                        res
                    } else if let Some(Type::Callback { return_type: Some(rt), .. }) =
                        component_type
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
                        default_value_for_type(&*rt)
                    } else {
                        res
                    });
                } else if enclosing_component.component_type.original.is_global() {
                    return None;
                }
            };
            let item_info = &component_type.items[element.id.as_str()];
            let item = unsafe { item_info.item_from_component(enclosing_component.as_ptr()) };
            item_info.rtti.callbacks.get(callback_name).map(|callback| callback.call(item, args))
        }
        ComponentInstance::GlobalComponent(global) => {
            Some(global.as_ref().invoke_callback(callback_name, args).unwrap())
        }
    }
}

/// Invoke the function.
///
/// Return None if the function don't exist
pub(crate) fn call_function(
    component_instance: ComponentInstance,
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

fn root_component_instance<'a, 'old_id, 'new_id>(
    component: InstanceRef<'a, 'old_id>,
    guard: generativity::Guard<'new_id>,
) -> InstanceRef<'a, 'new_id> {
    if let Some(parent_offset) = component.component_type.parent_component_offset {
        let parent_component =
            if let Some(parent) = parent_offset.apply(&*component.instance.get_ref()) {
                *parent
            } else {
                panic!("invalid parent ptr");
            };
        // we need a 'static guard in order to be able to re-borrow with lifetime 'a.
        // Safety: This is the only 'static Id in scope.
        let static_guard = unsafe { generativity::Guard::new(generativity::Id::<'static>::new()) };
        root_component_instance(
            unsafe { InstanceRef::from_pin_ref(parent_component, static_guard) },
            guard,
        )
    } else {
        // Safety: new_id is an unique id
        unsafe {
            std::mem::transmute::<InstanceRef<'a, 'old_id>, InstanceRef<'a, 'new_id>>(component)
        }
    }
}

pub fn window_adapter_ref<'a>(
    component: InstanceRef<'a, '_>,
) -> Option<&'a Rc<dyn i_slint_core::window::WindowAdapter>> {
    component.component_type.window_adapter_offset.apply(component.instance.get_ref()).as_ref()
}

pub fn window_ref<'a>(
    component: InstanceRef<'a, '_>,
) -> Option<&'a i_slint_core::window::WindowInner> {
    window_adapter_ref(component)
        .map(|window_adapter| WindowInner::from_pub(window_adapter.window()))
}

/// Return the component instance which hold the given element.
/// Does not take in account the global component.
pub fn enclosing_component_for_element<'a, 'old_id, 'new_id>(
    element: &'a ElementRc,
    component: InstanceRef<'a, 'old_id>,
    guard: generativity::Guard<'new_id>,
) -> InstanceRef<'a, 'new_id> {
    let enclosing = &element.borrow().enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(enclosing, &component.component_type.original) {
        // Safety: new_id is an unique id
        unsafe {
            std::mem::transmute::<InstanceRef<'a, 'old_id>, InstanceRef<'a, 'new_id>>(component)
        }
    } else {
        assert!(!enclosing.is_global());
        let parent_component = component
            .component_type
            .parent_component_offset
            .unwrap()
            .apply(component.as_ref())
            .unwrap();
        generativity::make_guard!(new_guard);
        let parent_instance = unsafe { InstanceRef::from_pin_ref(parent_component, new_guard) };
        let parent_instance = unsafe {
            core::mem::transmute::<InstanceRef, InstanceRef<'a, 'static>>(parent_instance)
        };
        enclosing_component_for_element(element, parent_instance, guard)
    }
}

/// Return the component instance which hold the given element.
/// The difference with enclosing_component_for_element is that it takes the GlobalComponent into account.
pub(crate) fn enclosing_component_instance_for_element<'a, 'old_id, 'new_id>(
    element: &'a ElementRc,
    component_instance: ComponentInstance<'a, 'old_id>,
    guard: generativity::Guard<'new_id>,
) -> ComponentInstance<'a, 'new_id> {
    let enclosing = &element.borrow().enclosing_component.upgrade().unwrap();
    match component_instance {
        ComponentInstance::InstanceRef(component) => {
            if enclosing.is_global() && !Rc::ptr_eq(enclosing, &component.component_type.original) {
                // we need a 'static guard in order to be able to borrow from `root` otherwise it does not work because of variance.
                // Safety: This is the only 'static Id in scope.
                let static_guard =
                    unsafe { generativity::Guard::new(generativity::Id::<'static>::new()) };
                let root = root_component_instance(component, static_guard);
                ComponentInstance::GlobalComponent(
                    &root.component_type.extra_data_offset.apply(&*root.instance.get_ref()).globals
                        [enclosing.root_element.borrow().id.as_str()],
                )
            } else {
                ComponentInstance::InstanceRef(enclosing_component_for_element(
                    element, component, guard,
                ))
            }
        }
        ComponentInstance::GlobalComponent(global) => {
            //assert!(Rc::ptr_eq(enclosing, &global.component));
            ComponentInstance::GlobalComponent(global)
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
        Type::Struct { fields, .. } => Value::Struct(
            fields.iter().map(|(n, t)| (n.clone(), default_value_for_type(t))).collect::<Struct>(),
        ),
        Type::Array(_) | Type::Model => Value::Model(Default::default()),
        Type::Percent => Value::Number(0.),
        Type::Enumeration(e) => {
            Value::EnumerationValue(e.name.clone(), e.values.get(e.default_value).unwrap().clone())
        }
        Type::Easing => Value::EasingCurve(Default::default()),
        Type::Void | Type::Invalid => Value::Void,
        Type::UnitProduct(_) => Value::Number(0.),
        Type::PathData => Value::PathData(Default::default()),
        Type::LayoutCache => Value::LayoutCache(Default::default()),
        Type::InferredProperty
        | Type::InferredCallback
        | Type::ElementReference
        | Type::Function { .. } => {
            panic!("There can't be such property")
        }
    }
}
