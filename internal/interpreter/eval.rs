// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Tree-walking evaluator for [`llr::Expression`].
//!
//! Called from property bindings, change callbacks, callback handlers,
//! layout info expressions and `init_code` blocks.
//! Resolves `MemberReference`s by walking the sub-component parent chain.

use crate::Value;
use crate::globals::{GlobalInstance, GlobalStorage};
use crate::instance::SubComponentInstance;
use i_slint_compiler::expression_tree::{BuiltinFunction, MinMaxOp};
use i_slint_compiler::langtype::Type;
use i_slint_compiler::llr::{self, Expression, LocalMemberIndex, MemberReference};
use i_slint_core::graphics::{
    Brush, ConicGradientBrush, GradientStop, LinearGradientBrush, RadialGradientBrush,
};
use i_slint_core::model::{Model, ModelExt, ModelRc, SharedVectorModel};
use i_slint_core::{Color, SharedString, SharedVector};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::{Rc, Weak};

/// Dynamic context for one expression evaluation.
pub struct EvalContext {
    /// Closest sub-component, set when the expression is evaluated from one.
    /// `None` when the expression is being evaluated in a global's init code.
    pub current: Option<Pin<Rc<SubComponentInstance>>>,
    /// Shared global storage, used to resolve `MemberReference::Global`.
    pub globals: Weak<GlobalStorage>,
    /// Local variables introduced by `StoreLocalVariable`.
    pub locals: HashMap<SmolStr, Value>,
    /// Arguments of the current function, if any.
    pub function_arguments: Vec<Value>,
    /// Set by `return` to stop further statement evaluation in a `CodeBlock`.
    pub return_value: Option<Value>,
}

impl EvalContext {
    /// Context rooted in a sub-component.
    /// The global storage is pulled from the sub-component's owning root.
    pub fn new(current: Pin<Rc<SubComponentInstance>>) -> Self {
        let globals = current
            .root
            .get()
            .and_then(|w| w.upgrade())
            .map(|inst| Rc::downgrade(&inst.globals))
            .unwrap_or_default();
        Self {
            current: Some(current),
            globals,
            locals: HashMap::new(),
            function_arguments: Vec::new(),
            return_value: None,
        }
    }

    /// Context rooted in a global. Only `MemberReference::Global` is valid.
    pub fn for_global(globals: Weak<GlobalStorage>) -> Self {
        Self {
            current: None,
            globals,
            locals: HashMap::new(),
            function_arguments: Vec::new(),
            return_value: None,
        }
    }

    pub fn with_arguments(current: Pin<Rc<SubComponentInstance>>, args: Vec<Value>) -> Self {
        let mut ctx = Self::new(current);
        ctx.function_arguments = args;
        ctx
    }
}

/// Walk `parent_level` steps up the parent chain.
fn walk_parent(
    start: &Pin<Rc<SubComponentInstance>>,
    level: usize,
) -> Pin<Rc<SubComponentInstance>> {
    let mut current = start.clone();
    for _ in 0..level {
        let parent = current.parent.upgrade().expect("parent vanished during evaluation");
        current = Pin::new(parent);
    }
    current
}

/// Walk down a `sub_component_path`.
fn walk_sub_path(
    mut current: Pin<Rc<SubComponentInstance>>,
    path: &[llr::SubComponentInstanceIdx],
) -> Pin<Rc<SubComponentInstance>> {
    for &idx in path {
        let next = current.sub_components[idx].clone();
        current = next;
    }
    current
}

/// Walk to the sub-component that owns `local`.
///
/// Panics if `ctx.current` is unset; the caller must check beforehand.
fn walk_to(
    ctx: &EvalContext,
    parent_level: usize,
    path: &[llr::SubComponentInstanceIdx],
) -> Pin<Rc<SubComponentInstance>> {
    let start = ctx.current.as_ref().expect("relative member reference without a sub-component");
    walk_sub_path(walk_parent(start, parent_level), path)
}

/// Scan the flat `item_table` for the entry whose `(sub_component_path,
/// item_idx)` matches the request, returning the flat tree index. Used by
/// `ItemAbsolutePosition` to address an item that lives in a nested
/// sub-component.
fn find_flat_item_index(
    item_table: &[Option<(
        Box<[i_slint_compiler::llr::SubComponentInstanceIdx]>,
        i_slint_compiler::llr::ItemInstanceIdx,
    )>],
    path: &[i_slint_compiler::llr::SubComponentInstanceIdx],
    item_index: i_slint_compiler::llr::ItemInstanceIdx,
) -> Option<usize> {
    item_table.iter().position(|entry| {
        entry.as_ref().is_some_and(|(p, i)| p.as_ref() == path && *i == item_index)
    })
}

fn load_local(instance: &SubComponentInstance, member: &LocalMemberIndex) -> Value {
    match member {
        LocalMemberIndex::Property(idx) => Pin::as_ref(&instance.properties[*idx]).get(),
        LocalMemberIndex::Native { item_index, prop_name, .. } => {
            Pin::as_ref(&instance.items[*item_index]).get_property(prop_name).unwrap_or(Value::Void)
        }
        LocalMemberIndex::Callback(_) | LocalMemberIndex::Function(_) => {
            panic!("load_local called on callback/function reference")
        }
    }
}

fn store_local(instance: &SubComponentInstance, member: &LocalMemberIndex, value: Value) {
    match member {
        LocalMemberIndex::Property(idx) => {
            let prop = Pin::as_ref(&instance.properties[*idx]);
            // Check if this property has a standalone `animate` declaration.
            // If so, use `set_animated_value` so the value transition is
            // interpolated over the declared duration/easing.
            let cu = &instance.compilation_unit;
            let sc = &cu.sub_components[instance.sub_component_idx];
            let local_ref = i_slint_compiler::llr::LocalMemberReference {
                sub_component_path: Vec::new(),
                reference: member.clone(),
            };
            if let Some(anim_expr) = sc.animations.get(&local_ref) {
                let anim_expr = anim_expr.clone();
                if let Some(owner) = instance.root.get().and_then(|w| w.upgrade()) {
                    let owner_pin = find_sub_component_pin(&owner.root_sub_component, instance);
                    let mut ctx = EvalContext::new(owner_pin);
                    let anim = crate::bindings::value_to_property_animation(eval_expression(
                        &mut ctx, &anim_expr,
                    ));
                    prop.set_animated_value(value, anim);
                    return;
                }
            }
            prop.set(value);
        }
        LocalMemberIndex::Native { item_index, prop_name, .. } => {
            // Native items handle their own animation via the rtti binding
            // set up in install_property_init, so a plain set is correct.
            let anim = find_native_animation(instance, *item_index, prop_name);
            let _ = Pin::as_ref(&instance.items[*item_index]).set_property(prop_name, value, anim);
        }
        LocalMemberIndex::Callback(_) | LocalMemberIndex::Function(_) => {
            panic!("store_local called on callback/function reference")
        }
    }
}

/// Find the animation expression for a native item property, evaluate it,
/// and return the `PropertyAnimation` if one exists.
fn find_native_animation(
    instance: &SubComponentInstance,
    item_index: i_slint_compiler::llr::ItemInstanceIdx,
    prop_name: &str,
) -> Option<i_slint_core::items::PropertyAnimation> {
    let cu = &instance.compilation_unit;
    let sc = &cu.sub_components[instance.sub_component_idx];
    // Native animations are keyed by their LocalMemberReference with the
    // Native variant.
    let local_ref = i_slint_compiler::llr::LocalMemberReference {
        sub_component_path: Vec::new(),
        reference: LocalMemberIndex::Native {
            item_index,
            prop_name: prop_name.into(),
            kind: i_slint_compiler::llr::NativeMemberKind::Property,
        },
    };
    let anim_expr = sc.animations.get(&local_ref)?;
    let anim_expr = anim_expr.clone();
    let owner = instance.root.get().and_then(|w| w.upgrade())?;
    let owner_pin = find_sub_component_pin(&owner.root_sub_component, instance);
    let mut ctx = EvalContext::new(owner_pin);
    Some(crate::bindings::value_to_property_animation(eval_expression(&mut ctx, &anim_expr)))
}

/// Walk the sub-component tree to find a Pin<Rc<SubComponentInstance>> that
/// points at the same allocation as `target`. Used to build an EvalContext
/// rooted at the right sub-component.
fn find_sub_component_pin(
    root: &Pin<Rc<SubComponentInstance>>,
    target: &SubComponentInstance,
) -> Pin<Rc<SubComponentInstance>> {
    if std::ptr::eq(&**root as *const SubComponentInstance, target) {
        return root.clone();
    }
    for nested in &root.sub_components {
        if let Some(found) = find_sub_component_pin_inner(nested, target) {
            return found;
        }
    }
    // Fallback: shouldn't happen, but return root to avoid panic
    root.clone()
}

fn find_sub_component_pin_inner(
    sub: &Pin<Rc<SubComponentInstance>>,
    target: &SubComponentInstance,
) -> Option<Pin<Rc<SubComponentInstance>>> {
    if std::ptr::eq(&**sub as *const SubComponentInstance, target) {
        return Some(sub.clone());
    }
    for nested in &sub.sub_components {
        if let Some(found) = find_sub_component_pin_inner(nested, target) {
            return Some(found);
        }
    }
    None
}

pub fn load_property(ctx: &EvalContext, mr: &MemberReference) -> Value {
    match mr {
        MemberReference::Global { global_index, member } => {
            let Some(storage) = ctx.globals.upgrade() else { return Value::Void };
            let Some(global) = storage.get(*global_index) else { return Value::Void };
            load_global(global, member)
        }
        MemberReference::Relative { parent_level, local_reference } => {
            let instance = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
            load_local(&instance, &local_reference.reference)
        }
    }
}

pub fn store_property(ctx: &EvalContext, mr: &MemberReference, value: Value) {
    match mr {
        MemberReference::Global { global_index, member } => {
            let Some(storage) = ctx.globals.upgrade() else { return };
            let Some(global) = storage.get(*global_index) else { return };
            store_global(global, member, value);
        }
        MemberReference::Relative { parent_level, local_reference } => {
            let instance = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
            store_local(&instance, &local_reference.reference, value);
        }
    }
}

pub fn invoke_callback(ctx: &EvalContext, mr: &MemberReference, args: &[Value]) -> Value {
    match mr {
        MemberReference::Global { global_index, member } => {
            let Some(storage) = ctx.globals.upgrade() else { return Value::Void };
            let Some(global) = storage.get(*global_index) else { return Value::Void };
            let LocalMemberIndex::Callback(idx) = member else {
                panic!("invoke_callback on non-callback global reference")
            };
            let cb = &global.compilation_unit.globals[global.global_idx].callbacks[*idx];
            let res = Pin::as_ref(&global.callbacks[*idx]).call(&(args.to_vec(),));
            ensure_typed_default(res, &cb.ret_ty)
        }
        MemberReference::Relative { parent_level, local_reference } => {
            let instance = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
            match &local_reference.reference {
                LocalMemberIndex::Callback(idx) => {
                    let res = Pin::as_ref(&instance.callbacks[*idx]).call(&(args.to_vec(),));
                    let ret_ty = instance.compilation_unit.sub_components
                        [instance.sub_component_idx]
                        .callbacks[*idx]
                        .ret_ty
                        .clone();
                    ensure_typed_default(res, &ret_ty)
                }
                LocalMemberIndex::Native { item_index, prop_name, .. } => {
                    Pin::as_ref(&instance.items[*item_index])
                        .call_callback(prop_name, args)
                        .unwrap_or(Value::Void)
                }
                _ => panic!("invoke_callback on non-callback reference: {mr:?}"),
            }
        }
    }
}

/// Replace `Value::Void` results with the type-appropriate default.
/// Used when an unset callback returns the `Value` default and the caller
/// expects a numeric / string / bool.
fn ensure_typed_default(value: Value, ret_ty: &Type) -> Value {
    if matches!(value, Value::Void) { default_value_for_type(ret_ty) } else { value }
}

pub fn invoke_function(ctx: &EvalContext, mr: &MemberReference, args: Vec<Value>) -> Value {
    match mr {
        MemberReference::Global { global_index, member } => {
            let Some(storage) = ctx.globals.upgrade() else { return Value::Void };
            let Some(global) = storage.get(*global_index) else { return Value::Void };
            let LocalMemberIndex::Function(idx) = member else {
                panic!("invoke_function on non-function global reference")
            };
            let code =
                global.compilation_unit.globals[global.global_idx].functions[*idx].code.clone();
            let mut inner_ctx = EvalContext::for_global(ctx.globals.clone());
            inner_ctx.function_arguments = args;
            eval_expression(&mut inner_ctx, &code)
        }
        MemberReference::Relative { parent_level, local_reference } => {
            let instance = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
            let LocalMemberIndex::Function(idx) = &local_reference.reference else {
                panic!("invoke_function on non-function reference")
            };
            let sc = &instance.compilation_unit.sub_components[instance.sub_component_idx];
            let function = &sc.functions[*idx];
            let mut inner_ctx = EvalContext::with_arguments(instance.clone(), args);
            eval_expression(&mut inner_ctx, &function.code)
        }
    }
}

fn load_global(global: &Rc<GlobalInstance>, member: &LocalMemberIndex) -> Value {
    match member {
        LocalMemberIndex::Property(idx) => Pin::as_ref(&global.properties[*idx]).get(),
        _ => panic!("load_global called on non-property"),
    }
}

fn store_global(global: &Rc<GlobalInstance>, member: &LocalMemberIndex, value: Value) {
    if let LocalMemberIndex::Property(idx) = member {
        Pin::as_ref(&global.properties[*idx]).set(value);
    }
}

