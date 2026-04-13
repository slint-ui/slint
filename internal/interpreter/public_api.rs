// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Name-based bridge between the public API (`get_property`, `invoke`,
//! `set_callback`, …) and the LLR's index-based `MemberReference`s.
//!
//! Each `PublicComponent::public_properties` entry already carries a
//! `MemberReference`, so name lookup is just a linear scan; dispatch
//! forwards to the evaluator helpers in [`crate::eval`].

use crate::Value;
use crate::api::SetPropertyError;
use crate::eval::{EvalContext, invoke_callback, invoke_function, load_property, store_property};
use crate::instance::{Instance, SubComponentInstance};
use i_slint_compiler::langtype::Type;
use i_slint_compiler::llr::{MemberReference, PublicComponent, PublicProperty};
use i_slint_core::item_tree::ItemTreeVTable;
use i_slint_core::model::Model;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use vtable::VRc;

/// Look up a public property by name on the given public component.
/// Normalizes `name` through `normalize_identifier` so
/// snake_case and kebab-case both work.
pub fn find_public_property<'a>(
    public: &'a PublicComponent,
    name: &str,
) -> Option<&'a PublicProperty> {
    let normalized = i_slint_compiler::parser::normalize_identifier(name);
    public.public_properties.get(normalized.as_str())
}

/// Read the value of a public property on `instance`.
pub fn get(instance: &VRc<ItemTreeVTable, Instance>, name: &str) -> Option<Value> {
    let (public, sub) = resolve(instance)?;
    let prop = find_public_property(public, name)?;
    if !prop.ty.is_property_type() {
        return None;
    }
    let ctx = EvalContext::new(sub);
    Some(load_property(&ctx, &prop.prop))
}

/// Write a public property on `instance`.
pub fn set(
    instance: &VRc<ItemTreeVTable, Instance>,
    name: &str,
    mut value: Value,
) -> Result<(), SetPropertyError> {
    let (public, sub) = resolve(instance).ok_or(SetPropertyError::NoSuchProperty)?;
    let prop = find_public_property(public, name).ok_or(SetPropertyError::NoSuchProperty)?;
    if !prop.ty.is_property_type() {
        return Err(SetPropertyError::NoSuchProperty);
    }
    if prop.read_only() {
        return Err(SetPropertyError::AccessDenied);
    }
    // Run every `set_property` through a type check
    // that also auto-fills missing struct fields with their defaults. Do
    // the same here so public API behavior stays consistent.
    if !check_and_coerce(&mut value, &prop.ty) {
        return Err(SetPropertyError::WrongType);
    }
    let ctx = EvalContext::new(sub);
    store_property(&ctx, &prop.prop, value);
    Ok(())
}

/// Return true if `value` matches `ty` — and coerce it in place when useful
/// (struct values get missing fields filled with the type's defaults).
pub(crate) fn check_and_coerce(value: &mut Value, ty: &Type) -> bool {
    match ty {
        Type::Void => true,
        Type::Invalid
        | Type::InferredProperty
        | Type::InferredCallback
        | Type::Callback(_)
        | Type::Function(_)
        | Type::ElementReference => false,
        Type::Float32 | Type::Int32 => matches!(value, Value::Number(_)),
        Type::String => matches!(value, Value::String(_)),
        Type::Color | Type::Brush => matches!(value, Value::Brush(_)),
        Type::UnitProduct(_)
        | Type::Duration
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Angle
        | Type::Percent => matches!(value, Value::Number(_)),
        Type::Image => matches!(value, Value::Image(_)),
        Type::Bool => matches!(value, Value::Bool(_)),
        Type::Model => matches!(value, Value::Model(_) | Value::Bool(_) | Value::Number(_)),
        Type::PathData => matches!(value, Value::PathData(_)),
        Type::Easing => matches!(value, Value::EasingCurve(_)),
        Type::Array(inner) => match value {
            Value::Model(m) => {
                let mut ok = true;
                for i in 0..m.row_count() {
                    if let Some(mut v) = m.row_data(i) {
                        if !check_and_coerce(&mut v, inner) {
                            ok = false;
                            break;
                        }
                    }
                }
                ok
            }
            _ => false,
        },
        Type::Struct(s) => {
            let Value::Struct(str_value) = value else { return false };
            // Every provided key must be declared on the struct and have the
            // right type.
            let keys: Vec<String> = str_value.iter().map(|(k, _)| k.to_string()).collect();
            for k in keys {
                let Some(field_ty) = s.fields.get(k.as_str()) else {
                    return false;
                };
                let Some(v) = str_value.get_field(&k).cloned() else { continue };
                let mut v = v;
                if !check_and_coerce(&mut v, field_ty) {
                    return false;
                }
                str_value.set_field(k, v);
            }
            // Fill any declared field that wasn't provided with the type
            // default so downstream consumers always see a complete struct.
            for (k, field_ty) in s.fields.iter() {
                if str_value.get_field(k.as_str()).is_none() {
                    str_value.set_field(k.to_string(), default_value_for_ty(field_ty));
                }
            }
            true
        }
        Type::Enumeration(en) => {
            matches!(value, Value::EnumerationValue(name, _) if name == en.name.as_str())
        }
        Type::Keys => matches!(value, Value::Keys(_)),
        Type::LayoutCache => matches!(value, Value::LayoutCache(_)),
        Type::ArrayOfU16 => matches!(value, Value::ArrayOfU16(_)),
        Type::ComponentFactory => matches!(value, Value::ComponentFactory(_)),
        Type::StyledText => matches!(value, Value::StyledText(_)),
    }
}

