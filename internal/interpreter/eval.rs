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
use i_slint_compiler::langtype::{ConstantExpression, Type};
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
    /// The compilation unit, for type resolution even when `current` is
    /// `None` (global context).
    pub compilation_unit: Rc<llr::CompilationUnit>,
    /// Shared global storage, used to resolve `MemberReference::Global`.
    pub globals: Weak<GlobalStorage>,
    /// Local variables introduced by `StoreLocalVariable`.
    pub locals: HashMap<SmolStr, Value>,
    /// Arguments of the current function, if any.
    pub function_arguments: Vec<Value>,
    /// Declared types of `function_arguments`, for
    /// [`i_slint_compiler::llr::TypeResolutionContext::arg_type`].
    pub function_arg_types: Vec<Type>,
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
            compilation_unit: current.compilation_unit.clone(),
            current: Some(current),
            globals,
            locals: HashMap::new(),
            function_arguments: Vec::new(),
            function_arg_types: Vec::new(),
            return_value: None,
        }
    }

    /// Context rooted in a global. Only `MemberReference::Global` is valid.
    pub fn for_global(globals: Weak<GlobalStorage>, cu: Rc<llr::CompilationUnit>) -> Self {
        Self {
            current: None,
            compilation_unit: cu,
            globals,
            locals: HashMap::new(),
            function_arguments: Vec::new(),
            function_arg_types: Vec::new(),
            return_value: None,
        }
    }

    pub fn with_arguments(current: Pin<Rc<SubComponentInstance>>, args: Vec<Value>) -> Self {
        let mut ctx = Self::new(current);
        ctx.function_arguments = args;
        ctx
    }
}

/// The root instance, for builtins that need the window.
/// In a global context, reach it through the global storage.
fn root_instance(
    ctx: &EvalContext,
) -> Option<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, crate::instance::Instance>> {
    match ctx.current.as_ref() {
        Some(c) => c.root.get()?.upgrade(),
        None => ctx.globals.upgrade()?.root.get()?.upgrade(),
    }
}