/// Build a `Value::PathData` from the `from` expression of a
/// `Expression::Cast { to: Type::PathData, .. }`.
///
/// `lower_expression::compile_path` lowers `Path::Elements` to an
/// `Expression::Array` of builtin-struct literals, `Path::Events` to an
/// `Expression::Struct` with `events` / `points` fields, and `Path::Commands`
/// to a plain string expression. The codegens navigate these statically; the
/// interpreter has to pattern-match on the unlowered expression to recover
/// which path shape to build, since `Value::Struct` doesn't carry its LLR
/// type name.
fn cast_to_path_data(ctx: &mut EvalContext, from: &Expression) -> Value {
    use i_slint_core::graphics::PathData;
    use i_slint_core::items::PathEvent;

    match from {
        Expression::Array { values, .. } => {
            let elements: SharedVector<i_slint_core::graphics::PathElement> =
                values.iter().filter_map(|e| path_element_from_expression(ctx, e)).collect();
            Value::PathData(PathData::Elements(elements))
        }
        Expression::Struct { values, .. }
            if values.contains_key("events") && values.contains_key("points") =>
        {
            let events_value = eval_expression(ctx, &values["events"]);
            let points_value = eval_expression(ctx, &values["points"]);
            // `for_each_enums!` already produces a `TryFrom<Value>` impl for
            // every Slint enum (via `declare_value_enum_conversion!` in
            // `api.rs`), so model rows of `Value::EnumerationValue` convert
            // straight to `PathEvent` without manual string matching.
            let events: SharedVector<PathEvent> = match events_value {
                Value::Model(m) => {
                    (0..m.row_count()).filter_map(|i| m.row_data(i)?.try_into().ok()).collect()
                }
                _ => SharedVector::default(),
            };
            let points: SharedVector<lyon_path::math::Point> = match points_value {
                Value::Model(m) => (0..m.row_count())
                    .filter_map(|i| {
                        let Value::Struct(s) = m.row_data(i)? else { return None };
                        let x = f64::try_from(s.get_field("x").cloned()?).ok()? as f32;
                        let y = f64::try_from(s.get_field("y").cloned()?).ok()? as f32;
                        Some(lyon_path::math::Point::new(x, y))
                    })
                    .collect(),
                _ => SharedVector::default(),
            };
            Value::PathData(PathData::Events(events, points))
        }
        _ => match eval_expression(ctx, from) {
            Value::String(s) => Value::PathData(PathData::Commands(s)),
            _ => Value::PathData(PathData::None),
        },
    }
}

/// Resolve an `Expression::Struct` in a `Cast`-to-`PathData` array into the
/// matching [`PathElement`] variant, dispatching on the struct's
/// `StructName::BuiltinPrivate` tag.
fn path_element_from_expression(
    ctx: &mut EvalContext,
    expr: &Expression,
) -> Option<i_slint_core::graphics::PathElement> {
    use i_slint_compiler::langtype::{BuiltinPrivateStruct, StructName};
    use i_slint_core::graphics::{
        PathArcTo, PathCubicTo, PathElement, PathLineTo, PathMoveTo, PathQuadraticTo,
    };
    let Expression::Struct { ty, values } = expr else { return None };
    let StructName::BuiltinPrivate(bs) = &ty.name else { return None };
    let get_f32 = |field: &str, ctx: &mut EvalContext| -> f32 {
        values
            .get(field)
            .map(|e| eval_expression(ctx, e))
            .and_then(|v| f64::try_from(v).ok())
            .unwrap_or(0.0) as f32
    };
    let get_bool = |field: &str, ctx: &mut EvalContext| -> bool {
        values
            .get(field)
            .map(|e| eval_expression(ctx, e))
            .map(|v| matches!(v, Value::Bool(true)))
            .unwrap_or(false)
    };
    Some(match bs {
        BuiltinPrivateStruct::PathMoveTo => {
            PathElement::MoveTo(PathMoveTo { x: get_f32("x", ctx), y: get_f32("y", ctx) })
        }
        BuiltinPrivateStruct::PathLineTo => {
            PathElement::LineTo(PathLineTo { x: get_f32("x", ctx), y: get_f32("y", ctx) })
        }
        BuiltinPrivateStruct::PathArcTo => PathElement::ArcTo(PathArcTo {
            x: get_f32("x", ctx),
            y: get_f32("y", ctx),
            radius_x: get_f32("radius_x", ctx),
            radius_y: get_f32("radius_y", ctx),
            x_rotation: get_f32("x_rotation", ctx),
            large_arc: get_bool("large_arc", ctx),
            sweep: get_bool("sweep", ctx),
        }),
        BuiltinPrivateStruct::PathCubicTo => PathElement::CubicTo(PathCubicTo {
            x: get_f32("x", ctx),
            y: get_f32("y", ctx),
            control_1_x: get_f32("control_1_x", ctx),
            control_1_y: get_f32("control_1_y", ctx),
            control_2_x: get_f32("control_2_x", ctx),
            control_2_y: get_f32("control_2_y", ctx),
        }),
        BuiltinPrivateStruct::PathQuadraticTo => PathElement::QuadraticTo(PathQuadraticTo {
            x: get_f32("x", ctx),
            y: get_f32("y", ctx),
            control_x: get_f32("control_x", ctx),
            control_y: get_f32("control_y", ctx),
        }),
        BuiltinPrivateStruct::PathClose => PathElement::Close,
        _ => return None,
    })
}

/// Type-default mirroring an existing `Value`.
/// Used by `ArrayIndex` to synthesize a default when an out-of-bounds row
/// access happens against a non-empty model.
fn default_like(v: Value) -> Value {
    match v {
        Value::Number(_) => Value::Number(0.),
        Value::String(_) => Value::String(Default::default()),
        Value::Bool(_) => Value::Bool(false),
        Value::Brush(_) => Value::Brush(Default::default()),
        Value::Image(_) => Value::Image(Default::default()),
        Value::Model(_) => Value::Model(i_slint_core::model::ModelRc::default()),
        Value::Struct(s) => {
            let fields = s.iter().map(|(k, v)| (k.to_string(), default_like(v.clone()))).collect();
            Value::Struct(fields)
        }
        _ => Value::Void,
    }
}

/// Default value for a type, used by `ArrayIndex` and `RepeaterModelReference`
/// when the underlying store returns `None`.
pub fn default_value_for_type(ty: &Type) -> Value {
    match ty {
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Percent
        | Type::UnitProduct(_) => Value::Number(0.),
        Type::String => Value::String(Default::default()),
        Type::Color | Type::Brush => Value::Brush(Brush::default()),
        Type::Bool => Value::Bool(false),
        Type::Image => Value::Image(Default::default()),
        Type::Struct(s) => Value::Struct(
            s.fields.iter().map(|(k, v)| (k.to_string(), default_value_for_type(v))).collect(),
        ),
        Type::Array(_) | Type::Model => Value::Model(ModelRc::default()),
        Type::Enumeration(en) => {
            let default = en.clone().default_value();
            Value::EnumerationValue(en.name.to_string(), default.to_string())
        }
        _ => Value::Void,
    }
}

pub fn eval_expression(ctx: &mut EvalContext, expression: &Expression) -> Value {
    if let Some(r) = &ctx.return_value {
        return r.clone();
    }
    match expression {
        Expression::StringLiteral(s) => Value::String(s.as_str().into()),
        Expression::NumberLiteral(n) => Value::Number(*n),
        Expression::BoolLiteral(b) => Value::Bool(*b),
        Expression::KeysLiteral(ks) => Value::Keys(i_slint_core::input::make_keys(
            SharedString::from(&*ks.key),
            i_slint_core::input::KeyboardModifiers {
                alt: ks.modifiers.alt,
                control: ks.modifiers.control,
                shift: ks.modifiers.shift,
                meta: ks.modifiers.meta,
            },
            ks.ignore_shift,
            ks.ignore_alt,
        )),
        Expression::PropertyReference(mr) => load_property(ctx, mr),
        Expression::FunctionParameterReference { index } => ctx.function_arguments[*index].clone(),
        Expression::StoreLocalVariable { name, value } => {
            let v = eval_expression(ctx, value);
            ctx.locals.insert(name.clone(), v);
            Value::Void
        }
        Expression::ReadLocalVariable { name, .. } => {
            ctx.locals.get(name).cloned().unwrap_or(Value::Void)
        }
        Expression::StructFieldAccess { base, name } => {
            if let Value::Struct(s) = eval_expression(ctx, base) {
                s.get_field(name).cloned().unwrap_or(Value::Void)
            } else {
                Value::Void
            }
        }
        Expression::ArrayIndex { array, index } => {
            let array_v = eval_expression(ctx, array);
            let index = eval_expression(ctx, index);
            match (array_v, index) {
                (Value::Model(m), Value::Number(i)) => {
                    let idx = i as isize as usize;
                    m.row_data_tracked(idx).unwrap_or_else(|| {
                        // Out of bounds: synthesize a type-default like the
                        // generated code does. Peek at row 0 to learn the
                        // element type, falling back to a numeric zero for
                        // empty models (the most common case in tests).
                        m.row_data(0).map(default_like).unwrap_or(Value::Number(0.))
                    })
                }
                _ => Value::Void,
            }
        }
        Expression::Cast { from, to } => {
            // `Path::Elements` / `Path::Events` / `Path::Commands` lower to a
            // `Cast { to: Type::PathData, .. }` over an array-of-structs,
            // a struct with `events` / `points` fields, or a string
            // expression. Handle each shape explicitly before the generic
            // value-coercion table below; the rtti setter for the `Path`
            // native item needs a real `Value::PathData` rather than the
            // raw `Value::Model`/`Value::Struct` that `from` evaluates to.
            if matches!(to, Type::PathData) {
                return cast_to_path_data(ctx, from);
            }
            let v = eval_expression(ctx, from);
            match (v, to) {
                (Value::Number(n), Type::Int32) => Value::Number(n.trunc()),
                (Value::Number(n), Type::String) => {
                    Value::String(i_slint_core::string::shared_string_from_number(n))
                }
                (Value::Number(n), Type::Color) => Color::from_argb_encoded(n as u32).into(),
                (Value::Brush(brush), Type::Color) => brush.color().into(),
                (Value::EnumerationValue(_, val), Type::String) => Value::String(val.into()),
                (v, _) => v,
            }
        }
        Expression::CodeBlock(sub) => {
            let mut v = Value::Void;
            for e in sub {
                v = eval_expression(ctx, e);
                if let Some(r) = &ctx.return_value {
                    return r.clone();
                }
            }
            v
        }
        Expression::BuiltinFunctionCall { function, arguments } => {
            call_builtin_function(ctx, function.clone(), arguments)
        }
        Expression::CallBackCall { callback, arguments } => {
            let args: Vec<Value> = arguments.iter().map(|e| eval_expression(ctx, e)).collect();
            invoke_callback(ctx, callback, &args)
        }
        Expression::FunctionCall { function, arguments } => {
            let args: Vec<Value> = arguments.iter().map(|e| eval_expression(ctx, e)).collect();
            invoke_function(ctx, function, args)
        }
        Expression::ItemMemberFunctionCall { function } => {
            // Native-item method calls like `TextInput::select_all`,
            // `SwipeGestureHandler::cancel` — the LLR encodes them as a
            // `MemberReference` whose reference is
            // `LocalMemberIndex::Native { item_index, prop_name, .. }`. The
            // rust codegen resolves the method name to a concrete Rust
            // method at compile time (`<TextInput>::select_all(...)`);
            // the interpreter has to dispatch on the item's rtti class
            // name at runtime.
            call_item_member_function(ctx, function)
        }
        Expression::ExtraBuiltinFunctionCall { function, arguments, .. } => {
            crate::eval_layout::call_extra_builtin(ctx, function, arguments)
        }
        Expression::PropertyAssignment { property, value } => {
            let v = eval_expression(ctx, value);
            store_property(ctx, property, v);
            Value::Void
        }
        Expression::ModelDataAssignment { level, value } => {
            let new_value = eval_expression(ctx, value);
            if let Some(current) = ctx.current.as_ref() {
                let mut walker = current.clone();
                for _ in 0..*level {
                    let parent = walker.parent.upgrade().expect("parent vanished");
                    walker = std::pin::Pin::new(parent);
                }
                if let Some((parent_weak, repeater_idx)) = walker.repeated_in.get()
                    && let Some(parent) = parent_weak.upgrade()
                {
                    // Read the row index out of the repeated sub-component's
                    // `model_index` property.
                    let row = walker.compilation_unit.sub_components[walker.sub_component_idx]
                        .properties
                        .iter_enumerated()
                        .find(|(_, p)| p.name.as_str() == "model_index")
                        .map(|(idx, _)| {
                            let v = std::pin::Pin::as_ref(&walker.properties[idx]).get();
                            f64::try_from(v).unwrap_or(0.) as usize
                        })
                        .unwrap_or(0);
                    let parent_pinned = std::pin::Pin::new(parent);
                    let repeater = &parent_pinned.repeaters[*repeater_idx];
                    repeater.model_set_row_data(row, new_value);
                }
            }
            Value::Void
        }
        Expression::ArrayIndexAssignment { array, index, value } => {
            let value = eval_expression(ctx, value);
            let array = eval_expression(ctx, array);
            let index = eval_expression(ctx, index);
            if let (Value::Model(m), Value::Number(i)) = (array, index)
                && i >= 0.0
            {
                let i = i.trunc() as usize;
                if i < m.row_count() {
                    m.set_row_data(i, value);
                }
            }
            Value::Void
        }
        Expression::SliceIndexAssignment { slice_name, index, value } => {
            let value = eval_expression(ctx, value);
            match ctx.locals.get_mut(slice_name.as_str()) {
                Some(Value::ArrayOfU16(vec)) => {
                    if let Value::Number(n) = value
                        && *index < vec.len()
                    {
                        vec.make_mut_slice()[*index] = n as u16;
                    }
                }
                Some(Value::Model(m)) => {
                    if *index < m.row_count() {
                        m.set_row_data(*index, value);
                    }
                }
                _ => {}
            }
            Value::Void
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = eval_expression(ctx, lhs);
            let rhs = eval_expression(ctx, rhs);
            binary_op(*op, lhs, rhs)
        }
        Expression::UnaryOp { sub, op } => {
            let sub = eval_expression(ctx, sub);
            match (sub, op) {
                (Value::Number(a), '+') => Value::Number(a),
                (Value::Number(a), '-') => Value::Number(-a),
                (Value::Bool(a), '!') => Value::Bool(!a),
                // Tolerate unset / wrong-type operands rather than panicking.
                // Silently coerce
                // through `Value::default()` when a property hasn't been
                // initialized yet.
                (Value::Void, '+' | '-') => Value::Number(0.0),
                (Value::Void, '!') => Value::Bool(true),
                (s, o) => panic!("unsupported {o} {s:?}"),
            }
        }
        Expression::ImageReference { resource_ref, nine_slice } => {
            let mut image = load_image_reference(resource_ref);
            if let Some(n) = nine_slice {
                image.set_nine_slice_edges(n[0], n[1], n[2], n[3]);
            }
            Value::Image(image)
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            match eval_expression(ctx, condition) {
                Value::Bool(true) => eval_expression(ctx, true_expr),
                Value::Bool(false) => eval_expression(ctx, false_expr),
                _ => Value::Void,
            }
        }
        Expression::Array { values, .. } => Value::Model(ModelRc::new(SharedVectorModel::from(
            values.iter().map(|e| eval_expression(ctx, e)).collect::<SharedVector<_>>(),
        ))),
        Expression::Struct { values, .. } => Value::Struct(
            values.iter().map(|(k, v)| (k.to_string(), eval_expression(ctx, v))).collect(),
        ),
        Expression::EasingCurve(curve) => {
            use i_slint_compiler::expression_tree::EasingCurve as EC;
            use i_slint_core::animations::EasingCurve as Core;
            Value::EasingCurve(match curve {
                EC::Linear => Core::Linear,
                EC::EaseInElastic => Core::EaseInElastic,
                EC::EaseOutElastic => Core::EaseOutElastic,
                EC::EaseInOutElastic => Core::EaseInOutElastic,
                EC::EaseInBounce => Core::EaseInBounce,
                EC::EaseOutBounce => Core::EaseOutBounce,
                EC::EaseInOutBounce => Core::EaseInOutBounce,
                EC::CubicBezier(a, b, c, d) => Core::CubicBezier([*a, *b, *c, *d]),
            })
        }
        Expression::LinearGradient { angle, stops } => {
            let angle: f32 = eval_expression(ctx, angle).try_into().unwrap_or_default();
            Value::Brush(Brush::LinearGradient(LinearGradientBrush::new(
                angle,
                eval_stops(ctx, stops),
            )))
        }
        Expression::RadialGradient { stops } => Value::Brush(Brush::RadialGradient(
            RadialGradientBrush::new_circle(eval_stops(ctx, stops)),
        )),
        Expression::ConicGradient { from_angle, stops } => {
            let from_angle: f32 = eval_expression(ctx, from_angle).try_into().unwrap_or_default();
            Value::Brush(Brush::ConicGradient(ConicGradientBrush::new(
                from_angle,
                eval_stops(ctx, stops),
            )))
        }
        Expression::EnumerationValue(value) => {
            Value::EnumerationValue(value.enumeration.name.to_string(), value.to_string())
        }
        Expression::LayoutCacheAccess {
            layout_cache_prop,
            index,
            repeater_index,
            entries_per_item,
        } => {
            let cache = load_property(ctx, layout_cache_prop);
            layout_cache_access(ctx, cache, *index, repeater_index.as_deref(), *entries_per_item)
        }
        Expression::GridRepeaterCacheAccess {
            layout_cache_prop,
            index,
            repeater_index,
            stride,
            child_offset,
            inner_repeater_index,
            entries_per_item,
        } => {
            let cache = load_property(ctx, layout_cache_prop);
            let offset: usize = eval_expression(ctx, repeater_index).try_into().unwrap_or_default();
            let stride_val: usize = eval_expression(ctx, stride).try_into().unwrap_or_default();
            let inner_offset: usize = inner_repeater_index
                .as_deref()
                .map(|e| {
                    let i: usize = eval_expression(ctx, e).try_into().unwrap_or_default();
                    i * *entries_per_item
                })
                .unwrap_or(0);
            grid_repeater_cache_access(
                cache,
                *index,
                offset,
                stride_val,
                *child_offset,
                inner_offset,
            )
        }
        Expression::WithLayoutItemInfo {
            cells_variable,
            elements,
            orientation,
            sub_expression,
            ..
        } => with_layout_item_info(ctx, cells_variable, elements, *orientation, sub_expression),
        Expression::WithFlexboxLayoutItemInfo {
            cells_h_variable,
            cells_v_variable,
            elements,
            sub_expression,
            ..
        } => with_flexbox_layout_item_info(
            ctx,
            cells_h_variable,
            cells_v_variable,
            elements,
            sub_expression,
        ),
        Expression::WithGridInputData { cells_variable, elements, sub_expression, .. } => {
            with_grid_input_data(ctx, cells_variable, elements, sub_expression)
        }
        Expression::MinMax { ty: _, op, lhs, rhs } => {
            let Value::Number(lhs) = eval_expression(ctx, lhs) else { return Value::Void };
            let Value::Number(rhs) = eval_expression(ctx, rhs) else { return Value::Void };
            match op {
                MinMaxOp::Min => Value::Number(lhs.min(rhs)),
                MinMaxOp::Max => Value::Number(lhs.max(rhs)),
            }
        }
        Expression::EmptyComponentFactory => Value::ComponentFactory(Default::default()),
        Expression::TranslationReference { .. } => {
            // TranslationReference is only emitted when `bundle-translations`
            // is active, which the interpreter does not use. Runtime @tr()
            // goes through BuiltinFunction::Translate instead.
            Value::String(Default::default())
        }
    }
}