fn default_value_for_ty(ty: &Type) -> Value {
    crate::eval::default_value_for_type(ty)
}

/// Invoke a public callback or function by name.
pub fn invoke(
    instance: &VRc<ItemTreeVTable, Instance>,
    name: &str,
    args: &[Value],
) -> Option<Value> {
    use i_slint_compiler::langtype::Type;
    let (public, sub) = resolve(instance)?;
    let prop = find_public_property(public, name)?;
    // Only callbacks and functions are callable; propagate a miss for
    // anything else so the public API surface a `NoSuchCallable` error.
    if !matches!(&prop.ty, Type::Callback(_) | Type::Function(_)) {
        return None;
    }
    let mut ctx = EvalContext::new(sub);
    Some(if matches!(&prop.ty, Type::Function(_)) || prop.prop.is_function() {
        invoke_function(&mut ctx, &prop.prop, args.to_vec())
    } else {
        invoke_callback(&mut ctx, &prop.prop, args)
    })
}

/// Install a host-side handler on a public callback.
///
/// Host handlers take the callback args as a flat `&[Value]` and return a
/// `Value`; they're adapted to the sub-component's
/// `Callback<(Vec<Value>,), Value>` shape before being installed.
pub fn set_callback(
    instance: &VRc<ItemTreeVTable, Instance>,
    name: &str,
    handler: Box<dyn Fn(&[Value]) -> Value>,
) -> Result<(), ()> {
    let (public, sub) = resolve(instance).ok_or(())?;
    let prop = find_public_property(public, name).ok_or(())?;
    match &prop.prop {
        MemberReference::Relative { parent_level, local_reference } => {
            let target = walk_to(sub, *parent_level, &local_reference.sub_component_path);
            match &local_reference.reference {
                i_slint_compiler::llr::LocalMemberIndex::Callback(idx) => {
                    let cb = Pin::as_ref(&target.callbacks[*idx]);
                    cb.set_handler(move |(args,): &(Vec<Value>,)| handler(args));
                    Ok(())
                }
                i_slint_compiler::llr::LocalMemberIndex::Native {
                    item_index, prop_name, ..
                } => {
                    Pin::as_ref(&target.items[*item_index]).set_callback_handler(prop_name, handler)
                }
                _ => Err(()),
            }
        }
        MemberReference::Global { global_index, member } => {
            // An alias like `callback foo <=> Glo.bar` surfaces as a
            // public property whose `prop` is a global reference. Route
            // directly to the matching `GlobalInstance::callbacks` slot.
            let global_inst = instance.globals.get(*global_index).ok_or(())?;
            let i_slint_compiler::llr::LocalMemberIndex::Callback(idx) = member else {
                return Err(());
            };
            let cb = Pin::as_ref(&global_inst.callbacks[*idx]);
            cb.set_handler(move |(args,): &(Vec<Value>,)| handler(args));
            Ok(())
        }
    }
}

fn resolve(
    instance: &VRc<ItemTreeVTable, Instance>,
) -> Option<(&PublicComponent, Pin<Rc<SubComponentInstance>>)> {
    let cu = &instance.root_sub_component.compilation_unit;
    let public_index = instance.public_component_index?;
    let public = cu.public_components.get(public_index)?;
    Some((public, instance.root_sub_component.clone()))
}

/// Name-based lookup of a public property on an exported global singleton.
/// Returns the looked-up property plus the runtime `GlobalInstance`.
fn resolve_global<'a>(
    instance: &'a VRc<ItemTreeVTable, Instance>,
    global_name: &str,
    prop_name: &str,
) -> Option<(&'a PublicProperty, Rc<crate::globals::GlobalInstance>)> {
    let cu = &instance.root_sub_component.compilation_unit;
    let (_global, global_instance) = instance.globals.find_by_name(cu, global_name)?;
    let global_instance = global_instance.clone();
    let needle = i_slint_compiler::parser::normalize_identifier(prop_name);
    let global = &cu.globals[global_instance.global_idx];
    let prop = global.public_properties.get(needle.as_str())?;
    Some((prop, global_instance))
}