/// Walk `parent_level` steps up the parent chain.
pub(crate) fn walk_parent(
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

impl i_slint_compiler::llr::TypeResolutionContext for EvalContext {
    fn property_ty(&self, mr: &MemberReference) -> &Type {
        let cu = &self.compilation_unit;
        match mr {
            MemberReference::Global { global_index, member } => {
                let g = &cu.globals[*global_index];
                match member {
                    LocalMemberIndex::Property(idx) => &g.properties[*idx].ty,
                    LocalMemberIndex::Function(idx) => &g.functions[*idx].ret_ty,
                    // The stored `Type::Callback` — `Expression::ty()`'s
                    // CallBackCall arm extracts the return type from it.
                    LocalMemberIndex::Callback(idx) => &g.callbacks[*idx].ty,
                    LocalMemberIndex::Native { .. } | LocalMemberIndex::Timer(_) => &Type::Invalid,
                }
            }
            MemberReference::Relative { parent_level, local_reference } => {
                let current =
                    self.current.as_ref().expect("property_ty needs a sub-component context");
                // The `Type` values live in the shared `CompilationUnit`, so
                // resolve the target sub-component index through the runtime
                // parent chain and borrow from `cu`.
                let sub = walk_parent(current, *parent_level);
                let mut sc_idx = sub.sub_component_idx;
                for i in &local_reference.sub_component_path {
                    sc_idx = cu.sub_components[sc_idx].sub_components[*i].ty;
                }
                let sc = &cu.sub_components[sc_idx];
                match &local_reference.reference {
                    LocalMemberIndex::Property(idx) => &sc.properties[*idx].ty,
                    LocalMemberIndex::Function(idx) => &sc.functions[*idx].ret_ty,
                    LocalMemberIndex::Callback(idx) => &sc.callbacks[*idx].ty,
                    // A timer reference is only valid as the RestartTimer argument.
                    LocalMemberIndex::Timer(_) => &Type::Invalid,
                    LocalMemberIndex::Native { item_index, prop_name, .. } => {
                        if prop_name == "elements" {
                            // The `Path::elements` property is not in the NativeClass
                            return &Type::PathData;
                        }
                        sc.items[*item_index]
                            .ty
                            .lookup_property(prop_name)
                            .unwrap_or(&Type::Invalid)
                    }
                }
            }
        }
    }

    fn arg_type(&self, index: usize) -> &Type {
        self.function_arg_types.get(index).unwrap_or(&Type::Invalid)
    }
}

/// Walk down a `sub_component_path`.
pub(crate) fn walk_sub_path(
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
pub(crate) fn walk_to(
    ctx: &EvalContext,
    parent_level: usize,
    path: &[llr::SubComponentInstanceIdx],
) -> Pin<Rc<SubComponentInstance>> {
    let start = ctx.current.as_ref().expect("relative member reference without a sub-component");
    walk_sub_path(walk_parent(start, parent_level), path)
}

/// Flat tree index of the `item_table` entry matching `(path, item_index)`.
pub(crate) fn find_flat_item_index(
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
        LocalMemberIndex::Callback(_)
        | LocalMemberIndex::Function(_)
        | LocalMemberIndex::Timer(_) => {
            panic!("load_local called on callback/function/timer reference")
        }
    }
}

/// Set `value` on `prop`, interpolating through `animation` when present.
fn set_maybe_animated(
    prop: Pin<&i_slint_core::Property<Value>>,
    ty: &Type,
    value: Value,
    animation: Option<i_slint_core::items::PropertyAnimation>,
) {
    match animation {
        Some(anim) => match crate::bindings::animated_value_map(ty) {
            Some(map) => prop.set_animated_value_with_map(value, anim, map),
            None => prop.set_animated_value(value, anim),
        },
        None => prop.set(value),
    }
}

fn store_local(
    instance: &SubComponentInstance,
    member: &LocalMemberIndex,
    value: Value,
    animation: Option<i_slint_core::items::PropertyAnimation>,
) {
    match member {
        LocalMemberIndex::Property(idx) => {
            let sc = &instance.compilation_unit.sub_components[instance.sub_component_idx];
            set_maybe_animated(
                Pin::as_ref(&instance.properties[*idx]),
                &sc.properties[*idx].ty,
                value,
                animation,
            );
        }
        LocalMemberIndex::Native { item_index, prop_name, .. } => {
            let _ =
                Pin::as_ref(&instance.items[*item_index]).set_property(prop_name, value, animation);
        }
        LocalMemberIndex::Callback(_)
        | LocalMemberIndex::Function(_)
        | LocalMemberIndex::Timer(_) => {
            panic!("store_local called on callback/function/timer reference")
        }
    }
}

/// Walk down `local_reference.sub_component_path` from `start`, returning the
/// target instance and any standalone `animate` declaration for this member.
/// An `animate` on a child component's property lives in the enclosing
/// component's animations map with a non-empty path; like codegen's
/// `property_info`, the outermost declaration wins and its expression
/// evaluates in the scope that declared it.
fn walk_to_target_with_animation(
    start: Pin<Rc<SubComponentInstance>>,
    local_reference: &llr::LocalMemberReference,
) -> (Pin<Rc<SubComponentInstance>>, Option<i_slint_core::items::PropertyAnimation>) {
    let cu = start.compilation_unit.clone();
    let path = &local_reference.sub_component_path;
    let mut animation = None;
    let mut owner = start;
    for depth in 0..=path.len() {
        if animation.is_none() {
            let sc = &cu.sub_components[owner.sub_component_idx];
            if !sc.animations.is_empty() {
                let key = llr::LocalMemberReference {
                    sub_component_path: path[depth..].to_vec(),
                    reference: local_reference.reference.clone(),
                };
                if let Some(expr) = sc.animations.get(&key) {
                    animation = Some((owner.clone(), expr.clone()));
                }
            }
        }
        if let Some(&idx) = path.get(depth) {
            let next = owner.sub_components[idx].clone();
            owner = next;
        }
    }
    let animation = animation.map(|(scope, expr)| {
        let mut ctx = EvalContext::new(scope);
        crate::bindings::value_to_property_animation(eval_expression(&mut ctx, &expr))
    });
    (owner, animation)
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
            store_global(&ctx.globals, global, member, value);
        }
        MemberReference::Relative { parent_level, local_reference } => {
            let start =
                ctx.current.as_ref().expect("relative member reference without a sub-component");
            let (instance, animation) =
                walk_to_target_with_animation(walk_parent(start, *parent_level), local_reference);
            store_local(&instance, &local_reference.reference, value, animation);
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
            if let Some(native) = &global.native {
                let res = native.as_ref().invoke_callback(&cb.name, args).unwrap_or(Value::Void);
                return ensure_typed_default(res, &cb.ret_ty);
            }
            // Register a dependency on the handler so bindings invoking this
            // callback re-evaluate when a new handler is set.
            if let Some(tracker) = global.callback_trackers[*idx].as_ref() {
                Pin::as_ref(tracker).get();
            }
            let res = Pin::as_ref(&global.callbacks[*idx]).call(args);
            ensure_typed_default(res, &cb.ret_ty)
        }
        MemberReference::Relative { parent_level, local_reference } => {
            let instance = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
            match &local_reference.reference {
                LocalMemberIndex::Callback(idx) => {
                    // Register a dependency on the handler so bindings
                    // invoking this callback re-evaluate when a new handler
                    // is set.
                    if let Some(tracker) = instance.callback_trackers[*idx].as_ref() {
                        Pin::as_ref(tracker).get();
                    }
                    let res = Pin::as_ref(&instance.callbacks[*idx]).call(args);
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

/// Replace a `Value::Void` result (e.g. from an unset callback) with the
/// type-appropriate default.
pub(crate) fn ensure_typed_default(value: Value, ret_ty: &Type) -> Value {
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
            let function = &global.compilation_unit.globals[global.global_idx].functions[*idx];
            let code = function.code.borrow().clone();
            let mut inner_ctx =
                EvalContext::for_global(ctx.globals.clone(), global.compilation_unit.clone());
            inner_ctx.function_arg_types = function.args.clone();
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
            let code = function.code.borrow().clone();
            let mut inner_ctx = EvalContext::with_arguments(instance.clone(), args);
            inner_ctx.function_arg_types = function.args.clone();
            eval_expression(&mut inner_ctx, &code)
        }
    }
}

fn load_global(global: &Rc<GlobalInstance>, member: &LocalMemberIndex) -> Value {
    match member {
        LocalMemberIndex::Property(idx) => {
            if let Some(native) = &global.native {
                let g = &global.compilation_unit.globals[global.global_idx];
                return native
                    .as_ref()
                    .get_property(&g.properties[*idx].name)
                    .unwrap_or(Value::Void);
            }
            Pin::as_ref(&global.properties[*idx]).get()
        }
        _ => panic!("load_global called on non-property"),
    }
}

pub(crate) fn store_global(
    globals: &Weak<GlobalStorage>,
    global: &Rc<GlobalInstance>,
    member: &LocalMemberIndex,
    value: Value,
) {
    if let LocalMemberIndex::Property(idx) = member {
        let g = &global.compilation_unit.globals[global.global_idx];
        // A standalone `animate` that resolved to this global property (e.g.
        // through a two-way alias) interpolates the new value.
        let animation = g.animations.get(member).map(|anim_expr| {
            let anim_expr = anim_expr.clone();
            let mut ctx = EvalContext::for_global(globals.clone(), global.compilation_unit.clone());
            crate::bindings::value_to_property_animation(eval_expression(&mut ctx, &anim_expr))
        });
        if let Some(native) = &global.native {
            let _ = native.as_ref().set_property(&g.properties[*idx].name, value, animation);
            return;
        }
        set_maybe_animated(
            Pin::as_ref(&global.properties[*idx]),
            &g.properties[*idx].ty,
            value,
            animation,
        );
    }
}

/// Build a `Value::PathData` from the `from` expression of a
/// `Expression::Cast { to: Type::PathData, .. }`.
///
/// `lower_expression::compile_path` lowers `Path::Elements` to an array of
/// builtin-struct literals, `Path::Events` to a struct with `events` /
/// `points` fields, and `Path::Commands` to a string expression. The code
/// generators navigate these statically; the interpreter pattern-matches on
/// the expression itself because `Value::Struct` doesn't carry its LLR type
/// name.
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
                Value::Model(m) => {
                    (0..m.row_count()).filter_map(|i| m.row_data(i)?.try_into().ok()).collect()
                }
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
/// `StructName::Builtin` tag.
fn path_element_from_expression(
    ctx: &mut EvalContext,
    expr: &Expression,
) -> Option<i_slint_core::graphics::PathElement> {
    use i_slint_compiler::langtype::{BuiltinStruct, StructName};
    use i_slint_core::graphics::{
        PathArcTo, PathCubicTo, PathElement, PathLineTo, PathMoveTo, PathQuadraticTo,
    };
    let Expression::Struct { ty, values } = expr else { return None };
    let StructName::Builtin(bs) = &ty.name else { return None };
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
        BuiltinStruct::PathMoveTo => {
            PathElement::MoveTo(PathMoveTo { x: get_f32("x", ctx), y: get_f32("y", ctx) })
        }
        BuiltinStruct::PathLineTo => {
            PathElement::LineTo(PathLineTo { x: get_f32("x", ctx), y: get_f32("y", ctx) })
        }
        BuiltinStruct::PathArcTo => PathElement::ArcTo(PathArcTo {
            x: get_f32("x", ctx),
            y: get_f32("y", ctx),
            radius_x: get_f32("radius-x", ctx),
            radius_y: get_f32("radius-y", ctx),
            x_rotation: get_f32("x-rotation", ctx),
            large_arc: get_bool("large-arc", ctx),
            sweep: get_bool("sweep", ctx),
        }),
        BuiltinStruct::PathCubicTo => PathElement::CubicTo(PathCubicTo {
            x: get_f32("x", ctx),
            y: get_f32("y", ctx),
            control_1_x: get_f32("control-1-x", ctx),
            control_1_y: get_f32("control-1-y", ctx),
            control_2_x: get_f32("control-2-x", ctx),
            control_2_y: get_f32("control-2-y", ctx),
        }),
        BuiltinStruct::PathQuadraticTo => PathElement::QuadraticTo(PathQuadraticTo {
            x: get_f32("x", ctx),
            y: get_f32("y", ctx),
            control_x: get_f32("control-x", ctx),
            control_y: get_f32("control-y", ctx),
        }),
        BuiltinStruct::PathClose => PathElement::Close,
        _ => return None,
    })
}