// `call_extra_builtin` and layout converters live in `eval_layout.rs`.

fn with_layout_item_info(
    ctx: &mut EvalContext,
    cells_variable: &str,
    elements: &[itertools::Either<Expression, i_slint_compiler::llr::LayoutRepeatedElement>],
    orientation: i_slint_compiler::layout::Orientation,
    sub_expression: &Expression,
) -> Value {
    let mut cells: Vec<Value> = Vec::with_capacity(elements.len());
    let mut repeated_indices: Vec<u32> = Vec::new();
    let mut repeater_steps: Vec<u32> = Vec::new();
    for el in elements {
        match el {
            itertools::Either::Left(expr) => cells.push(eval_expression(ctx, expr)),
            itertools::Either::Right(repeater) => {
                let offset = cells.len() as u32;
                let (instances, step) = push_repeater_layout_items(
                    ctx,
                    repeater.repeater_index,
                    repeater.row_child_templates.as_deref(),
                    orientation,
                    &mut cells,
                );
                repeated_indices.push(offset);
                repeated_indices.push(instances);
                repeater_steps.push(step);
            }
        }
    }
    let prev_cells =
        ctx.locals.insert(SmolStr::from(cells_variable), Value::Model(model_from_vec(cells)));
    let prev_ri = ctx.locals.insert(
        SmolStr::new_static("repeated_indices"),
        Value::Model(model_from_vec(
            repeated_indices.into_iter().map(|i| Value::Number(i as f64)).collect(),
        )),
    );
    let prev_rs = ctx.locals.insert(
        SmolStr::new_static("repeater_steps"),
        Value::Model(model_from_vec(
            repeater_steps.into_iter().map(|i| Value::Number(i as f64)).collect(),
        )),
    );
    let result = eval_expression(ctx, sub_expression);
    restore_local(ctx, cells_variable, prev_cells);
    restore_local(ctx, "repeated_indices", prev_ri);
    restore_local(ctx, "repeater_steps", prev_rs);
    result
}

fn push_repeater_layout_items(
    ctx: &mut EvalContext,
    repeater_idx: i_slint_compiler::llr::RepeatedElementIdx,
    row_child_templates: Option<&[i_slint_compiler::llr::RowChildTemplateInfo]>,
    orientation: i_slint_compiler::layout::Orientation,
    cells: &mut Vec<Value>,
) -> (u32, u32) {
    use i_slint_core::model::RepeatedItemTree;
    let Some(current) = ctx.current.as_ref() else { return (0, 0) };
    current.ensure_repeater_updated(repeater_idx);
    let repeater = &current.repeaters[repeater_idx];
    let instances = repeater.instances_vec();
    let core_orientation = llr_to_core_orientation(orientation);
    let push_cell = |cells: &mut Vec<Value>, info: i_slint_core::layout::LayoutItemInfo| {
        let mut struct_value = crate::api::Struct::default();
        struct_value.set_field("constraint".to_string(), info.constraint.into());
        cells.push(Value::Struct(struct_value));
    };
    let step = match row_child_templates {
        None => {
            // Column repeater: one cell per instance, asking the sub-component
            // for its own layout info.
            for instance in &instances {
                let info = RepeatedItemTree::layout_item_info(
                    instance.as_pin_ref(),
                    core_orientation,
                    None,
                );
                push_cell(cells, info);
            }
            1
        }
        Some(templates) => {
            // Row repeater: the step is the maximum total child count across
            // instances (static children plus each instance's inner repeaters
            // realized via RowChildTemplateInfo::Repeated).
            let max_total = instances
                .iter()
                .map(|inst| total_row_child_count(&inst.root_sub_component, templates))
                .max()
                .unwrap_or(i_slint_compiler::llr::static_child_count(templates));
            for instance in &instances {
                for child_idx in 0..max_total {
                    let info = RepeatedItemTree::layout_item_info(
                        instance.as_pin_ref(),
                        core_orientation,
                        Some(child_idx),
                    );
                    push_cell(cells, info);
                }
            }
            max_total as u32
        }
    };
    (instances.len() as u32, step)
}

fn total_row_child_count(
    sub: &Pin<std::rc::Rc<crate::instance::SubComponentInstance>>,
    templates: &[i_slint_compiler::llr::RowChildTemplateInfo],
) -> usize {
    use i_slint_compiler::llr::{RowChildTemplateInfo, static_child_count};
    let mut total = static_child_count(templates);
    for entry in templates {
        if let RowChildTemplateInfo::Repeated { repeater_index } = entry {
            sub.ensure_repeater_updated(*repeater_index);
            let repeater = &sub.repeaters[*repeater_index];
            total += repeater.instances_vec().len();
        }
    }
    total
}

fn llr_to_core_orientation(
    o: i_slint_compiler::layout::Orientation,
) -> i_slint_core::items::Orientation {
    match o {
        i_slint_compiler::layout::Orientation::Horizontal => {
            i_slint_core::items::Orientation::Horizontal
        }
        i_slint_compiler::layout::Orientation::Vertical => {
            i_slint_core::items::Orientation::Vertical
        }
    }
}

fn with_flexbox_layout_item_info(
    ctx: &mut EvalContext,
    cells_h_variable: &str,
    cells_v_variable: &str,
    elements: &[itertools::Either<
        (Expression, Expression),
        i_slint_compiler::llr::LayoutRepeatedElement,
    >],
    sub_expression: &Expression,
) -> Value {
    let mut cells_h: Vec<Value> = Vec::with_capacity(elements.len());
    let mut cells_v: Vec<Value> = Vec::with_capacity(elements.len());
    let mut repeated_indices: Vec<u32> = Vec::new();
    for el in elements {
        match el {
            itertools::Either::Left((h, v)) => {
                cells_h.push(eval_expression(ctx, h));
                cells_v.push(eval_expression(ctx, v));
            }
            itertools::Either::Right(repeater) => {
                let offset = cells_h.len() as u32;
                let instances = push_repeater_flexbox_items(
                    ctx,
                    repeater.repeater_index,
                    &mut cells_h,
                    &mut cells_v,
                );
                repeated_indices.push(offset);
                repeated_indices.push(instances);
            }
        }
    }
    let prev_h =
        ctx.locals.insert(SmolStr::from(cells_h_variable), Value::Model(model_from_vec(cells_h)));
    let prev_v =
        ctx.locals.insert(SmolStr::from(cells_v_variable), Value::Model(model_from_vec(cells_v)));
    let prev_ri = ctx.locals.insert(
        SmolStr::new_static("repeated_indices"),
        Value::Model(model_from_vec(
            repeated_indices.into_iter().map(|i| Value::Number(i as f64)).collect(),
        )),
    );
    let result = eval_expression(ctx, sub_expression);
    restore_local(ctx, cells_h_variable, prev_h);
    restore_local(ctx, cells_v_variable, prev_v);
    restore_local(ctx, "repeated_indices", prev_ri);
    result
}