/// Resolve a public global property's underlying `(GlobalInstance,
/// LocalMemberIndex)`. Handles `data <=> G1.data` aliases that surface as
/// `MemberReference::Global { global_index, member }` referring to a
/// *different* global from the one whose `public_properties` map carries
/// the entry — looking up `properties[idx]` on the source global would
/// otherwise hit an out-of-bounds when the source global has no
/// properties of its own.
fn resolve_global_property(
    instance: &VRc<ItemTreeVTable, Instance>,
    source_inst: Rc<crate::globals::GlobalInstance>,
    prop: &PublicProperty,
) -> Option<(Rc<crate::globals::GlobalInstance>, i_slint_compiler::llr::LocalMemberIndex)> {
    match &prop.prop {
        MemberReference::Global { global_index, member } => {
            let target = instance.globals.get(*global_index)?.clone();
            Some((target, member.clone()))
        }
        MemberReference::Relative { local_reference, .. } => {
            Some((source_inst, local_reference.reference.clone()))
        }
    }
}

/// Read a property on a public global singleton.
pub fn get_global(
    instance: &VRc<ItemTreeVTable, Instance>,
    global_name: &str,
    prop_name: &str,
) -> Option<Value> {
    let (prop, source_inst) = resolve_global(instance, global_name, prop_name)?;
    let (target_inst, member) = resolve_global_property(instance, source_inst, prop)?;
    match member {
        i_slint_compiler::llr::LocalMemberIndex::Property(idx) => {
            Some(Pin::as_ref(&target_inst.properties[idx]).get())
        }
        _ => None,
    }
}

/// Write a property on a public global singleton.
pub fn set_global(
    instance: &VRc<ItemTreeVTable, Instance>,
    global_name: &str,
    prop_name: &str,
    mut value: Value,
) -> Result<(), SetPropertyError> {
    let (prop, source_inst) =
        resolve_global(instance, global_name, prop_name).ok_or(SetPropertyError::NoSuchProperty)?;
    if prop.read_only() {
        return Err(SetPropertyError::AccessDenied);
    }
    if !check_and_coerce(&mut value, &prop.ty) {
        return Err(SetPropertyError::WrongType);
    }
    let (target_inst, member) = resolve_global_property(instance, source_inst, prop)
        .ok_or(SetPropertyError::NoSuchProperty)?;
    match member {
        i_slint_compiler::llr::LocalMemberIndex::Property(idx) => {
            Pin::as_ref(&target_inst.properties[idx]).set(value);
            Ok(())
        }
        _ => Err(SetPropertyError::NoSuchProperty),
    }
}

/// Install a handler on a public callback declared on an exported global
/// singleton.
pub fn set_global_callback(
    instance: &VRc<ItemTreeVTable, Instance>,
    global_name: &str,
    callback_name: &str,
    handler: Box<dyn Fn(&[Value]) -> Value>,
) -> Result<(), ()> {
    let (prop, source_inst) = resolve_global(instance, global_name, callback_name).ok_or(())?;
    let (target_inst, member) = resolve_global_property(instance, source_inst, prop).ok_or(())?;
    match member {
        i_slint_compiler::llr::LocalMemberIndex::Callback(idx) => {
            let cb = Pin::as_ref(&target_inst.callbacks[idx]);
            cb.set_handler(move |(args,): &(Vec<Value>,)| handler(args));
            Ok(())
        }
        _ => Err(()),
    }
}

/// Invoke a public callback or function on an exported global singleton.
pub fn invoke_global(
    instance: &VRc<ItemTreeVTable, Instance>,
    global_name: &str,
    name: &str,
    args: &[Value],
) -> Option<Value> {
    use i_slint_compiler::llr::LocalMemberIndex;
    let (prop, source_inst) = resolve_global(instance, global_name, name)?;
    let (target_inst, member) = resolve_global_property(instance, source_inst, prop)?;
    match member {
        LocalMemberIndex::Callback(idx) => {
            let cb = Pin::as_ref(&target_inst.callbacks[idx]);
            Some(cb.call(&(args.to_vec(),)))
        }
        LocalMemberIndex::Function(fn_idx) => {
            let cu = &instance.root_sub_component.compilation_unit;
            let global = &cu.globals[target_inst.global_idx];
            let function = &global.functions[fn_idx];
            let expr = function.code.clone();
            let mut ctx =
                crate::eval::EvalContext::for_global(std::rc::Rc::downgrade(&instance.globals));
            ctx.function_arguments = args.to_vec();
            Some(crate::eval::eval_expression(&mut ctx, &expr))
        }
        _ => None,
    }
}

fn walk_to(
    start: Pin<Rc<SubComponentInstance>>,
    parent_level: usize,
    path: &[i_slint_compiler::llr::SubComponentInstanceIdx],
) -> Pin<Rc<SubComponentInstance>> {
    let mut current = start;
    for _ in 0..parent_level {
        let parent: Weak<SubComponentInstance> = current.parent.clone();
        current = Pin::new(parent.upgrade().expect("parent vanished"));
    }
    for &idx in path {
        let next = current.sub_components[idx].clone();
        current = next;
    }
    current
}