/// Default `Value` for a type, used when a callback or model access yields
/// nothing but the caller expects a typed value.
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
            s.fields
                .keys()
                .map(|k| (k.to_string(), default_value_for_struct_field(s, k)))
                .collect(),
        ),
        Type::Array(_) | Type::Model => Value::Model(ModelRc::default()),
        Type::Keys => Value::Keys(Default::default()),
        Type::DataTransfer => Value::DataTransfer(Default::default()),
        Type::StyledText => Value::StyledText(Default::default()),
        Type::Enumeration(en) => {
            let default = en.clone().default_value();
            Value::EnumerationValue(en.name.to_string(), default.to_string())
        }
        _ => Value::Void,
    }
}

/// The default for a struct field: the user-declared default value
/// (`struct Foo { bar: int = 42 }`) if there is one, otherwise the default for
/// the field's type.
pub fn default_value_for_struct_field(
    s: &i_slint_compiler::langtype::Struct,
    field_name: &str,
) -> Value {
    match s.field_defaults.get(field_name) {
        Some(expr) => eval_constant_expression(expr),
        None => default_value_for_type(
            s.fields.get(field_name).expect("default value requested for unknown struct field"),
        ),
    }
}

/// Evaluate a constant expression as stored in
/// [`i_slint_compiler::langtype::Struct::field_defaults`].
fn eval_constant_expression(expr: &ConstantExpression) -> Value {
    match expr {
        ConstantExpression::StringLiteral(s) => Value::String(s.as_str().into()),
        ConstantExpression::NumberLiteral(n, _unit) => Value::Number(*n),
        ConstantExpression::BoolLiteral(b) => Value::Bool(*b),
        ConstantExpression::EnumerationValue(value) => {
            Value::EnumerationValue(value.enumeration.name.to_string(), value.to_string())
        }
        ConstantExpression::Cast { from, to } => {
            cast_constant_value(eval_constant_expression(from), to)
        }
        ConstantExpression::UnaryOp { sub, op } => {
            // The resolver only accepts unary operators on matching operand types.
            match (eval_constant_expression(sub), op) {
                (Value::Number(a), '+') => Value::Number(a),
                (Value::Number(a), '-') => Value::Number(-a),
                (Value::Bool(a), '!') => Value::Bool(!a),
                (sub, _) => panic!("unsupported {op} {sub:?}"),
            }
        }
        ConstantExpression::Struct { values, .. } => Value::Struct(
            values
                .iter()
                .map(|(k, v)| (k.to_string(), eval_constant_expression(v)))
                .collect::<crate::api::Struct>(),
        ),
        ConstantExpression::Array { values, .. } => {
            Value::Model(ModelRc::new(SharedVectorModel::from(
                values.iter().map(eval_constant_expression).collect::<SharedVector<_>>(),
            )))
        }
    }
}