fn push_repeater_flexbox_items(
    ctx: &mut EvalContext,
    repeater_idx: i_slint_compiler::llr::RepeatedElementIdx,
    cells_h: &mut Vec<Value>,
    cells_v: &mut Vec<Value>,
) -> u32 {
    use i_slint_core::items::Orientation;
    use i_slint_core::model::RepeatedItemTree;
    let Some(current) = ctx.current.as_ref() else { return 0 };
    current.ensure_repeater_updated(repeater_idx);
    let repeater = &current.repeaters[repeater_idx];
    let instances = repeater.instances_vec();
    let instance_count = instances.len() as u32;
    for instance in instances {
        // Box layout returns LayoutItemInfo; flexbox needs FlexboxLayoutItemInfo
        // which adds flex-grow / flex-shrink / etc. The default
        // `RepeatedItemTree::flexbox_layout_item_info` impl wraps the box one
        // and zero-fills the flex fields.
        let info_h = RepeatedItemTree::flexbox_layout_item_info(
            instance.as_pin_ref(),
            Orientation::Horizontal,
            None,
        );
        let info_v = RepeatedItemTree::flexbox_layout_item_info(
            instance.as_pin_ref(),
            Orientation::Vertical,
            None,
        );
        cells_h.push(flexbox_item_info_to_value(info_h));
        cells_v.push(flexbox_item_info_to_value(info_v));
    }
    instance_count
}

fn flexbox_item_info_to_value(info: i_slint_core::layout::FlexboxLayoutItemInfo) -> Value {
    let mut s = crate::api::Struct::default();
    s.set_field("constraint".to_string(), info.constraint.into());
    s.set_field("flex_grow".to_string(), Value::Number(info.flex_grow as f64));
    s.set_field("flex_shrink".to_string(), Value::Number(info.flex_shrink as f64));
    s.set_field("flex_basis".to_string(), Value::Number(info.flex_basis as f64));
    s.set_field(
        "flex_align_self".to_string(),
        Value::EnumerationValue(
            "FlexboxLayoutAlignSelf".to_string(),
            format!("{:?}", info.flex_align_self).to_lowercase(),
        ),
    );
    s.set_field("flex_order".to_string(), Value::Number(info.flex_order as f64));
    Value::Struct(s)
}

fn with_grid_input_data(
    ctx: &mut EvalContext,
    cells_variable: &str,
    elements: &[itertools::Either<Expression, i_slint_compiler::llr::GridLayoutRepeatedElement>],
    sub_expression: &Expression,
) -> Value {
    // Mirror `generate_with_grid_input_data` in the Rust codegen:
    // `repeated_indices` holds `(offset, len)` pairs into `cells`; `repeater_steps`
    // holds the per-instance item count. `organize_grid_layout` needs both to
    // thread the repeater indirection through `organized_data`.
    //
    // A `new_row` local tracks whether the next static cell in the same row
    // should start a new row. Each repeater resets it to its static `new_row`
    // before iterating, then sets it to `false` after a column-repeater ran at
    // least once. Static cells after the repeater consult this local via
    // `ReadLocalVariable("new_row")`.
    let saved_new_row = ctx.locals.remove("new_row");
    let mut cells: Vec<Value> = Vec::with_capacity(elements.len());
    let mut repeated_indices: Vec<u32> = Vec::new();
    let mut repeater_steps: Vec<u32> = Vec::new();

    for el in elements {
        match el {
            itertools::Either::Left(expr) => cells.push(eval_expression(ctx, expr)),
            itertools::Either::Right(repeater) => {
                ctx.locals.insert(SmolStr::new_static("new_row"), Value::Bool(repeater.new_row));
                let offset = cells.len() as u32;
                let is_row_repeater = repeater.row_child_templates.is_some();
                let (instances, step) = push_repeater_grid_input_data(
                    ctx,
                    repeater.repeater_index,
                    repeater.new_row,
                    repeater.row_child_templates.as_deref(),
                    &mut cells,
                );
                if !is_row_repeater && instances > 0 {
                    ctx.locals.insert(SmolStr::new_static("new_row"), Value::Bool(false));
                }
                repeated_indices.push(offset);
                repeated_indices.push(instances);
                repeater_steps.push(step);
            }
        }
    }
    restore_local(ctx, "new_row", saved_new_row);

    let prev_cells =
        ctx.locals.insert(SmolStr::from(cells_variable), Value::Model(model_from_vec(cells)));
    let prev_ri = ctx.locals.insert(
        SmolStr::new_static("repeated_indices"),
        Value::Model(model_from_vec(
            repeated_indices.into_iter().map(|i| Value::Number(i as f64)).collect(),
        )),
    );
    let prev_rs = ctx.locals.insert(
        SmolStr::new_static("repeater_steps"),
        Value::Model(model_from_vec(
            repeater_steps.into_iter().map(|i| Value::Number(i as f64)).collect(),
        )),
    );

    let result = eval_expression(ctx, sub_expression);

    restore_local(ctx, cells_variable, prev_cells);
    restore_local(ctx, "repeated_indices", prev_ri);
    restore_local(ctx, "repeater_steps", prev_rs);
    result
}

fn restore_local(ctx: &mut EvalContext, name: &str, prev: Option<Value>) {
    if let Some(prev) = prev {
        ctx.locals.insert(SmolStr::from(name), prev);
    } else {
        ctx.locals.remove(name);
    }
}

fn push_repeater_grid_input_data(
    ctx: &mut EvalContext,
    repeater_idx: i_slint_compiler::llr::RepeatedElementIdx,
    new_row: bool,
    row_child_templates: Option<&[i_slint_compiler::llr::RowChildTemplateInfo]>,
    cells: &mut Vec<Value>,
) -> (u32, u32) {
    use i_slint_compiler::llr::RowChildTemplateInfo;
    use i_slint_core::model::VecModel;
    use std::rc::Rc;
    let Some(current) = ctx.current.as_ref() else { return (0, 0) };
    current.ensure_repeater_updated(repeater_idx);
    let repeater = &current.repeaters[repeater_idx];

    let is_row_repeater = row_child_templates.is_some();
    let static_count =
        row_child_templates.map(i_slint_compiler::llr::static_child_count).unwrap_or(1);

    let instances = repeater.instances_vec();
    let instance_count = instances.len() as u32;

    // Step is the max total cells per instance: static_count + max inner-repeater
    // cells across instances. All instances must contribute exactly `step` entries
    // so that the flattened cell vector lines up with `repeater_steps` and
    // `repeated_indices` the same way the rust codegen emits it.
    let step = if let Some(templates) = row_child_templates {
        instances
            .iter()
            .map(|inst| total_row_child_count(&inst.root_sub_component, templates))
            .max()
            .unwrap_or(static_count)
    } else {
        1
    };

    let mut current_new_row = new_row;

    for instance in &instances {
        let inner_sub = instance.root_sub_component.clone();
        let cu = inner_sub.compilation_unit.clone();
        let sc = &cu.sub_components[inner_sub.sub_component_idx];

        // Evaluate `grid_layout_input_for_repeated` to populate the `statics`
        // array (one entry per `RowChildTemplateInfo::Static`). For a simple
        // column repeater this is the full result.
        let mut statics: Vec<Value> = vec![Value::Void; static_count];
        if let Some(expr) = &sc.grid_layout_input_for_repeated {
            let expr = expr.borrow().clone();
            let mut inner_ctx = EvalContext::new(inner_sub.clone());
            let result_model: Rc<VecModel<Value>> = Rc::new(VecModel::default());
            for _ in 0..static_count {
                result_model.push(Value::Void);
            }
            inner_ctx.locals.insert(
                SmolStr::new_static("result"),
                Value::Model(i_slint_core::model::ModelRc::from(result_model.clone())),
            );
            inner_ctx.locals.insert(SmolStr::new_static("new_row"), Value::Bool(current_new_row));
            eval_expression(&mut inner_ctx, &expr);
            for i in 0..result_model.row_count() {
                if let Some(v) = result_model.row_data(i) {
                    statics[i] = v;
                }
            }
        }

        if let Some(templates) = row_child_templates {
            // Walk templates, interleaving statics and auto-positioned
            // placeholder cells for inner-repeater instances. Any leftover
            // slot up to `step` gets an auto-positioned default as well
            // (matches the Rust codegen's `result[write_idx..].fill(default)`).
            let mut written = 0usize;
            let mut static_idx = 0usize;
            for entry in templates {
                if written >= step {
                    break;
                }
                match entry {
                    RowChildTemplateInfo::Static { .. } => {
                        let mut v = statics.get(static_idx).cloned().unwrap_or(Value::Void);
                        static_idx += 1;
                        override_new_row(&mut v, written == 0 && current_new_row);
                        cells.push(v);
                        written += 1;
                    }
                    RowChildTemplateInfo::Repeated { repeater_index } => {
                        inner_sub.ensure_repeater_updated(*repeater_index);
                        let inner_rep = &inner_sub.repeaters[*repeater_index];
                        let inner_len = inner_rep.instances_vec().len();
                        for _ in 0..inner_len {
                            if written >= step {
                                break;
                            }
                            let mut v = auto_grid_input_data();
                            override_new_row(&mut v, written == 0 && current_new_row);
                            cells.push(v);
                            written += 1;
                        }
                    }
                }
            }
            while written < step {
                cells.push(auto_grid_input_data());
                written += 1;
            }
        } else {
            // Column repeater: one cell per instance.
            cells.push(statics.pop().unwrap_or_else(auto_grid_input_data));
        }

        if !is_row_repeater {
            current_new_row = false;
        }
    }
    (instance_count, step as u32)
}

/// A `GridLayoutInputData` struct with auto row/col and unit span — matches
/// `GridLayoutInputData::default()` in `i_slint_core::layout`.
fn auto_grid_input_data() -> Value {
    let mut s = crate::api::Struct::default();
    s.set_field("new_row".into(), Value::Bool(false));
    s.set_field("row".into(), Value::Number(i_slint_common::ROW_COL_AUTO as f64));
    s.set_field("col".into(), Value::Number(i_slint_common::ROW_COL_AUTO as f64));
    s.set_field("rowspan".into(), Value::Number(1.0));
    s.set_field("colspan".into(), Value::Number(1.0));
    Value::Struct(s)
}

fn override_new_row(v: &mut Value, new_row: bool) {
    if let Value::Struct(s) = v {
        s.set_field("new_row".into(), Value::Bool(new_row));
    }
}

fn model_from_vec(values: Vec<Value>) -> ModelRc<Value> {
    ModelRc::new(SharedVectorModel::from(values.into_iter().collect::<SharedVector<_>>()))
}

fn binary_op(op: char, lhs: Value, rhs: Value) -> Value {
    // Coerce a `Void` operand to the type-default of the other side so we
    // don't panic on uninitialized property reads.
    let (lhs, rhs) = match (lhs, rhs) {
        (Value::Void, Value::Number(b)) => (Value::Number(0.), Value::Number(b)),
        (Value::Number(a), Value::Void) => (Value::Number(a), Value::Number(0.)),
        (Value::Void, Value::Bool(b)) => (Value::Bool(false), Value::Bool(b)),
        (Value::Bool(a), Value::Void) => (Value::Bool(a), Value::Bool(false)),
        (Value::Void, Value::String(b)) => (Value::String(Default::default()), Value::String(b)),
        (Value::String(a), Value::Void) => (Value::String(a), Value::String(Default::default())),
        (a, b) => (a, b),
    };
    match (op, lhs, rhs) {
        ('+', Value::String(mut a), Value::String(b)) => {
            a.push_str(b.as_str());
            Value::String(a)
        }
        ('+', Value::Number(a), Value::Number(b)) => Value::Number(a + b),
        ('+', a @ Value::Struct(_), b @ Value::Struct(_)) => {
            let la: Option<i_slint_core::layout::LayoutInfo> = a.try_into().ok();
            let lb: Option<i_slint_core::layout::LayoutInfo> = b.try_into().ok();
            if let (Some(a), Some(b)) = (la, lb) {
                a.merge(&b).into()
            } else {
                panic!("unsupported struct + struct");
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
        (op, a, b) => panic!("unsupported {a:?} {op} {b:?}"),
    }
}

fn eval_stops(ctx: &mut EvalContext, stops: &[(Expression, Expression)]) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|(color, stop)| GradientStop {
            color: eval_expression(ctx, color).try_into().unwrap_or_default(),
            position: eval_expression(ctx, stop).try_into().unwrap_or_default(),
        })
        .collect()
}

fn load_image_reference(
    resource_ref: &i_slint_compiler::expression_tree::ImageReference,
) -> i_slint_core::graphics::Image {
    use i_slint_compiler::expression_tree::ImageReference as Ref;
    match resource_ref {
        Ref::None => Default::default(),
        Ref::AbsolutePath(path) => {
            if path.starts_with("data:") {
                i_slint_compiler::data_uri::decode_data_uri(path)
                    .map(|(data, extension)| {
                        let data: &'static [u8] = Box::leak(data.into_boxed_slice());
                        let ext: &'static [u8] =
                            Box::leak(extension.into_boxed_str().into_boxed_bytes());
                        i_slint_core::graphics::load_image_from_embedded_data(
                            i_slint_core::slice::Slice::from_slice(data),
                            i_slint_core::slice::Slice::from_slice(ext),
                        )
                    })
                    .unwrap_or_default()
            } else if path.starts_with("builtin:/") {
                // Style-bundled resources (e.g. cosmic/material widget icons)
                // are baked into the compiler's builtin library and need to be
                // fetched through `fileaccess::load_file` rather than the
                // filesystem. The bytes are static, so leak them so the core
                // image loader can hold a reference for the lifetime of the
                // process.
                let virtual_file =
                    i_slint_compiler::fileaccess::load_file(std::path::Path::new(path.as_str()));
                let Some(virtual_file) = virtual_file else { return Default::default() };
                let Some(contents) = virtual_file.builtin_contents else {
                    return Default::default();
                };
                let extension = std::path::Path::new(path.as_str())
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .as_bytes();
                let ext: &'static [u8] = Box::leak(extension.to_vec().into_boxed_slice());
                i_slint_core::graphics::load_image_from_embedded_data(
                    i_slint_core::slice::Slice::from_slice(contents),
                    i_slint_core::slice::Slice::from_slice(ext),
                )
            } else {
                i_slint_core::graphics::Image::load_from_path(std::path::Path::new(path))
                    .unwrap_or_default()
            }
        }
        Ref::EmbeddedData { .. } | Ref::EmbeddedTexture { .. } => Default::default(),
    }
}

fn layout_cache_access(
    ctx: &mut EvalContext,
    cache: Value,
    index: usize,
    repeater_index: Option<&Expression>,
    entries_per_item: usize,
) -> Value {
    match cache {
        Value::LayoutCache(cache) => {
            if let Some(ri) = repeater_index {
                let offset: usize = eval_expression(ctx, ri).try_into().unwrap_or_default();
                Value::Number(
                    cache
                        .get((cache[index] as usize) + offset * entries_per_item)
                        .copied()
                        .unwrap_or(0.)
                        .into(),
                )
            } else {
                Value::Number(cache[index].into())
            }
        }
        Value::ArrayOfU16(cache) => {
            if let Some(ri) = repeater_index {
                let offset: usize = eval_expression(ctx, ri).try_into().unwrap_or_default();
                Value::Number(
                    cache
                        .get((cache[index] as usize) + offset * entries_per_item)
                        .copied()
                        .unwrap_or(0)
                        .into(),
                )
            } else {
                Value::Number(cache[index].into())
            }
        }
        _ => Value::Number(0.),
    }
}

/// Two-level indirection cache read for grid layouts with repeaters.
/// `base = cache[index]` points at the start of a repeated row's entries;
/// the final index offsets from there by `repeater_index * stride`, a
/// per-cell `child_offset`, and an optional inner-repeater offset.
fn grid_repeater_cache_access(
    cache: Value,
    index: usize,
    repeater_index: usize,
    stride: usize,
    child_offset: usize,
    inner_offset: usize,
) -> Value {
    let get = |data_idx: usize, slice_len: usize, read: &dyn Fn(usize) -> f64| {
        if data_idx < slice_len { Value::Number(read(data_idx)) } else { Value::Number(0.) }
    };
    match cache {
        Value::LayoutCache(cache) => {
            let base = cache.get(index).copied().unwrap_or(0.) as usize;
            let data_idx = base + repeater_index * stride + child_offset + inner_offset;
            get(data_idx, cache.len(), &|i| cache[i] as f64)
        }
        Value::ArrayOfU16(cache) => {
            let base = cache.get(index).copied().unwrap_or(0) as usize;
            let data_idx = base + repeater_index * stride + child_offset + inner_offset;
            get(data_idx, cache.len(), &|i| cache[i] as f64)
        }
        _ => Value::Number(0.),
    }
}