/// Convert a value to the given type, as [`Expression::Cast`] does.
fn cast_constant_value(value: Value, to: &Type) -> Value {
    match (value, to) {
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

pub fn eval_expression(ctx: &mut EvalContext, expression: &Expression) -> Value {
    if let Some(r) = &ctx.return_value {
        return r.clone();
    }
    match expression {
        Expression::StringLiteral(s) => Value::String(s.as_str().into()),
        Expression::NumberLiteral(n) => Value::Number(*n),
        Expression::BoolLiteral(b) => Value::Bool(*b),
        Expression::KeysLiteral(ks) => Value::Keys({
            let mut modifiers = i_slint_core::input::KeyboardModifiers::default();
            modifiers.alt = ks.modifiers.alt;
            modifiers.control = ks.modifiers.control;
            modifiers.shift = ks.modifiers.shift;
            modifiers.meta = ks.modifiers.meta;
            i_slint_core::input::make_keys(
                SharedString::from(&*ks.key),
                modifiers,
                ks.ignore_shift,
                ks.ignore_alt,
            )
        }),
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
                        // Out of bounds or empty model: synthesize the element
                        // type's default like the generated code does.
                        default_value_for_type(&expression.ty(&*ctx))
                    })
                }
                _ => Value::Void,
            }
        }
        Expression::Cast { from, to } => {
            // The `Path` native item's rtti setter needs a real
            // `Value::PathData`, not the raw model / struct / string that
            // `from` evaluates to.
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
        Expression::ItemMemberFunctionCall { function } => call_item_member_function(ctx, function),
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
                Some(Value::Model(m)) if *index < m.row_count() => {
                    m.set_row_data(*index, value);
                }
                _ => {}
            }
            Value::Void
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = eval_expression(ctx, lhs);
            // `&&` and `||` short-circuit like in the generated code, or else
            // rhs side effects would run in the interpreter only.
            match (op, &lhs) {
                ('&', Value::Bool(false)) => return Value::Bool(false),
                ('|', Value::Bool(true)) => return Value::Bool(true),
                _ => {}
            }
            let rhs = eval_expression(ctx, rhs);
            binary_op(*op, lhs, rhs)
        }
        Expression::UnaryOp { sub, op } => {
            let sub = eval_expression(ctx, sub);
            match (sub, op) {
                (Value::Number(a), '+') => Value::Number(a),
                (Value::Number(a), '-') => Value::Number(-a),
                (Value::Bool(a), '!') => Value::Bool(!a),
                // Coerce `Void` from uninitialized properties instead of
                // panicking.
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
        Expression::MouseCursor(cursor) => {
            use i_slint_compiler::expression_tree::MouseCursorInner as Expr;
            use i_slint_core::cursor::MouseCursorInner as Core;
            Value::MouseCursorInner(match cursor {
                Expr::BuiltIn(cursor) => {
                    Core::BuiltIn(eval_expression(ctx, cursor).try_into().unwrap_or_default())
                }
                Expr::CustomMouseCursor { image, hotspot_x, hotspot_y } => {
                    Core::CustomMouseCursor {
                        image: eval_expression(ctx, image).try_into().unwrap_or_default(),
                        hotspot_x: eval_expression(ctx, hotspot_x).try_into().unwrap_or_default(),
                        hotspot_y: eval_expression(ctx, hotspot_y).try_into().unwrap_or_default(),
                    }
                }
            })
        }
        Expression::LinearGradient { angle, stops } => {
            let angle: f32 = eval_expression(ctx, angle).try_into().unwrap_or_default();
            Value::Brush(Brush::LinearGradient(LinearGradientBrush::new(
                angle,
                eval_stops(ctx, stops),
            )))
        }
        Expression::RadialGradient { stops, center, radius } => {
            let mut g = RadialGradientBrush::new_circle(eval_stops(ctx, stops));
            if let Some((cx, cy)) = center {
                let cx: f32 = eval_expression(ctx, cx).try_into().unwrap_or_default();
                let cy: f32 = eval_expression(ctx, cy).try_into().unwrap_or_default();
                g = g.with_center(cx, cy);
            }
            if let Some(r) = radius {
                let r: f32 = eval_expression(ctx, r).try_into().unwrap_or_default();
                g = g.with_radius(r);
            }
            Value::Brush(Brush::RadialGradient(g))
        }
        Expression::ConicGradient { from_angle, stops, center } => {
            let from_angle: f32 = eval_expression(ctx, from_angle).try_into().unwrap_or_default();
            let mut g = ConicGradientBrush::new(from_angle, eval_stops(ctx, stops));
            if let Some((cx, cy)) = center {
                let cx: f32 = eval_expression(ctx, cx).try_into().unwrap_or_default();
                let cy: f32 = eval_expression(ctx, cy).try_into().unwrap_or_default();
                g = g.with_center(cx, cy);
            }
            Value::Brush(Brush::ConicGradient(g))
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
            repeated_cross_width,
            sub_expression,
            ..
        } => with_flexbox_layout_item_info(
            ctx,
            cells_h_variable,
            cells_v_variable,
            elements,
            repeated_cross_width.as_deref(),
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
        Expression::EmptyDataTransfer => Value::DataTransfer(Default::default()),
        Expression::SolveFlexboxLayoutWithMeasure { .. } => {
            crate::eval_layout::solve_flexbox_layout_with_measure(ctx, expression)
        }
        Expression::TranslationReference { .. } => {
            // TranslationReference is only emitted when `bundle-translations`
            // is active, which the interpreter does not use. Runtime @tr()
            // goes through BuiltinFunction::Translate instead.
            Value::String(Default::default())
        }
        Expression::DebugHook { expression, id } => {
            if let Some(hook_value) = crate::debug_hook::trigger_debug_hook(ctx, id) {
                return hook_value;
            }
            eval_expression(ctx, expression)
        }
    }
}

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
    let repeater = &current.repeaters[repeater_idx];
    repeater.track_instance_changes();
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
            let repeater = &sub.repeaters[*repeater_index];
            repeater.track_instance_changes();
            total += repeater.range().len();
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
    repeated_cross_width: Option<&Expression>,
    sub_expression: &Expression,
) -> Value {
    // For a column flex, re-measure each repeated cell at the container width so
    // a height-for-width instance wraps like an equivalent static cell.
    let cross_width =
        repeated_cross_width.map(|e| eval_expression(ctx, e).try_into().unwrap_or_default());
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
                    cross_width,
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
    cross_width: Option<f32>,
    cells_h: &mut Vec<Value>,
    cells_v: &mut Vec<Value>,
) -> u32 {
    use i_slint_core::items::Orientation;
    use i_slint_core::model::RepeatedItemTree;
    let Some(current) = ctx.current.as_ref() else { return 0 };
    let repeater = &current.repeaters[repeater_idx];
    repeater.track_instance_changes();
    let instances = repeater.instances_vec();
    let instance_count = instances.len() as u32;
    for instance in instances {
        // Flexbox needs `FlexboxLayoutItemInfo` (constraint plus flex fields);
        // the default `RepeatedItemTree::flexbox_layout_item_info` impl wraps
        // the box-layout info and zero-fills the flex fields.
        let info_h = RepeatedItemTree::flexbox_layout_item_info(
            instance.as_pin_ref(),
            Orientation::Horizontal,
            None,
        );
        // For a column flex, measure the vertical info at the container width so
        // a height-for-width cell wraps to the real width, not its preferred one.
        let info_v = match cross_width {
            Some(w) => instance.as_pin_ref().flexbox_layout_item_info_at_cross_width(w),
            None => RepeatedItemTree::flexbox_layout_item_info(
                instance.as_pin_ref(),
                Orientation::Vertical,
                None,
            ),
        };
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
    // Matches `generate_with_grid_input_data` in the Rust codegen:
    // `repeated_indices` holds `(offset, len)` pairs into `cells`,
    // `repeater_steps` the per-instance item count.
    // The `new_row` local tracks whether the next static cell starts a new
    // row: each repeater resets it to its static `new_row`, and a column
    // repeater that ran at least once clears it. Static cells after the
    // repeater read it via `ReadLocalVariable("new_row")`.
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
    let repeater = &current.repeaters[repeater_idx];
    repeater.track_instance_changes();

    let is_row_repeater = row_child_templates.is_some();
    let static_count =
        row_child_templates.map(i_slint_compiler::llr::static_child_count).unwrap_or(1);

    let instances = repeater.instances_vec();
    let instance_count = instances.len() as u32;

    // Step is the max total cells per instance. Every instance contributes
    // exactly `step` entries so the flattened cell vector lines up with
    // `repeater_steps` and `repeated_indices`, like the rust codegen emits it.
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
            let expr = expr.borrow();
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
            for (slot, i) in statics.iter_mut().zip(0..result_model.row_count()) {
                if let Some(v) = result_model.row_data(i) {
                    *slot = v;
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
                        let inner_rep = &inner_sub.repeaters[*repeater_index];
                        inner_rep.track_instance_changes();
                        // Let each inner cell report its own
                        // col/row/colspan/rowspan via its
                        // `grid_layout_input_for_repeated` expression.
                        for inner_inst in inner_rep.instances_vec() {
                            if written >= step {
                                break;
                            }
                            for mut v in eval_grid_input_for_repeated(
                                &inner_inst.root_sub_component,
                                written == 0 && current_new_row,
                            ) {
                                if written >= step {
                                    break;
                                }
                                override_new_row(&mut v, written == 0 && current_new_row);
                                cells.push(v);
                                written += 1;
                            }
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

/// Evaluate a repeated cell's own `grid_layout_input_for_repeated`
/// expression, so it reports its declared col/row/colspan/rowspan. Falls
/// back to a single auto-positioned cell when the sub-component has no
/// grid input expression.
fn eval_grid_input_for_repeated(
    sub: &Pin<Rc<crate::instance::SubComponentInstance>>,
    new_row: bool,
) -> Vec<Value> {
    use i_slint_core::model::{Model, VecModel};
    let cu = sub.compilation_unit.clone();
    let sc = &cu.sub_components[sub.sub_component_idx];
    let count = sc
        .row_child_templates
        .as_ref()
        .map(|t| i_slint_compiler::llr::static_child_count(t))
        .unwrap_or(1)
        .max(1);
    let Some(expr) = &sc.grid_layout_input_for_repeated else {
        return vec![auto_grid_input_data()];
    };
    let expr = expr.borrow();
    let mut ctx = EvalContext::new(sub.clone());
    let result_model: Rc<VecModel<Value>> = Rc::new(VecModel::default());
    for _ in 0..count {
        result_model.push(Value::Void);
    }
    ctx.locals.insert(
        SmolStr::new_static("result"),
        Value::Model(i_slint_core::model::ModelRc::from(result_model.clone())),
    );
    ctx.locals.insert(SmolStr::new_static("new_row"), Value::Bool(new_row));
    eval_expression(&mut ctx, &expr);
    (0..result_model.row_count())
        .map(|i| result_model.row_data(i).unwrap_or_else(auto_grid_input_data))
        .collect()
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
    let image = match resource_ref {
        Ref::None => Ok(Default::default()),
        Ref::DataUri(data_uri) => i_slint_compiler::data_uri::decode_data_uri(data_uri)
            .ok()
            .and_then(|(data, extension)| {
                i_slint_core::graphics::load_image_from_data_uri(data_uri, &data, &extension).ok()
            })
            .ok_or_else(Default::default),
        Ref::Url(url) if url.scheme() == "builtin" => {
            // Style-bundled resources (e.g. cosmic/material widget icons) are
            // baked into the compiler's builtin library and need to be fetched
            // through `fileaccess::load_file` rather than the filesystem.
            let path = std::path::Path::new(url.as_str());
            i_slint_compiler::fileaccess::load_file(path)
                .and_then(|virtual_file| virtual_file.builtin_contents)
                .map(|contents| {
                    let extension = path.extension().unwrap().to_str().unwrap();
                    i_slint_core::graphics::load_image_from_embedded_data(
                        i_slint_core::slice::Slice::from_slice(contents),
                        i_slint_core::slice::Slice::from_slice(extension.as_bytes()),
                    )
                })
                .ok_or_else(Default::default)
        }
        Ref::Path(path) => {
            i_slint_core::graphics::Image::load_from_path(std::path::Path::new(path.as_str()))
        }
        Ref::Url(url) => {
            #[cfg(target_arch = "wasm32")]
            {
                i_slint_core::graphics::load_as_html_image(url.as_str())
            }
            // URL image references only work on the web, where the browser fetches them.
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = url;
                Err(Default::default())
            }
        }
        Ref::EmbeddedData { .. } | Ref::EmbeddedTexture { .. } => Ok(Default::default()),
    };
    image.unwrap_or_else(|_| {
        eprintln!("Could not load image {resource_ref:?}");
        Default::default()
    })
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
        BuiltinFunction::StringStartsWith => Value::Bool(
            to_string(ctx, &arguments[0])
                .as_str()
                .starts_with(to_string(ctx, &arguments[1]).as_str()),
        ),
        BuiltinFunction::StringEndsWith => Value::Bool(
            to_string(ctx, &arguments[0])
                .as_str()
                .ends_with(to_string(ctx, &arguments[1]).as_str()),
        ),
        BuiltinFunction::ToStringUnlocalized => {
            let n = to_num(ctx, &arguments[0]);
            Value::String(i_slint_core::string::shared_string_from_number_unlocalized(n))
        }
        BuiltinFunction::DecimalSeparator => Value::String(
            find_window_adapter(ctx)
                .map(|adapter| {
                    i_slint_core::window::WindowInner::from_pub(adapter.window())
                        .context()
                        .locale_decimal_separator()
                })
                .unwrap_or_default()
                .into(),
        ),
        BuiltinFunction::MacosBringAllWindowsToFront => {
            i_slint_core::macos_bring_all_windows_to_front();
            Value::Void
        }
        BuiltinFunction::ColorToStyledText => {
            let color: i_slint_core::Color =
                eval_expression(ctx, &arguments[0]).try_into().unwrap_or_default();
            Value::StyledText(i_slint_core::styled_text::color_to_styled_text(color))
        }
        BuiltinFunction::SetupSystemTrayIcon => {
            crate::popup::setup_system_tray_icon(ctx, arguments)
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
        BuiltinFunction::ArrayPush => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ArrayPush")
            }

            let model = match eval_expression(ctx, &arguments[0]) {
                Value::Model(m) => m,
                _ => panic!("First argument not an array: {:?}", arguments[0]),
            };
            let value = eval_expression(ctx, &arguments[1]);

            model.push_row(value);

            Value::Void
        }
        BuiltinFunction::ArrayRemove => {
            if arguments.len() != 2 {
                panic!("internal error: incorrect argument count to ArrayRemove")
            }

            let model = match eval_expression(ctx, &arguments[0]) {
                Value::Model(m) => m,
                _ => panic!("First argument not an array: {:?}", arguments[0]),
            };
            let index = match eval_expression(ctx, &arguments[1]) {
                Value::Number(i) => i,
                _ => panic!("Second argument not an integer: {:?}", arguments[1]),
            };

            model.remove_row(index as isize);

            Value::Void
        }

        BuiltinFunction::ArrayInsert => {
            if arguments.len() != 3 {
                panic!("internal error: incorrect argument count to ArrayInsert")
            }

            let model = match eval_expression(ctx, &arguments[0]) {
                Value::Model(m) => m,
                _ => panic!("First argument not an array: {:?}", arguments[0]),
            };
            let index = match eval_expression(ctx, &arguments[1]) {
                Value::Number(i) => i,
                _ => panic!("Second argument not an integer: {:?}", arguments[1]),
            };

            let value = eval_expression(ctx, &arguments[2]);
            model.insert_row(index as isize, value);

            Value::Void
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
            let factor = root_instance(ctx)
                .and_then(|inst| inst.window_adapter_or_default())
                .map(|adapter| {
                    i_slint_core::window::WindowInner::from_pub(adapter.window()).scale_factor()
                        as f64
                })
                .unwrap_or(1.0);
            Value::Number(factor)
        }
        BuiltinFunction::GetWindowDefaultFontSize => {
            // Read `default-font-size` from the nearest enclosing
            // `WindowItem`. The walk crosses popup and embedded-tree
            // boundaries, so `1rem` inside a popup of an embedded component
            // resolves against that component's own window, not the host
            // window that the window adapter points at.
            let size = root_instance(ctx)
                .map(|inst| {
                    i_slint_core::items::WindowItem::resolved_default_font_size(
                        vtable::VRc::into_dyn(inst),
                    )
                    .get() as f64
                })
                .unwrap_or(12.0);
            Value::Number(size)
        }
        BuiltinFunction::DetectOperatingSystem => i_slint_core::detect_operating_system().into(),
        BuiltinFunction::Use24HourFormat => {
            Value::Bool(i_slint_core::date_time::use_24_hour_format())
        }
        BuiltinFunction::ColorScheme => {
            let scheme = root_instance(ctx)
                .map(vtable::VRc::into_dyn)
                .and_then(|root| {
                    i_slint_core::window::context_for_root(&root)
                        .map(|ctx| ctx.color_scheme(Some(&root)))
                })
                .unwrap_or(i_slint_core::items::ColorScheme::Unknown);
            scheme.into()
        }
        BuiltinFunction::AccentColor => {
            let color = root_instance(ctx)
                .map(vtable::VRc::into_dyn)
                .map(|root| i_slint_core::window::accent_color(&root))
                .unwrap_or_default();
            Value::Brush(i_slint_core::Brush::SolidColor(color))
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
            // Timers react to property changes through the change trackers
            // installed in `bindings::install_timers`; nothing to do here.
            Value::Void
        }
        BuiltinFunction::RestartTimer => {
            // The timer is referenced through a member reference carrying a
            // `LocalMemberIndex::Timer`, so it resolves in the component that
            // declares it even when the call is made from (or inlined into) a
            // repeated/conditional child or another component.
            if let [
                Expression::PropertyReference(MemberReference::Relative {
                    parent_level,
                    local_reference,
                }),
            ] = arguments
                && let LocalMemberIndex::Timer(timer_idx) = &local_reference.reference
                && ctx.current.is_some()
            {
                let instance = walk_to(ctx, *parent_level, &local_reference.sub_component_path);
                if let Some(timer) = instance.timers.borrow().get(usize::from(*timer_idx)) {
                    timer.restart();
                }
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
            if let Value::String(s) = eval_expression(ctx, &arguments[0])
                && let Some(root) = find_root_instance(ctx)
            {
                // Log and skip if the window adapter can't be created; the
                // same error resurfaces when the window is actually used.
                let result =
                    root.try_window_adapter().map_err(|e| e.to_string()).and_then(|adapter| {
                        adapter
                            .renderer()
                            .register_font_from_path(&std::path::PathBuf::from(s.as_str()))
                            .map_err(|e| format!("Cannot load custom font {}: {e}", s.as_str()))
                    });
                if let Err(err) = result {
                    i_slint_core::debug_log!("{err}");
                }
            }
            Value::Void
        }
        BuiltinFunction::SetupMenuBar => crate::popup::setup_menubar(ctx, arguments),
        BuiltinFunction::ItemFontMetrics => {
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let Some((inst, flat_idx)) = resolve_item_rc_from_ref(ctx, mr)
                && let Some(adapter) = inst.window_adapter_or_default()
            {
                let item_rc =
                    i_slint_core::items::ItemRc::new(vtable::VRc::into_dyn(inst), flat_idx as u32);
                let metrics = i_slint_core::items::slint_text_item_fontmetrics(
                    &adapter,
                    item_rc.borrow(),
                    &item_rc,
                );
                return metrics.into();
            }
            i_slint_core::items::FontMetrics::default().into()
        }
        BuiltinFunction::ItemAbsolutePosition => {
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let Some((inst, flat_idx)) = resolve_item_rc_from_ref(ctx, mr)
            {
                let item_rc =
                    i_slint_core::items::ItemRc::new(vtable::VRc::into_dyn(inst), flat_idx as u32);
                // Map the item's own geometry origin through the ancestor transforms so the
                // result is the item's absolute position (not its parent's). The lowering no
                // longer adds the element's x/y on top (see the ItemAbsolutePosition change).
                return item_rc.map_to_window(item_rc.geometry().origin).to_untyped().into();
            }
            i_slint_core::api::LogicalPosition::default().into()
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            // The argument is a `PropertyReference` to a `Native { prop_name: "" }`,
            // i.e. the item itself; the optional second argument carries the
            // cross-axis constraint (-1 when unconstrained).
            let constraint: f32 = arguments
                .get(1)
                .map(|e| eval_expression(ctx, e).try_into().unwrap_or(-1.))
                .unwrap_or(-1.);
            if let Some(Expression::PropertyReference(mr)) = arguments.first()
                && let Some((inst, flat_idx)) = resolve_item_rc_from_ref(ctx, mr)
                && let Some(adapter) = inst.window_adapter_or_default()
            {
                let item_rc =
                    i_slint_core::items::ItemRc::new(vtable::VRc::into_dyn(inst), flat_idx as u32);
                return item_rc
                    .borrow()
                    .as_ref()
                    .layout_info(
                        llr_to_core_orientation(orient),
                        constraint as _,
                        &adapter,
                        &item_rc,
                    )
                    .into();
            }
            i_slint_core::layout::LayoutInfo::default().into()
        }
        BuiltinFunction::Debug => {
            use i_slint_core::debug_log::*;
            let msg = to_string(ctx, &arguments[0]);
            let root = ctx
                .current
                .as_ref()
                .and_then(|c| c.root.get())
                .and_then(|w| w.upgrade())
                .map(vtable::VRc::into_dyn);
            if let Some(context) = root.as_ref().and_then(i_slint_core::window::context_for_root) {
                context.dispatch_log_message(LogMessage::new(
                    LogMessageSource::SlintCode,
                    None,
                    format_args!("{msg}"),
                ));
            } else {
                log_message(LogMessage::new(
                    LogMessageSource::SlintCode,
                    None,
                    format_args!("{msg}"),
                ));
            }
            Value::Void
        }
        BuiltinFunction::ArrayLength => match eval_expression(ctx, &arguments[0]) {
            // Track the row count so bindings reading `.length` re-evaluate
            // when rows are added or removed.
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
        BuiltinFunction::ShowPopupWindow => crate::popup::show_popup_window(ctx, arguments),
        BuiltinFunction::ClosePopupWindow => crate::popup::close_popup_window(ctx, arguments),
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
            crate::popup::show_popup_menu(ctx, arguments)
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

/// Resolve a `PropertyReference` that targets a native item into the owning
/// `Instance` and the item's flat tree index, for builtins that need a
/// runtime `ItemRc` to hand to core APIs.
pub(crate) fn resolve_item_rc_from_ref(
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
/// `Instance` of the public component. A repeated or conditional sub-tree
/// doesn't have its own window adapter or public component index.
pub(crate) fn find_root_instance(
    ctx: &EvalContext,
) -> Option<vtable::VRc<i_slint_core::item_tree::ItemTreeVTable, crate::instance::Instance>> {
    let current = ctx.current.as_ref()?;
    let mut sub = current.clone();
    loop {
        if let Some(root) = sub.root.get()
            && let Some(inst) = root.upgrade()
            && inst.public_component_index.is_some()
        {
            return Some(inst);
        }
        let parent = sub.parent.upgrade()?;
        sub = Pin::new(parent);
    }
}

/// The root Instance's window adapter, if one can be found or created.
pub(crate) fn find_window_adapter(
    ctx: &EvalContext,
) -> Option<i_slint_core::window::WindowAdapterRc> {
    find_root_instance(ctx)?.window_adapter_or_default()
}

/// Dispatch an `Expression::ItemMemberFunctionCall` (like
/// `TextInput.select-all()`) to the matching native item method by
/// downcasting the runtime `ItemRc` to its concrete item type;
/// the rust codegen resolves the same dispatch at compile time.
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
            "undo" => undo => (),
            "redo" => redo => (),
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
    if let Some(window) = vtable::VRef::downcast_pin::<WindowItem>(item_rc.borrow()) {
        match prop_name.as_str() {
            "hide" => {
                window.hide(&adapter, &item_rc);
                return Value::Void;
            }
            "close" => return Value::Bool(window.close(&adapter, &item_rc)),
            _ => {}
        }
    }
    unimplemented!("ItemMemberFunctionCall `{prop_name}`")
}