/// Dispatch table for the computational `BuiltinFunction` variants.
///
/// Dispatch a `BuiltinFunction` call to the corresponding runtime helper.
fn call_builtin_function(
    ctx: &mut EvalContext,
    f: BuiltinFunction,
    arguments: &[Expression],
) -> Value {
    let to_num = |ctx: &mut EvalContext, e: &Expression| -> f64 {
        eval_expression(ctx, e).try_into().unwrap_or_default()
    };
    let to_string = |ctx: &mut EvalContext, e: &Expression| -> SharedString {
        eval_expression(ctx, e).try_into().unwrap_or_default()
    };

    match f {
        BuiltinFunction::Mod => {
            Value::Number(to_num(ctx, &arguments[0]).rem_euclid(to_num(ctx, &arguments[1])))
        }
        BuiltinFunction::Round => Value::Number(to_num(ctx, &arguments[0]).round()),
        BuiltinFunction::Ceil => Value::Number(to_num(ctx, &arguments[0]).ceil()),
        BuiltinFunction::Floor => Value::Number(to_num(ctx, &arguments[0]).floor()),
        BuiltinFunction::Sqrt => Value::Number(to_num(ctx, &arguments[0]).sqrt()),
        BuiltinFunction::Abs => Value::Number(to_num(ctx, &arguments[0]).abs()),
        BuiltinFunction::Sin => Value::Number(to_num(ctx, &arguments[0]).to_radians().sin()),
        BuiltinFunction::Cos => Value::Number(to_num(ctx, &arguments[0]).to_radians().cos()),
        BuiltinFunction::Tan => Value::Number(to_num(ctx, &arguments[0]).to_radians().tan()),
        BuiltinFunction::ASin => Value::Number(to_num(ctx, &arguments[0]).asin().to_degrees()),
        BuiltinFunction::ACos => Value::Number(to_num(ctx, &arguments[0]).acos().to_degrees()),
        BuiltinFunction::ATan => Value::Number(to_num(ctx, &arguments[0]).atan().to_degrees()),
        BuiltinFunction::ATan2 => {
            Value::Number(to_num(ctx, &arguments[0]).atan2(to_num(ctx, &arguments[1])).to_degrees())
        }
        BuiltinFunction::Log => {
            Value::Number(to_num(ctx, &arguments[0]).log(to_num(ctx, &arguments[1])))
        }
        BuiltinFunction::Ln => Value::Number(to_num(ctx, &arguments[0]).ln()),
        BuiltinFunction::Pow => {
            Value::Number(to_num(ctx, &arguments[0]).powf(to_num(ctx, &arguments[1])))
        }
        BuiltinFunction::Exp => Value::Number(to_num(ctx, &arguments[0]).exp()),
        BuiltinFunction::ToFixed => {
            let n = to_num(ctx, &arguments[0]);
            let digits: i32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or_default();
            Value::String(i_slint_core::string::shared_string_from_number_fixed(
                n,
                digits.max(0) as usize,
            ))
        }
        BuiltinFunction::ToPrecision => {
            let n = to_num(ctx, &arguments[0]);
            let p: i32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or_default();
            Value::String(i_slint_core::string::shared_string_from_number_precision(
                n,
                p.max(0) as usize,
            ))
        }
        BuiltinFunction::StringIsFloat => Value::Bool(
            <f64 as core::str::FromStr>::from_str(to_string(ctx, &arguments[0]).as_str()).is_ok(),
        ),
        BuiltinFunction::StringToFloat => Value::Number(
            core::str::FromStr::from_str(to_string(ctx, &arguments[0]).as_str()).unwrap_or(0.),
        ),
        BuiltinFunction::StringIsEmpty => Value::Bool(to_string(ctx, &arguments[0]).is_empty()),
        BuiltinFunction::StringCharacterCount => Value::Number(
            unicode_segmentation::UnicodeSegmentation::graphemes(
                to_string(ctx, &arguments[0]).as_str(),
                true,
            )
            .count() as f64,
        ),
        BuiltinFunction::StringToLowercase => {
            Value::String(to_string(ctx, &arguments[0]).to_lowercase().into())
        }
        BuiltinFunction::StringToUppercase => {
            Value::String(to_string(ctx, &arguments[0]).to_uppercase().into())
        }
        BuiltinFunction::ColorRgbaStruct => {
            if let Value::Brush(brush) = eval_expression(ctx, &arguments[0]) {
                let color = brush.color();
                let values = [
                    ("red".to_string(), Value::Number(color.red().into())),
                    ("green".to_string(), Value::Number(color.green().into())),
                    ("blue".to_string(), Value::Number(color.blue().into())),
                    ("alpha".to_string(), Value::Number(color.alpha().into())),
                ]
                .into_iter()
                .collect();
                Value::Struct(values)
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorHsvaStruct => {
            if let Value::Brush(brush) = eval_expression(ctx, &arguments[0]) {
                let color = brush.color().to_hsva();
                let values = [
                    ("hue".to_string(), Value::Number(color.hue.into())),
                    ("saturation".to_string(), Value::Number(color.saturation.into())),
                    ("value".to_string(), Value::Number(color.value.into())),
                    ("alpha".to_string(), Value::Number(color.alpha.into())),
                ]
                .into_iter()
                .collect();
                Value::Struct(values)
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorOklchStruct => {
            if let Value::Brush(brush) = eval_expression(ctx, &arguments[0]) {
                let color = brush.color().to_oklch();
                let values = [
                    ("lightness".to_string(), Value::Number(color.lightness.into())),
                    ("chroma".to_string(), Value::Number(color.chroma.into())),
                    ("hue".to_string(), Value::Number(color.hue.into())),
                    ("alpha".to_string(), Value::Number(color.alpha.into())),
                ]
                .into_iter()
                .collect();
                Value::Struct(values)
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorBrighter => {
            if let Value::Brush(brush) = eval_expression(ctx, &arguments[0]) {
                brush.brighter(to_num(ctx, &arguments[1]) as f32).into()
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorDarker => {
            if let Value::Brush(brush) = eval_expression(ctx, &arguments[0]) {
                brush.darker(to_num(ctx, &arguments[1]) as f32).into()
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorTransparentize => {
            if let Value::Brush(brush) = eval_expression(ctx, &arguments[0]) {
                brush.transparentize(to_num(ctx, &arguments[1]) as f32).into()
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorWithAlpha => {
            if let Value::Brush(brush) = eval_expression(ctx, &arguments[0]) {
                brush.with_alpha(to_num(ctx, &arguments[1]) as f32).into()
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ColorMix => {
            let a = eval_expression(ctx, &arguments[0]);
            let b = eval_expression(ctx, &arguments[1]);
            let factor = to_num(ctx, &arguments[2]) as f32;
            if let (
                Value::Brush(i_slint_core::Brush::SolidColor(ca)),
                Value::Brush(i_slint_core::Brush::SolidColor(cb)),
            ) = (a, b)
            {
                ca.mix(&cb, factor).into()
            } else {
                Value::Void
            }
        }
        BuiltinFunction::Rgb => {
            let r: i32 = eval_expression(ctx, &arguments[0]).try_into().unwrap_or(0);
            let g: i32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or(0);
            let b: i32 = eval_expression(ctx, &arguments[2]).try_into().unwrap_or(0);
            let a: f32 = eval_expression(ctx, &arguments[3]).try_into().unwrap_or(1.0);
            let r: u8 = r.clamp(0, 255) as u8;
            let g: u8 = g.clamp(0, 255) as u8;
            let b: u8 = b.clamp(0, 255) as u8;
            let a: u8 = (255. * a).clamp(0., 255.) as u8;
            Value::Brush(i_slint_core::Brush::SolidColor(i_slint_core::Color::from_argb_u8(
                a, r, g, b,
            )))
        }
        BuiltinFunction::Hsv => {
            let h: f32 = eval_expression(ctx, &arguments[0]).try_into().unwrap_or(0.0);
            let s: f32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or(0.0);
            let v: f32 = eval_expression(ctx, &arguments[2]).try_into().unwrap_or(0.0);
            let a: f32 = eval_expression(ctx, &arguments[3]).try_into().unwrap_or(1.0);
            let a = a.clamp(0., 1.);
            Value::Brush(i_slint_core::Brush::SolidColor(i_slint_core::Color::from_hsva(
                h, s, v, a,
            )))
        }
        BuiltinFunction::Oklch => {
            let l: f32 = eval_expression(ctx, &arguments[0]).try_into().unwrap_or(0.0);
            let c: f32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or(0.0);
            let h: f32 = eval_expression(ctx, &arguments[2]).try_into().unwrap_or(0.0);
            let a: f32 = eval_expression(ctx, &arguments[3]).try_into().unwrap_or(1.0);
            Value::Brush(i_slint_core::Brush::SolidColor(i_slint_core::Color::from_oklch(
                l.clamp(0.0, 1.0),
                c,
                h,
                a.clamp(0.0, 1.0),
            )))
        }
        BuiltinFunction::AnimationTick => {
            Value::Number(i_slint_core::animations::animation_tick() as f64)
        }
        BuiltinFunction::GetWindowScaleFactor => {
            let factor = ctx
                .current
                .as_ref()
                .and_then(|c| c.root.get())
                .and_then(|w| w.upgrade())
                .and_then(|inst| inst.window_adapter_or_default())
                .map(|adapter| {
                    i_slint_core::window::WindowInner::from_pub(adapter.window()).scale_factor()
                        as f64
                })
                .unwrap_or(1.0);
            Value::Number(factor)
        }
        BuiltinFunction::GetWindowDefaultFontSize => {
            // Default font size lives on the window-item's
            // `default-font-size` property; pull it via the window adapter
            // and fall back to 12px (headless testing default) if no window
            // is set up yet. For popups the local instance has no window
            // adapter of its own, so walk the parent chain to reach the
            // owning window — otherwise `1rem` would resolve to the headless
            // default instead of inheriting the parent window's setting.
            let size = find_window_adapter(ctx)
                .map(|adapter| {
                    let win = i_slint_core::window::WindowInner::from_pub(adapter.window());
                    win.window_item()
                        .map(|wi| wi.as_pin_ref().default_font_size().get() as f64)
                        .unwrap_or(12.0)
                })
                .unwrap_or(12.0);
            Value::Number(size)
        }
        BuiltinFunction::DetectOperatingSystem => i_slint_core::detect_operating_system().into(),
        BuiltinFunction::Use24HourFormat => {
            Value::Bool(i_slint_core::date_time::use_24_hour_format())
        }
        BuiltinFunction::ColorScheme => {
            // Mirror the rust codegen: ask the active window adapter for the
            // current color scheme. Walk up to the root instance to find the
            // adapter; fall back to `Unknown` if there is no window yet.
            let scheme = ctx
                .current
                .as_ref()
                .and_then(|c| c.root.get())
                .and_then(|w| w.upgrade())
                .and_then(|inst| inst.window_adapter_or_default())
                .map(|adapter| {
                    i_slint_core::window::WindowInner::from_pub(adapter.window()).color_scheme()
                })
                .unwrap_or(i_slint_core::items::ColorScheme::Unknown);
            scheme.into()
        }
        BuiltinFunction::AccentColor => {
            let color = ctx
                .current
                .as_ref()
                .and_then(|c| c.root.get())
                .and_then(|w| w.upgrade())
                .and_then(|inst| inst.window_adapter_or_default())
                .map(|adapter| {
                    i_slint_core::window::WindowInner::from_pub(adapter.window()).accent_color()
                })
                .unwrap_or_default();
            color.into()
        }
        BuiltinFunction::SupportsNativeMenuBar => {
            let supports = find_window_adapter(ctx).is_some_and(|a| {
                a.internal(i_slint_core::InternalToken)
                    .is_some_and(|x| x.supports_native_menu_bar())
            });
            Value::Bool(supports)
        }
        BuiltinFunction::TextInputFocused => {
            let focused = ctx
                .current
                .as_ref()
                .and_then(|c| c.root.get())
                .and_then(|w| w.upgrade())
                .and_then(|inst| inst.window_adapter_or_default())
                .map(|adapter| {
                    i_slint_core::window::WindowInner::from_pub(adapter.window())
                        .text_input_focused()
                })
                .unwrap_or(false);
            Value::Bool(focused)
        }
        BuiltinFunction::SetTextInputFocused => {
            let value = arguments
                .first()
                .map(|e| eval_expression(ctx, e))
                .and_then(|v| bool::try_from(v).ok())
                .unwrap_or(false);
            if let Some(adapter) = ctx
                .current
                .as_ref()
                .and_then(|c| c.root.get())
                .and_then(|w| w.upgrade())
                .and_then(|inst| inst.window_adapter_or_default())
            {
                i_slint_core::window::WindowInner::from_pub(adapter.window())
                    .set_text_input_focused(value);
            }
            Value::Void
        }
        BuiltinFunction::UpdateTimers => {
            // The interpreter installs change trackers on every timer's
            // `running` / `interval` expressions in `bindings::install_timers`,
            // so they react to property changes automatically. The
            // `UpdateTimers` builtin is the rust codegen's way of triggering
            // that work explicitly; for the interpreter it's a no-op.
            Value::Void
        }
        BuiltinFunction::RestartTimer => {
            // Argument is `NumberLiteral(timer_index)` rooted in the current
            // sub-component.
            if let [Expression::NumberLiteral(timer_index)] = arguments
                && let Some(current) = ctx.current.as_ref()
                && let Some(timer) = current.timers.borrow().get(*timer_index as usize)
            {
                timer.restart();
            }
            Value::Void
        }
        BuiltinFunction::KeysToString => {
            let v = arguments.first().map(|e| eval_expression(ctx, e));
            if let Some(Value::Keys(keys)) = v {
                Value::String(keys.to_string().into())
            } else {
                Value::String(Default::default())
            }
        }
        BuiltinFunction::SetSelectionOffsets => {
            // (item_ref, start, end) — applied to a TextInput.
            use i_slint_core::items::TextInput;
            let [Expression::PropertyReference(mr), start_expr, end_expr] = arguments else {
                return Value::Void;
            };
            let start: i32 = eval_expression(ctx, start_expr).try_into().unwrap_or(0);
            let end: i32 = eval_expression(ctx, end_expr).try_into().unwrap_or(0);
            let Some((parent_inst, flat_idx)) = resolve_item_rc_from_ref(ctx, mr) else {
                return Value::Void;
            };
            let Some(adapter) = parent_inst.window_adapter_or_default() else {
                return Value::Void;
            };
            let parent_dyn = vtable::VRc::into_dyn(parent_inst);
            let item_rc = i_slint_core::items::ItemRc::new(parent_dyn, flat_idx as u32);
            if let Some(text_input) = vtable::VRef::downcast_pin::<TextInput>(item_rc.borrow()) {
                text_input.set_selection_offsets(&adapter, &item_rc, start, end);
            }
            Value::Void
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            // Font registration is a no-op in the headless test backend.
            // The rust codegen wires this through
            // `slint::private_unstable_api::register_font_from_path`, which
            // returns `Ok(())` for the testing backend anyway.
            Value::Void
        }
        BuiltinFunction::SetupMenuBar => setup_menubar(ctx, arguments),
        BuiltinFunction::ItemFontMetrics => {
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let MemberReference::Relative { parent_level, local_reference } = mr
                && let LocalMemberIndex::Native { item_index, prop_name, .. } =
                    &local_reference.reference
                && prop_name.is_empty()
            {
                let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
                let item = &owner.items[*item_index];
                let item_ref = Pin::as_ref(item).as_item_ref();
                if let Some(parent_inst) = owner.root.get().and_then(|w| w.upgrade())
                    && let Some(adapter) = parent_inst.window_adapter_or_default()
                {
                    let parent_dyn = vtable::VRc::into_dyn(parent_inst);
                    let item_rc = i_slint_core::items::ItemRc::new(
                        parent_dyn,
                        usize::from(*item_index) as u32,
                    );
                    let metrics = i_slint_core::items::slint_text_item_fontmetrics(
                        &adapter, item_ref, &item_rc,
                    );
                    return metrics.into();
                }
            }
            i_slint_core::items::FontMetrics::default().into()
        }
        BuiltinFunction::ItemAbsolutePosition => {
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let MemberReference::Relative { parent_level, local_reference } = mr
                && let LocalMemberIndex::Native { item_index, prop_name, .. } =
                    &local_reference.reference
                && prop_name.is_empty()
            {
                let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
                if let Some(parent_inst) = owner.root.get().and_then(|w| w.upgrade()) {
                    // The sub_component_path on `local_reference` is relative
                    // to the caller's current sub-component. The item_table
                    // stores paths rooted at the owning `Instance`. Build the
                    // absolute path by walking from the instance root down to
                    // the owning SubComponentInstance.
                    let full_path =
                        crate::item_tree_vtable::sub_component_path_of(&owner, &parent_inst);
                    let flat_idx =
                        find_flat_item_index(&parent_inst.item_table, &full_path, *item_index);
                    if let Some(flat_idx) = flat_idx {
                        let parent_dyn = vtable::VRc::into_dyn(parent_inst);
                        let item_rc = i_slint_core::items::ItemRc::new(parent_dyn, flat_idx as u32);
                        return item_rc.map_to_window(Default::default()).to_untyped().into();
                    }
                }
            }
            i_slint_core::api::LogicalPosition::default().into()
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            // The argument is a `PropertyReference` to a `Native { prop_name: "" }`,
            // i.e. the item itself. Resolve to the `ErasedItemRc` and call
            // its `ItemVTable::layout_info` directly.
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let MemberReference::Relative { parent_level, local_reference } = mr
                && let LocalMemberIndex::Native { item_index, prop_name, .. } =
                    &local_reference.reference
                && prop_name.is_empty()
            {
                let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
                let item = &owner.items[*item_index];
                let item_ref = Pin::as_ref(item).as_item_ref();
                let orient = match orient {
                    i_slint_compiler::layout::Orientation::Horizontal => {
                        i_slint_core::items::Orientation::Horizontal
                    }
                    i_slint_compiler::layout::Orientation::Vertical => {
                        i_slint_core::items::Orientation::Vertical
                    }
                };
                let parent_inst = owner.root.get().and_then(|w| w.upgrade());
                if let Some(parent_inst) = parent_inst
                    && let Some(adapter) = parent_inst.window_adapter_or_default()
                {
                    let parent_dyn = vtable::VRc::into_dyn(parent_inst);
                    let item_rc = i_slint_core::items::ItemRc::new(
                        parent_dyn,
                        usize::from(*item_index) as u32,
                    );
                    return item_ref.as_ref().layout_info(orient, &adapter, &item_rc).into();
                }
                i_slint_core::layout::LayoutInfo::default().into()
            } else {
                i_slint_core::layout::LayoutInfo::default().into()
            }
        }
        BuiltinFunction::Debug => {
            let msg = to_string(ctx, &arguments[0]);
            let handler = ctx
                .current
                .as_ref()
                .and_then(|c| c.root.get())
                .and_then(|w| w.upgrade())
                .and_then(|inst| inst.debug_handler.borrow().clone());
            if let Some(handler) = handler {
                handler(None, msg.as_str());
            } else {
                eprintln!("{msg}");
            }
            Value::Void
        }
        BuiltinFunction::ArrayLength => match eval_expression(ctx, &arguments[0]) {
            // Register a dependency on the model's row count so that
            // bindings reading `.length` re-evaluate when rows are added
            // or removed. Without this, `length` reads the current count
            // once and never refreshes.
            Value::Model(m) => {
                m.model_tracker().track_row_count_changes();
                Value::Number(m.row_count() as f64)
            }
            _ => Value::Number(0.),
        },
        BuiltinFunction::ImageSize => {
            if let Value::Image(img) = eval_expression(ctx, &arguments[0]) {
                let size = img.size();
                let mut s = crate::api::Struct::default();
                s.set_field("width".to_string(), Value::Number(size.width as f64));
                s.set_field("height".to_string(), Value::Number(size.height as f64));
                Value::Struct(s)
            } else {
                Value::Void
            }
        }
        BuiltinFunction::ParseMarkdown => {
            let format_string: SharedString =
                eval_expression(ctx, &arguments[0]).try_into().unwrap_or_default();
            let args = eval_expression(ctx, &arguments[1]);
            let args: Vec<i_slint_core::styled_text::StyledText> = if let Value::Model(m) = args {
                (0..m.row_count())
                    .filter_map(|i| match m.row_data(i)? {
                        Value::StyledText(t) => Some(t),
                        _ => None,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Value::StyledText(i_slint_core::styled_text::parse_markdown(&format_string, &args))
        }
        BuiltinFunction::StringToStyledText => {
            let string: SharedString =
                eval_expression(ctx, &arguments[0]).try_into().unwrap_or_default();
            Value::StyledText(i_slint_core::styled_text::string_to_styled_text(string.to_string()))
        }
        BuiltinFunction::Translate => {
            let original: SharedString = to_string(ctx, &arguments[0]);
            let context: SharedString = to_string(ctx, &arguments[1]);
            let domain: SharedString = to_string(ctx, &arguments[2]);
            let args = eval_expression(ctx, &arguments[3]);
            let Value::Model(args) = args else {
                return Value::String(original);
            };
            struct StringModelWrapper(ModelRc<Value>);
            impl i_slint_core::translations::FormatArgs for StringModelWrapper {
                type Output<'a> = SharedString;
                fn from_index(&self, index: usize) -> Option<SharedString> {
                    self.0.row_data(index).and_then(|v| v.try_into().ok())
                }
            }
            let n: i32 = eval_expression(ctx, &arguments[4]).try_into().unwrap_or(0);
            let plural: SharedString = to_string(ctx, &arguments[5]);
            Value::String(i_slint_core::translations::translate(
                &original,
                &context,
                &domain,
                &StringModelWrapper(args),
                n,
                &plural,
            ))
        }
        BuiltinFunction::ShowPopupWindow => show_popup_window(ctx, arguments),
        BuiltinFunction::ClosePopupWindow => close_popup_window(ctx, arguments),
        BuiltinFunction::SetFocusItem => {
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let Some((inst, flat_idx)) = resolve_item_rc_from_ref(ctx, mr)
                && let Some(adapter) = find_window_adapter(ctx)
            {
                let dyn_rc = vtable::VRc::into_dyn(inst);
                let item_rc = i_slint_core::items::ItemRc::new(dyn_rc, flat_idx as u32);
                i_slint_core::window::WindowInner::from_pub(adapter.window()).set_focus_item(
                    &item_rc,
                    true,
                    i_slint_core::input::FocusReason::Programmatic,
                );
            }
            Value::Void
        }
        BuiltinFunction::ClearFocusItem => {
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let Some((inst, flat_idx)) = resolve_item_rc_from_ref(ctx, mr)
                && let Some(adapter) = find_window_adapter(ctx)
            {
                let dyn_rc = vtable::VRc::into_dyn(inst);
                let item_rc = i_slint_core::items::ItemRc::new(dyn_rc, flat_idx as u32);
                i_slint_core::window::WindowInner::from_pub(adapter.window()).set_focus_item(
                    &item_rc,
                    false,
                    i_slint_core::input::FocusReason::Programmatic,
                );
            }
            Value::Void
        }
        BuiltinFunction::MonthDayCount => {
            let m: u32 = eval_expression(ctx, &arguments[0]).try_into().unwrap_or(0);
            let y: i32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or(0);
            Value::Number(i_slint_core::date_time::month_day_count(m, y).unwrap_or(0) as f64)
        }
        BuiltinFunction::MonthOffset => {
            let m: u32 = eval_expression(ctx, &arguments[0]).try_into().unwrap_or(0);
            let y: i32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or(0);
            Value::Number(i_slint_core::date_time::month_offset(m, y) as f64)
        }
        BuiltinFunction::FormatDate => {
            let f: SharedString = to_string(ctx, &arguments[0]);
            let d: u32 = eval_expression(ctx, &arguments[1]).try_into().unwrap_or(0);
            let m: u32 = eval_expression(ctx, &arguments[2]).try_into().unwrap_or(0);
            let y: i32 = eval_expression(ctx, &arguments[3]).try_into().unwrap_or(0);
            Value::String(i_slint_core::date_time::format_date(&f, d, m, y))
        }
        BuiltinFunction::DateNow => {
            Value::Model(i_slint_core::model::ModelRc::new(i_slint_core::model::VecModel::from(
                i_slint_core::date_time::date_now()
                    .into_iter()
                    .map(|x| Value::Number(x as f64))
                    .collect::<Vec<_>>(),
            )))
        }
        BuiltinFunction::ValidDate => {
            let d: SharedString = to_string(ctx, &arguments[0]);
            let f: SharedString = to_string(ctx, &arguments[1]);
            Value::Bool(i_slint_core::date_time::parse_date(d.as_str(), f.as_str()).is_some())
        }
        BuiltinFunction::ParseDate => {
            let d: SharedString = to_string(ctx, &arguments[0]);
            let f: SharedString = to_string(ctx, &arguments[1]);
            Value::Model(i_slint_core::model::ModelRc::new(i_slint_core::model::VecModel::from(
                i_slint_core::date_time::parse_date(d.as_str(), f.as_str())
                    .map(|v| v.into_iter().map(|x| Value::Number(x as f64)).collect::<Vec<_>>())
                    .unwrap_or_default(),
            )))
        }
        BuiltinFunction::ShowPopupMenu | BuiltinFunction::ShowPopupMenuInternal => {
            show_popup_menu(ctx, arguments)
        }
        BuiltinFunction::OpenUrl => {
            let url = to_string(ctx, &arguments[0]);
            let result = find_window_adapter(ctx)
                .map(|adapter| i_slint_core::open_url(&url, adapter.window()).is_ok())
                .unwrap_or(false);
            Value::Bool(result)
        }
        BuiltinFunction::RegisterCustomFontByMemory | BuiltinFunction::RegisterBitmapFont => {
            // Bitmap font registration is generated by build.rs, not callable from .slint.
            Value::Void
        }
        BuiltinFunction::StartTimer | BuiltinFunction::StopTimer => {
            // Lowered into property assignments by `materialize_state`; never reached.
            Value::Void
        }
    }
}

/// Resolve an `Expression::PropertyReference` that targets a native item
/// (a `LocalMemberIndex::Native { prop_name: "" }`) into the owning
/// `Instance` and the flat tree index of that item within it. Used by the
/// focus / `ItemMemberFunctionCall` builtins which need a runtime
/// `ItemRc` to hand to core APIs.
fn resolve_item_rc_from_ref(
    ctx: &EvalContext,
    mr: &MemberReference,
) -> Option<(vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, crate::instance::Instance>, usize)>
{
    let MemberReference::Relative { parent_level, local_reference } = mr else { return None };
    let LocalMemberIndex::Native { item_index, .. } = &local_reference.reference else {
        return None;
    };
    let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
    let parent_inst = owner.root.get().and_then(|w| w.upgrade())?;
    let full_path = crate::item_tree_vtable::sub_component_path_of(&owner, &parent_inst);
    let flat_idx = find_flat_item_index(&parent_inst.item_table, &full_path, *item_index)?;
    Some((parent_inst, flat_idx))
}

/// Walk up the parent chain from the current context to find the root
/// Instance's window adapter. Used by `SetFocusItem` / `ClearFocusItem`
/// which may execute inside a repeated or conditional sub-tree that
/// doesn't have its own window adapter.
fn find_window_adapter(ctx: &EvalContext) -> Option<i_slint_core::window::WindowAdapterRc> {
    let current = ctx.current.as_ref()?;
    // Walk up the parent chain to find the root Instance.
    let mut sub = current.clone();
    loop {
        if let Some(root) = sub.root.get()
            && let Some(inst) = root.upgrade()
            && inst.public_component_index.is_some()
        {
            return inst.window_adapter_or_default();
        }
        let parent = sub.parent.upgrade()?;
        sub = Pin::new(parent);
    }
}

/// Dispatch a `Expression::ItemMemberFunctionCall` to the matching native
/// item method. The LLR's `function` reference carries the item slot and a
/// `prop_name`; we downcast the runtime `ItemRc`'s concrete item type and
/// call the method directly — the same work the rust codegen does
/// statically, just moved to runtime.
fn call_item_member_function(ctx: &EvalContext, function: &MemberReference) -> Value {
    use i_slint_core::items::{ContextMenu, SwipeGestureHandler, TextInput, WindowItem};
    let MemberReference::Relative { local_reference, .. } = function else {
        return Value::Void;
    };
    let LocalMemberIndex::Native { prop_name, .. } = &local_reference.reference else {
        return Value::Void;
    };
    let Some((parent_inst, flat_idx)) = resolve_item_rc_from_ref(ctx, function) else {
        return Value::Void;
    };
    let Some(adapter) = parent_inst.window_adapter_or_default() else { return Value::Void };
    let parent_dyn = vtable::VRc::into_dyn(parent_inst);
    let item_rc = i_slint_core::items::ItemRc::new(parent_dyn, flat_idx as u32);
    let item_ref = item_rc.borrow();

    // Map a Slint-side member-function name to the matching Rust method on
    // a downcast item type.
    macro_rules! dispatch {
        ($item:expr, $name:expr; $($slint_name:literal => $rust_method:ident $(=> $into:ty)?),* $(,)?) => {
            match $name {
                $(
                    $slint_name => {
                        let res = $item.$rust_method(&adapter, &item_rc);
                        $(let res: $into = res.into();)?
                        return res.into();
                    }
                )*
                _ => {}
            }
        };
    }

    if let Some(text_input) = vtable::VRef::downcast_pin::<TextInput>(item_ref) {
        dispatch!(text_input, prop_name.as_str();
            "select-all" => select_all => (),
            "clear-selection" => clear_selection => (),
            "select-word" => select_word => (),
            "cut" => cut => (),
            "copy" => copy => (),
            "paste" => paste => (),
        );
    }
    if let Some(swipe) = vtable::VRef::downcast_pin::<SwipeGestureHandler>(item_rc.borrow()) {
        dispatch!(swipe, prop_name.as_str();
            "cancel" => cancel => (),
        );
    }
    if let Some(menu) = vtable::VRef::downcast_pin::<ContextMenu>(item_rc.borrow()) {
        dispatch!(menu, prop_name.as_str();
            "close" => close => (),
            "is-open" => is_open,
        );
    }
    if prop_name == "hide"
        && let Some(window) = vtable::VRef::downcast_pin::<WindowItem>(item_rc.borrow())
    {
        window.hide(&adapter);
        return Value::Void;
    }
    unimplemented!("ItemMemberFunctionCall `{prop_name}`")
}

fn show_popup_window(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    // Expected args (from `BuiltinFunction::ShowPopupWindow`):
    //   0: NumberLiteral(popup_index) into the owning sub-component's popup_windows
    //   1: close_policy expression
    //   2: PropertyReference to the parent item (relative, parent_level walks up)
    let [
        Expression::NumberLiteral(popup_index),
        close_policy_expr,
        Expression::PropertyReference(parent_ref),
    ] = arguments
    else {
        return Value::Void;
    };
    let MemberReference::Relative { parent_level, local_reference } = parent_ref else {
        return Value::Void;
    };
    let LocalMemberIndex::Native { item_index, .. } = &local_reference.reference else {
        return Value::Void;
    };

    let current = match ctx.current.as_ref() {
        Some(c) => c.clone(),
        None => return Value::Void,
    };
    let owner = walk_parent(&current, *parent_level);
    let cu = owner.compilation_unit.clone();
    let sc = &cu.sub_components[owner.sub_component_idx];
    let popup = match sc.popup_windows.get(*popup_index as usize) {
        Some(p) => p,
        None => return Value::Void,
    };

    // Build the popup Instance (no public_component_index — popups aren't
    // public components). The weak parent ties close_policy lifetime to
    // the owning sub-component.
    let parent_weak = std::rc::Rc::downgrade(&Pin::into_inner(owner.clone()));
    let globals = owner
        .root
        .get()
        .and_then(|w| w.upgrade())
        .map(|inst| inst.globals.clone())
        .unwrap_or_else(|| std::rc::Rc::new(crate::globals::GlobalStorage::new(&cu)));
    let popup_vrc =
        crate::instance::Instance::new_popup(cu.clone(), &popup.item_tree, parent_weak, globals);

    // `Value` has a generated `TryFrom<Value> for PopupClosePolicy` via
    // `declare_value_enum_conversion!` (which uses strum's
    // `serialize_all = "kebab-case"` `FromStr`), so the kebab-case
    // enumeration name from the `Value::EnumerationValue` parses directly.
    let close_policy: i_slint_core::items::PopupClosePolicy =
        eval_expression(ctx, close_policy_expr).try_into().unwrap_or_default();

    // Find the flat item index of the parent item so the window knows which
    // item owns the popup.
    let Some(parent_instance) = owner.root.get().and_then(|w| w.upgrade()) else {
        return Value::Void;
    };
    // Build the absolute path from the Instance root to the item. The
    // local_reference.sub_component_path is relative to the owning
    // sub-component; prepend the path from the Instance root down to that
    // sub-component so find_flat_item_index resolves correctly.
    let owner_path = crate::item_tree_vtable::sub_component_path_of(&owner, &parent_instance);
    let mut full_path = owner_path;
    full_path.extend_from_slice(&local_reference.sub_component_path);
    let parent_flat = find_flat_item_index(&parent_instance.item_table, &full_path, *item_index);
    let Some(parent_flat) = parent_flat else { return Value::Void };

    let parent_item_rc = i_slint_core::items::ItemRc::new(
        vtable::VRc::into_dyn(parent_instance.clone()),
        parent_flat as u32,
    );
    // The owning instance (`parent_instance`) may itself be a popup that
    // never had a window adapter assigned to it. Walk the parent chain so
    // a popup-in-popup is registered with the root window's adapter
    // instead of accidentally creating a fresh, headless one.
    let Some(adapter) = find_window_adapter(ctx) else {
        return Value::Void;
    };

    // Install bindings (including `x`/`y`) but defer `init_code` until after
    // `show_popup` so that `forward-focus` calls reach a popup that the
    // window adapter already considers active. The Rust codegen splits these
    // the same way (`new` then `show_popup` then `user_init`).
    crate::instance::install_bindings_for_repeated_row(&popup_vrc);

    // Evaluate the popup position expression (a LogicalPosition struct built
    // from the popup root's `x` and `y` properties).  Now that bindings are
    // installed the property reads return the correct values.
    let position: i_slint_core::api::LogicalPosition = {
        let pos_expr = popup.position.borrow().clone();
        let mut popup_ctx = EvalContext::new(popup_vrc.root_sub_component.clone());
        match eval_expression(&mut popup_ctx, &pos_expr) {
            Value::Struct(s) => {
                let x = s
                    .get_field("x")
                    .and_then(|v| if let Value::Number(n) = v { Some(*n as f32) } else { None })
                    .unwrap_or(0.0);
                let y = s
                    .get_field("y")
                    .and_then(|v| if let Value::Number(n) = v { Some(*n as f32) } else { None })
                    .unwrap_or(0.0);
                i_slint_core::api::LogicalPosition::new(x, y)
            }
            _ => i_slint_core::api::LogicalPosition::default(),
        }
    };
    let popup_dyn = vtable::VRc::into_dyn(popup_vrc.clone());
    let popup_id = i_slint_core::window::WindowInner::from_pub(adapter.window()).show_popup(
        &popup_dyn,
        position,
        close_policy,
        &parent_item_rc,
        false,
    );
    // Remember the id so `close_popup_window` can find it by popup index.
    {
        let mut ids = owner.popup_ids.borrow_mut();
        if (*popup_index as usize) < ids.len() {
            ids[*popup_index as usize] = Some(popup_id);
        }
    }
    // Run the popup's `init_code` now that the window adapter has the popup
    // registered, so `forward-focus` calls can target items in the popup.
    crate::instance::finalize_instance(&popup_vrc);
    Value::Void
}

fn close_popup_window(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    // Expected args (from `BuiltinFunction::ClosePopupWindow`):
    //   0: NumberLiteral(popup_index) into the owning sub-component's popup_windows
    //   1: PropertyReference to the parent item (used for its parent_level walk)
    let [Expression::NumberLiteral(popup_index), Expression::PropertyReference(parent_ref)] =
        arguments
    else {
        return Value::Void;
    };
    let MemberReference::Relative { parent_level, .. } = parent_ref else {
        return Value::Void;
    };
    let Some(current) = ctx.current.as_ref() else { return Value::Void };
    let owner = walk_parent(current, *parent_level);
    // Take the stored popup id, if any, and hand it to the window adapter.
    let id = {
        let mut ids = owner.popup_ids.borrow_mut();
        ids.get_mut(*popup_index as usize).and_then(|slot| slot.take())
    };
    if let Some(id) = id
        && let Some(root_inst) = owner.root.get().and_then(|w| w.upgrade())
        && let Some(adapter) = root_inst.window_adapter_or_default()
    {
        i_slint_core::window::WindowInner::from_pub(adapter.window()).close_popup(id);
    }
    Value::Void
}

fn setup_menubar(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    let (entries_ref, sub_menu_ref, activated_ref, tree_index, no_native, condition) =
        match arguments {
            [
                Expression::PropertyReference(e),
                Expression::PropertyReference(s),
                Expression::PropertyReference(a),
                Expression::NumberLiteral(idx),
                Expression::BoolLiteral(nn),
            ] => (e, s, a, *idx as usize, *nn, None),
            [
                Expression::PropertyReference(e),
                Expression::PropertyReference(s),
                Expression::PropertyReference(a),
                Expression::NumberLiteral(idx),
                Expression::BoolLiteral(nn),
                cond,
            ] => (e, s, a, *idx as usize, *nn, Some(cond)),
            _ => return Value::Void,
        };

    let current = match ctx.current.as_ref() {
        Some(c) => c.clone(),
        None => return Value::Void,
    };
    let cu = current.compilation_unit.clone();
    let sc = &cu.sub_components[current.sub_component_idx];
    let Some(menu_tree) = sc.menu_item_trees.get(tree_index) else {
        return Value::Void;
    };

    let parent_weak = std::rc::Rc::downgrade(&Pin::into_inner(current.clone()));
    let globals = current
        .root
        .get()
        .and_then(|w| w.upgrade())
        .map(|inst| inst.globals.clone())
        .unwrap_or_else(|| std::rc::Rc::new(crate::globals::GlobalStorage::new(&cu)));
    let menu_vrc =
        crate::instance::Instance::new_popup(cu.clone(), menu_tree, parent_weak, globals);
    crate::instance::finalize_instance(&menu_vrc);

    let menu_dyn = vtable::VRc::into_dyn(menu_vrc);
    let menu_item_tree = if let Some(cond_expr) = condition {
        let cond_expr = cond_expr.clone();
        let weak_current = std::rc::Rc::downgrade(&Pin::into_inner(current.clone()));
        vtable::VRc::new(i_slint_core::menus::MenuFromItemTree::new_with_condition(
            menu_dyn,
            move || {
                let Some(owner) = weak_current.upgrade() else { return false };
                let mut ctx = EvalContext::new(Pin::new(owner));
                matches!(eval_expression(&mut ctx, &cond_expr), Value::Bool(true))
            },
        ))
    } else {
        vtable::VRc::new(i_slint_core::menus::MenuFromItemTree::new(menu_dyn))
    };

    let Some(adapter) = find_window_adapter(ctx) else { return Value::Void };
    let window_inner = i_slint_core::window::WindowInner::from_pub(adapter.window());
    let menubar = vtable::VRc::into_dyn(vtable::VRc::clone(&menu_item_tree));
    window_inner.setup_menubar_shortcuts(vtable::VRc::clone(&menubar));

    if !no_native && window_inner.supports_native_menu_bar() {
        window_inner.setup_menubar(menubar);
        return Value::Void;
    }

    // Keep the menubar alive on the owning sub-component.
    *current.menubar.borrow_mut() = Some(menubar);

    // Wire up entries/sub_menu/activated for the fallback menu bar widget.
    let mt1 = vtable::VRc::clone(&menu_item_tree);
    let mt2 = vtable::VRc::clone(&menu_item_tree);
    let mt3 = menu_item_tree;

    if let MemberReference::Relative { parent_level, local_reference } = entries_ref {
        let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
        if let LocalMemberIndex::Property(idx) = &local_reference.reference {
            Pin::as_ref(&owner.properties[*idx]).set_binding(move || {
                let mut entries = i_slint_core::SharedVector::default();
                i_slint_core::menus::Menu::sub_menu(&*mt1, None.into(), &mut entries);
                Value::Model(i_slint_core::model::ModelRc::new(
                    i_slint_core::model::VecModel::from(
                        entries.into_iter().map(Value::from).collect::<Vec<_>>(),
                    ),
                ))
            });
        }
    }
    if let MemberReference::Relative { parent_level, local_reference } = sub_menu_ref {
        let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
        if let LocalMemberIndex::Callback(idx) = &local_reference.reference {
            Pin::as_ref(&owner.callbacks[*idx]).set_handler(
                move |(args,): &(Vec<Value>,)| -> Value {
                    let entry =
                        args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
                    let mut entries = i_slint_core::SharedVector::default();
                    i_slint_core::menus::Menu::sub_menu(&*mt2, Some(&entry).into(), &mut entries);
                    Value::Model(i_slint_core::model::ModelRc::new(
                        i_slint_core::model::VecModel::from(
                            entries.into_iter().map(Value::from).collect::<Vec<_>>(),
                        ),
                    ))
                },
            );
        }
    }
    if let MemberReference::Relative { parent_level, local_reference } = activated_ref {
        let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
        if let LocalMemberIndex::Callback(idx) = &local_reference.reference {
            Pin::as_ref(&owner.callbacks[*idx]).set_handler(
                move |(args,): &(Vec<Value>,)| -> Value {
                    let entry =
                        args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
                    i_slint_core::menus::Menu::activate(&*mt3, &entry);
                    Value::Void
                },
            );
        }
    }

    Value::Void
}

fn show_popup_menu(ctx: &mut EvalContext, arguments: &[Expression]) -> Value {
    let [Expression::PropertyReference(context_menu_ref), entries_expr, position_expr] = arguments
    else {
        return Value::Void;
    };

    let position: i_slint_core::api::LogicalPosition =
        eval_expression(ctx, position_expr).try_into().unwrap_or_default();

    let Some((parent_inst, context_flat_idx)) = resolve_item_rc_from_ref(ctx, context_menu_ref)
    else {
        return Value::Void;
    };
    let context_item_rc = i_slint_core::items::ItemRc::new(
        vtable::VRc::into_dyn(parent_inst.clone()),
        context_flat_idx as u32,
    );
    // The native `ContextMenu` item owns a `popup_id` Cell that the
    // generated `close()`/`is-open()` member functions read; mirror the
    // rust codegen and write it after `show_popup` returns so the
    // existing close/is_open paths keep working.
    let context_menu_item_weak = context_item_rc.downgrade();
    let Some(adapter) = find_window_adapter(ctx) else {
        return Value::Void;
    };

    let cu = ctx.current.as_ref().map(|c| c.compilation_unit.clone()).unwrap();
    let Some(popup_menu) = cu.popup_menu.as_ref() else {
        return Value::Void;
    };

    let current = ctx.current.as_ref().unwrap();
    let parent_weak = std::rc::Rc::downgrade(&Pin::into_inner(current.clone()));
    let globals = current
        .root
        .get()
        .and_then(|w| w.upgrade())
        .map(|inst| inst.globals.clone())
        .unwrap_or_else(|| std::rc::Rc::new(crate::globals::GlobalStorage::new(&cu)));
    let popup_vrc = crate::instance::Instance::new_popup(
        cu.clone(),
        &popup_menu.item_tree,
        parent_weak,
        globals,
    );
    // Install bindings now; defer `init_code` until after `show_popup` so
    // that `forward-focus` calls reach a popup the window adapter already
    // considers active. The Rust codegen splits these the same way.
    crate::instance::install_bindings_for_repeated_row(&popup_vrc);

    // Wire entries/sub_menu/activated on the popup. Two flavors:
    //
    //   - `ShowPopupMenu` (regular `ContextMenuArea`): the entries come
    //     from a `MenuItem` tree we walk via `MenuFromItemTree`.
    //   - `ShowPopupMenuInternal` (`ContextMenuInternal`): the entries
    //     come from an array property on the user's `ContextMenu` item,
    //     and `sub_menu` / `activated` forward back to the user's
    //     callbacks rather than a shadow tree.
    let popup_ctx = crate::eval::EvalContext::new(popup_vrc.root_sub_component.clone());

    if let Expression::NumberLiteral(tree_index) = entries_expr {
        let sc = &cu.sub_components[current.sub_component_idx];
        let Some(menu_tree) = sc.menu_item_trees.get(*tree_index as usize) else {
            return Value::Void;
        };
        let menu_vrc = crate::instance::Instance::new_popup(
            cu.clone(),
            menu_tree,
            std::rc::Rc::downgrade(&Pin::into_inner(current.clone())),
            popup_vrc.globals.clone(),
        );
        crate::instance::finalize_instance(&menu_vrc);
        let menu_item_tree = vtable::VRc::new(i_slint_core::menus::MenuFromItemTree::new(
            vtable::VRc::into_dyn(menu_vrc),
        ));

        let mt = vtable::VRc::clone(&menu_item_tree);
        wire_popup_menu_prop(&popup_ctx, &popup_menu.entries, move || {
            let mut entries = i_slint_core::SharedVector::default();
            i_slint_core::menus::Menu::sub_menu(&*mt, None.into(), &mut entries);
            Value::Model(i_slint_core::model::ModelRc::new(i_slint_core::model::VecModel::from(
                entries.into_iter().map(Value::from).collect::<Vec<_>>(),
            )))
        });

        let mt = vtable::VRc::clone(&menu_item_tree);
        wire_popup_menu_cb(&popup_ctx, &popup_menu.sub_menu, move |args| {
            let entry = args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
            let mut entries = i_slint_core::SharedVector::default();
            i_slint_core::menus::Menu::sub_menu(&*mt, Some(&entry).into(), &mut entries);
            Value::Model(i_slint_core::model::ModelRc::new(i_slint_core::model::VecModel::from(
                entries.into_iter().map(Value::from).collect::<Vec<_>>(),
            )))
        });

        wire_popup_menu_cb(&popup_ctx, &popup_menu.activated, move |args| {
            let entry = args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
            i_slint_core::menus::Menu::activate(&*menu_item_tree, &entry);
            Value::Void
        });
    } else {
        // ShowPopupMenuInternal: entries are an array property of the
        // `ContextMenuInternal` item; sub_menu/activated forward to the
        // user-defined callbacks on the same item.
        let entries_value = eval_expression(ctx, entries_expr);
        wire_popup_menu_prop(&popup_ctx, &popup_menu.entries, move || entries_value.clone());

        if let MemberReference::Relative { parent_level, local_reference } = context_menu_ref {
            let sub_menu_owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
            let LocalMemberIndex::Native { item_index, .. } = &local_reference.reference else {
                return Value::Void;
            };
            let item_index_for_sub = *item_index;
            let sub_menu_owner_weak =
                std::rc::Rc::downgrade(&Pin::into_inner(sub_menu_owner.clone()));
            wire_popup_menu_cb(&popup_ctx, &popup_menu.sub_menu, move |args| {
                let Some(owner) = sub_menu_owner_weak.upgrade() else { return Value::Void };
                let item = Pin::as_ref(&owner.items[item_index_for_sub]);
                let entry: i_slint_core::items::MenuEntry =
                    args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
                let raw = item.as_item_ref();
                use i_slint_core::items::ContextMenu;
                let Some(cm) = vtable::VRef::downcast_pin::<ContextMenu>(raw) else {
                    return Value::Void;
                };
                let mut out = i_slint_core::SharedVector::default();
                cm.sub_menu.call(&(entry,)).iter().for_each(|e| out.push(e));
                Value::Model(i_slint_core::model::ModelRc::new(
                    i_slint_core::model::VecModel::from(
                        out.into_iter().map(Value::from).collect::<Vec<_>>(),
                    ),
                ))
            });

            let activated_owner_weak =
                std::rc::Rc::downgrade(&Pin::into_inner(sub_menu_owner.clone()));
            let item_index_for_activated = *item_index;
            wire_popup_menu_cb(&popup_ctx, &popup_menu.activated, move |args| {
                let Some(owner) = activated_owner_weak.upgrade() else { return Value::Void };
                let item = Pin::as_ref(&owner.items[item_index_for_activated]);
                let entry: i_slint_core::items::MenuEntry =
                    args.first().cloned().unwrap_or_default().try_into().unwrap_or_default();
                let raw = item.as_item_ref();
                use i_slint_core::items::ContextMenu;
                let Some(cm) = vtable::VRef::downcast_pin::<ContextMenu>(raw) else {
                    return Value::Void;
                };
                cm.activated.call(&(entry,));
                Value::Void
            });
        }
    }

    // Wire the popup menu's `close` callback so navigating into a menu
    // item that calls `root.close()` actually dismisses the popup. Use a
    // shared `Cell` to bridge the popup id (only known after
    // `show_popup`) into the close handler installed before the show.
    let popup_id_cell: std::rc::Rc<std::cell::Cell<Option<core::num::NonZeroU32>>> =
        std::rc::Rc::new(std::cell::Cell::new(None));
    let id_cell_for_close = popup_id_cell.clone();
    let adapter_for_close = adapter.clone();
    wire_popup_menu_cb(&popup_ctx, &popup_menu.close, move |_args| {
        if let Some(id) = id_cell_for_close.take() {
            i_slint_core::window::WindowInner::from_pub(adapter_for_close.window()).close_popup(id);
        }
        Value::Void
    });

    let popup_dyn = vtable::VRc::into_dyn(popup_vrc.clone());
    let window_inner = i_slint_core::window::WindowInner::from_pub(adapter.window());
    let popup_id = window_inner.show_popup(
        &popup_dyn,
        position,
        i_slint_core::items::PopupClosePolicy::CloseOnClickOutside,
        &context_item_rc,
        true,
    );
    popup_id_cell.set(Some(popup_id));
    // Mirror the rust codegen: store the popup id on the native
    // `ContextMenu` item so the generated `is-open()` and `close()`
    // member functions see this popup as the active one.
    if let Some(item_rc) = context_menu_item_weak.upgrade()
        && let Some(cm) = item_rc.downcast::<i_slint_core::items::ContextMenu>()
    {
        cm.as_pin_ref().popup_id.set(Some(popup_id));
    }

    // Run the popup's `init_code` now that the window adapter has the
    // popup registered, so `forward-focus` calls can target items in the
    // popup.
    crate::instance::finalize_instance(&popup_vrc);

    Value::Void
}

fn wire_popup_menu_prop(
    ctx: &EvalContext,
    mr: &MemberReference,
    binding: impl Fn() -> Value + 'static,
) {
    if let MemberReference::Relative { parent_level, local_reference } = mr {
        let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
        if let LocalMemberIndex::Property(idx) = &local_reference.reference {
            Pin::as_ref(&owner.properties[*idx]).set_binding(binding);
        }
    }
}

fn wire_popup_menu_cb(
    ctx: &EvalContext,
    mr: &MemberReference,
    handler: impl Fn(&[Value]) -> Value + 'static,
) {
    if let MemberReference::Relative { parent_level, local_reference } = mr {
        let owner = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
        if let LocalMemberIndex::Callback(idx) = &local_reference.reference {
            Pin::as_ref(&owner.callbacks[*idx])
                .set_handler(move |(args,): &(Vec<Value>,)| handler(args));
        }
    }
}
