// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Install property bindings, callback handlers, two-way bindings, change
//! callbacks and `init_code` on an already-allocated [`Instance`].
//!
//! Called from [`Instance::new`] right after the `SubComponentInstance` tree
//! has been wired up. Walks the tree recursively, visiting each sub-component
//! (including nested ones) and copying its LLR entries onto the runtime
//! allocations.

use crate::Value;
use crate::eval::{EvalContext, eval_expression};
use crate::instance::{Instance, SubComponentInstance};
use i_slint_compiler::llr::{
    self, Animation, Expression, LocalMemberIndex, MemberReference, MutExpression, SubComponent,
};
use i_slint_core::Property;
use i_slint_core::items::PropertyAnimation;
use i_slint_core::rtti::AnimatedBindingKind;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use vtable::VRc;

use i_slint_core::item_tree::ItemTreeVTable;

/// Install every binding declared by the LLR on `instance`, skipping
/// `init_code`; call [`run_init_code_for_instance`] separately.
/// The listview row factory needs this split: virtualization measures row
/// heights before `RepeatedItemTree::init` runs, so the row's bindings must
/// be in place when the factory closure returns.
pub fn install_bindings_only(instance: &VRc<ItemTreeVTable, Instance>) {
    // Two passes: property bindings and two-way links across the whole
    // sub-component tree first, then change trackers and timers.
    // A change tracker reads its target's current value on `init()`, so it
    // must run after every use-site override has landed; otherwise an inner
    // component's tracker would see the default value and fire when the
    // outer component's `property_init` supersedes it.
    install_property_bindings(&instance.root_sub_component);
    install_trackers_and_timers(&instance.root_sub_component);
}

pub fn run_init_code_for_instance(instance: &VRc<ItemTreeVTable, Instance>) {
    run_init_code(&instance.root_sub_component);
}

fn install_property_bindings(sub: &Pin<Rc<SubComponentInstance>>) {
    let cu = &sub.compilation_unit;
    let sc: &SubComponent = &cu.sub_components[sub.sub_component_idx];
    let weak_sub = Rc::downgrade(&Pin::into_inner(sub.clone()));

    // Match the rust code generator's `init` ordering:
    //   1. Initialize each nested sub-component (including its own two-way
    //      links and property_init).
    //   2. Install this component's two-way bindings. They reference nested
    //      properties that now carry their defaults, so `link_two_way` can
    //      carry the other side's value over correctly.
    //   3. Install this component's property_init, which overrides the
    //      defaults (including any nested default that a use-site supersedes).
    for nested in &sub.sub_components {
        install_property_bindings(nested);
    }

    // Pre-init code (custom font registration) runs before the two-way
    // bindings and property_init.
    for e in &sc.pre_init_code {
        let expr = mut_expression_clone(e);
        let mut ctx = EvalContext::new(sub.clone());
        eval_expression(&mut ctx, &expr);
    }

    for twb in &sc.two_way_bindings {
        install_two_way_binding(twb, sub);
    }

    for (target, binding) in &sc.property_init {
        install_property_init(target, binding, sub, &weak_sub);
    }

    install_repeater_model_bindings(sub, &weak_sub);
}

fn install_trackers_and_timers(sub: &Pin<Rc<SubComponentInstance>>) {
    for nested in &sub.sub_components {
        install_trackers_and_timers(nested);
    }
    let weak_sub = Rc::downgrade(&Pin::into_inner(sub.clone()));
    install_change_callbacks(sub, &weak_sub);
    install_timers(sub, &weak_sub);
}

/// Configure each `Timer` declared on `sub`: evaluate its LLR `interval` /
/// `running` / `triggered` expressions and start or stop the matching
/// `i_slint_core::timers::Timer` in `SubComponentInstance::timers`.
/// Mirrors the rust codegen's `update_timers` entry point.
fn install_timers(sub: &Pin<Rc<SubComponentInstance>>, weak_sub: &Weak<SubComponentInstance>) {
    let cu = &sub.compilation_unit;
    let sc: &SubComponent = &cu.sub_components[sub.sub_component_idx];
    if sc.timers.is_empty() {
        return;
    }
    let update = {
        let weak_sub = weak_sub.clone();
        move || {
            let Some(owner_rc) = weak_sub.upgrade() else { return };
            let owner = Pin::new(owner_rc);
            let cu = owner.compilation_unit.clone();
            let sc = &cu.sub_components[owner.sub_component_idx];
            for (idx, t) in sc.timers.iter().enumerate() {
                let running_expr = t.running.borrow().clone();
                let interval_expr = t.interval.borrow().clone();
                let triggered_expr = t.triggered.borrow().clone();
                let mut ctx = EvalContext::new(owner.clone());
                let running = matches!(eval_expression(&mut ctx, &running_expr), Value::Bool(true));
                if !running {
                    if let Some(timer) = owner.timers.borrow().get(idx) {
                        timer.stop();
                    }
                    continue;
                }
                let mut ctx = EvalContext::new(owner.clone());
                let interval_ms: i64 =
                    eval_expression(&mut ctx, &interval_expr).try_into().unwrap_or(0);
                if interval_ms < 0 {
                    if let Some(timer) = owner.timers.borrow().get(idx) {
                        timer.stop();
                    }
                    continue;
                }
                let interval = std::time::Duration::from_millis(interval_ms as u64);
                let timers = owner.timers.borrow();
                let Some(timer) = timers.get(idx) else { continue };
                if timer.running() && timer.interval() == interval {
                    continue;
                }
                let weak = Rc::downgrade(&Pin::into_inner(owner.clone()));
                let expr = triggered_expr.clone();
                timer.start(i_slint_core::timers::TimerMode::Repeated, interval, move || {
                    let Some(owner) = weak.upgrade() else { return };
                    let mut ctx = EvalContext::new(Pin::new(owner));
                    eval_expression(&mut ctx, &expr);
                });
            }
        }
    };

    // Run once to start timers whose `running` is already true, then hook
    // every `running` / `interval` expression into its own change tracker
    // so mutations re-run the update step.
    update();
    let mut trackers = sub.change_trackers.borrow_mut();
    for t in &sc.timers {
        for expr in [&t.running, &t.interval] {
            let tracker = i_slint_core::properties::ChangeTracker::default();
            let get_expr = expr.borrow().clone();
            let weak_get = weak_sub.clone();
            let update = update.clone();
            tracker.init(
                (),
                move |()| -> Value {
                    let Some(owner) = weak_get.upgrade() else { return Value::Void };
                    let mut ctx = EvalContext::new(Pin::new(owner));
                    eval_expression(&mut ctx, &get_expr)
                },
                move |(), _| {
                    update();
                },
            );
            trackers.push(tracker);
        }
    }
}

fn install_change_callbacks(
    sub: &Pin<Rc<SubComponentInstance>>,
    weak_sub: &Weak<SubComponentInstance>,
) {
    let cu = &sub.compilation_unit;
    let sc: &SubComponent = &cu.sub_components[sub.sub_component_idx];
    if sc.change_callbacks.is_empty() {
        return;
    }
    let mut trackers = Vec::new();
    for (target, expr) in &sc.change_callbacks {
        let target = target.clone();
        let expr = mut_expression_clone(expr);
        let weak_get = weak_sub.clone();
        let weak_set = weak_sub.clone();
        let expr_for_set = expr.clone();
        let tracker = i_slint_core::properties::ChangeTracker::default();
        tracker.init(
            (),
            move |()| -> Value {
                let Some(owner) = weak_get.upgrade() else { return Value::Void };
                let ctx = EvalContext::new(Pin::new(owner));
                crate::eval::load_property(&ctx, &target)
            },
            move |(), _| {
                let Some(owner) = weak_set.upgrade() else { return };
                let mut ctx = EvalContext::new(Pin::new(owner));
                eval_expression(&mut ctx, &expr_for_set);
            },
        );
        trackers.push(tracker);
    }
    sub.change_trackers.borrow_mut().extend(trackers);
}

fn install_repeater_model_bindings(
    sub: &Pin<Rc<SubComponentInstance>>,
    weak_sub: &Weak<SubComponentInstance>,
) {
    let cu = &sub.compilation_unit;
    let sc: &SubComponent = &cu.sub_components[sub.sub_component_idx];
    for (idx, repeated) in sc.repeated.iter_enumerated() {
        let repeater = &sub.repeaters[idx];
        let expr = mut_expression_clone(&repeated.model);
        let weak_sub = weak_sub.clone();
        if repeater.is_conditional() {
            repeater.set_condition_binding(move || {
                let Some(owner) = weak_sub.upgrade() else { return false };
                let mut ctx = EvalContext::new(Pin::new(owner));
                match eval_expression(&mut ctx, &expr) {
                    Value::Bool(b) => b,
                    Value::Number(n) => n > 0.0,
                    _ => false,
                }
            });
        } else {
            repeater.set_model_binding(move || {
                let Some(owner) = weak_sub.upgrade() else {
                    return i_slint_core::model::ModelRc::default();
                };
                let mut ctx = EvalContext::new(Pin::new(owner));
                let v = eval_expression(&mut ctx, &expr);
                match v {
                    Value::Model(m) => m,
                    other => i_slint_core::model::ModelRc::new(
                        crate::value_model::ValueModel::new(other),
                    ),
                }
            });
        }
    }
}

fn install_property_init(
    target: &MemberReference,
    binding: &llr::BindingExpression,
    sub: &Pin<Rc<SubComponentInstance>>,
    weak_sub: &Weak<SubComponentInstance>,
) {
    if let MemberReference::Global { global_index, member } = target {
        install_global_property_init(*global_index, member, binding, weak_sub);
        return;
    }
    let MemberReference::Relative { parent_level, local_reference } = target else {
        unreachable!()
    };
    assert_eq!(*parent_level, 0, "property_init targets must be local to the sub-component");
    let instance = walk_sub_path(sub.clone(), &local_reference.sub_component_path);

    match &local_reference.reference {
        LocalMemberIndex::Callback(idx) => {
            let callback = Pin::as_ref(&instance.callbacks[*idx]);
            let arg_types = instance.compilation_unit.sub_components[instance.sub_component_idx]
                .callbacks[*idx]
                .args
                .clone();
            let handler = make_callback_handler(
                weak_sub.clone(),
                mut_expression_clone(&binding.expression),
                arg_types,
            );
            callback.set_handler(move |args: &[Value]| handler(args));
        }
        LocalMemberIndex::Property(idx) => {
            let prop = Pin::as_ref(&instance.properties[*idx]);
            let cu = &instance.compilation_unit;
            let ty = &cu.sub_components[instance.sub_component_idx].properties[*idx].ty;
            install_property_binding(prop, binding, weak_sub, ty);
        }
        LocalMemberIndex::Native { item_index, prop_name, kind } => {
            // The lowering pass already disambiguates rtti properties,
            // callbacks and member functions via `NativeMemberKind`, so
            // dispatch on `kind` rather than probing the rtti tables by
            // name.
            let item = Pin::as_ref(&instance.items[*item_index]);
            match kind {
                i_slint_compiler::llr::NativeMemberKind::Callback => {
                    let expr = mut_expression_clone(&binding.expression);
                    let cb_weak = weak_sub.clone();
                    let cu = &instance.compilation_unit;
                    let sc = &cu.sub_components[instance.sub_component_idx];
                    let arg_types = match sc.items[*item_index].ty.lookup_property(prop_name) {
                        Some(i_slint_compiler::langtype::Type::Callback(f)) => f.args.clone(),
                        _ => Vec::new(),
                    };
                    let _ = item.set_callback_handler(
                        prop_name,
                        Box::new(make_callback_handler(cb_weak, expr, arg_types)),
                    );
                }
                i_slint_compiler::llr::NativeMemberKind::Property => {
                    let expr = mut_expression_clone(&binding.expression);
                    let weak_sub_eval = weak_sub.clone();
                    let closure: Box<dyn Fn() -> Value> = Box::new(move || {
                        let Some(owner) = weak_sub_eval.upgrade() else { return Value::Void };
                        let mut ctx = EvalContext::new(Pin::new(owner));
                        eval_expression(&mut ctx, &expr)
                    });
                    let animation_kind =
                        animation_for_binding(binding.animation.as_ref(), weak_sub.clone());
                    let _ = item.set_property_binding(prop_name, closure, animation_kind);
                }
                i_slint_compiler::llr::NativeMemberKind::Function => {
                    // No state to install — `Expression::ItemMemberFunctionCall`
                    // dispatches to the native method on demand.
                }
            }
        }
        LocalMemberIndex::Function(_) => {
            // Function bodies live in `SubComponent::functions[*].code`;
            // `invoke_function` reads them directly.
        }
        LocalMemberIndex::Timer(_) => unreachable!("a timer is not a binding target"),
    }
}

/// Install a property_init entry that targets a global property or callback.
/// The binding expression is evaluated in the owning sub-component's context.
fn install_global_property_init(
    global_index: i_slint_compiler::llr::GlobalIdx,
    member: &LocalMemberIndex,
    binding: &llr::BindingExpression,
    weak_sub: &Weak<SubComponentInstance>,
) {
    let Some(sub) = weak_sub.upgrade() else { return };
    let globals = sub.root.get().and_then(|w| w.upgrade()).map(|inst| inst.globals.clone());
    let Some(globals) = globals else { return };
    let Some(global) = globals.get(global_index) else { return };

    match member {
        LocalMemberIndex::Property(idx) => {
            if let Some(native) = &global.native {
                let g = &global.compilation_unit.globals[global_index];
                if let Some(prop) =
                    native.as_ref().prepare_property_for_two_way_binding(&g.properties[*idx].name)
                {
                    let ty = &g.properties[*idx].ty;
                    install_property_binding(prop.as_ref(), binding, weak_sub, ty);
                }
                return;
            }
            let prop = Pin::as_ref(&global.properties[*idx]);
            let ty = &sub.compilation_unit.globals[global_index].properties[*idx].ty;
            install_property_binding(prop, binding, weak_sub, ty);
        }
        LocalMemberIndex::Callback(idx) => {
            if let Some(native) = &global.native {
                let g = &global.compilation_unit.globals[global_index];
                let _ = native.as_ref().set_callback_handler(
                    &g.callbacks[*idx].name,
                    Box::new(make_callback_handler(
                        weak_sub.clone(),
                        mut_expression_clone(&binding.expression),
                        g.callbacks[*idx].args.clone(),
                    )),
                );
                return;
            }
            let cb = Pin::as_ref(&global.callbacks[*idx]);
            let handler = make_callback_handler(
                weak_sub.clone(),
                mut_expression_clone(&binding.expression),
                sub.compilation_unit.globals[global_index].callbacks[*idx].args.clone(),
            );
            cb.set_handler(move |args: &[Value]| handler(args));
        }
        LocalMemberIndex::Function(_)
        | LocalMemberIndex::Native { .. }
        | LocalMemberIndex::Timer(_) => {}
    }
}

/// Typed properties in generated code interpolate `int` (i32) and
/// `duration` (i64) values with per-frame rounding; reproduce that when
/// the animation runs on a type-erased `Property<Value>`.
pub(crate) fn animated_value_map(
    ty: &i_slint_compiler::langtype::Type,
) -> Option<fn(Value) -> Value> {
    use i_slint_compiler::langtype::Type;
    matches!(ty, Type::Int32 | Type::Duration).then_some(|v| match v {
        Value::Number(n) => Value::Number(n.round()),
        other => other,
    })
}

fn install_property_binding(
    prop: Pin<&Property<Value>>,
    binding: &llr::BindingExpression,
    weak_sub: &Weak<SubComponentInstance>,
    ty: &i_slint_compiler::langtype::Type,
) {
    let expr = mut_expression_clone(&binding.expression);
    let weak_sub_eval = weak_sub.clone();

    use i_slint_compiler::llr::BindingKind;
    if binding.kind == BindingKind::Constant {
        if let Some(owner) = weak_sub_eval.upgrade() {
            let mut ctx = EvalContext::new(Pin::new(owner));
            prop.set(eval_expression(&mut ctx, &expr));
        }
        return;
    }

    if binding.kind == BindingKind::State {
        // The expression returns the state index; `set_state_binding`
        // tracks previous_state and change_time in the struct value.
        let weak = weak_sub.clone();
        i_slint_core::properties::set_state_binding(prop, move || {
            let Some(owner) = weak.upgrade() else { return 0 };
            let mut ctx = EvalContext::new(Pin::new(owner));
            match eval_expression(&mut ctx, &expr) {
                Value::Number(n) => n as i32,
                _ => 0,
            }
        });
        return;
    }

    let binding_fn: Box<dyn Fn() -> Value> = Box::new(move || {
        let Some(owner) = weak_sub_eval.upgrade() else { return Value::Void };
        let mut ctx = EvalContext::new(Pin::new(owner));
        eval_expression(&mut ctx, &expr)
    });

    match (
        animation_for_binding(binding.animation.as_ref(), weak_sub.clone()),
        animated_value_map(ty),
    ) {
        (AnimatedBindingKind::NotAnimated, _) => prop.set_binding(binding_fn),
        (AnimatedBindingKind::Animation(anim_fn), None) => {
            prop.set_animated_binding(binding_fn, move || (anim_fn(), None));
        }
        (AnimatedBindingKind::Animation(anim_fn), Some(map)) => {
            prop.set_animated_binding_with_map(binding_fn, move || (anim_fn(), None), map);
        }
        (AnimatedBindingKind::Transition(transition_fn), None) => {
            prop.set_animated_binding(binding_fn, move || {
                let (anim, change_time) = transition_fn();
                (anim, Some(change_time))
            });
        }
        (AnimatedBindingKind::Transition(transition_fn), Some(map)) => {
            prop.set_animated_binding_with_map(
                binding_fn,
                move || {
                    let (anim, change_time) = transition_fn();
                    (anim, Some(change_time))
                },
                map,
            );
        }
    }
}

fn animation_for_binding(
    animation: Option<&Animation>,
    weak_sub: Weak<SubComponentInstance>,
) -> AnimatedBindingKind {
    match animation {
        None => AnimatedBindingKind::NotAnimated,
        Some(Animation::Static(expr)) => {
            let expr = expr.clone();
            AnimatedBindingKind::Animation(Box::new(move || -> PropertyAnimation {
                let Some(owner) = weak_sub.upgrade() else { return Default::default() };
                let mut ctx = EvalContext::new(Pin::new(owner));
                value_to_property_animation(eval_expression(&mut ctx, &expr))
            }))
        }
        Some(Animation::Transition(expr)) => {
            let expr = expr.clone();
            AnimatedBindingKind::Transition(Box::new(
                move || -> (PropertyAnimation, i_slint_core::animations::Instant) {
                    let Some(owner) = weak_sub.upgrade() else {
                        return (Default::default(), Default::default());
                    };
                    let mut ctx = EvalContext::new(Pin::new(owner));
                    let v = eval_expression(&mut ctx, &expr);
                    // The transition expression returns a struct {"0": animation, "1": change_time}
                    let Value::Struct(s) = v else {
                        return (Default::default(), Default::default());
                    };
                    let anim = s
                        .get_field("0")
                        .cloned()
                        .map(value_to_property_animation)
                        .unwrap_or_default();
                    let change_time = s
                        .get_field("1")
                        .cloned()
                        .and_then(|v| v.try_into().ok())
                        .unwrap_or_default();
                    (anim, change_time)
                },
            ))
        }
    }
}

/// Convert a `Value::Struct` produced by an LLR animation expression into a
/// native `PropertyAnimation`.
/// Unknown fields are ignored and missing fields fall back to `Default`.
pub(crate) fn value_to_property_animation(v: Value) -> PropertyAnimation {
    let Value::Struct(s) = v else { return PropertyAnimation::default() };
    let mut anim = PropertyAnimation::default();
    if let Some(Value::Number(n)) = s.get_field("delay") {
        anim.delay = *n as i32;
    }
    if let Some(Value::Number(n)) = s.get_field("duration") {
        anim.duration = *n as i32;
    }
    if let Some(Value::Number(n)) = s.get_field("iteration-count") {
        anim.iteration_count = *n as f32;
    }
    if let Some(Value::EasingCurve(curve)) = s.get_field("easing") {
        anim.easing = *curve;
    }
    if let Some(direction) = s.get_field("direction")
        && let Ok(parsed) = direction.clone().try_into()
    {
        anim.direction = parsed;
    }
    if let Some(Value::Bool(b)) = s.get_field("enabled") {
        anim.enabled = *b;
    }
    anim
}

fn install_two_way_binding(
    twb: &i_slint_compiler::llr::TwoWayBinding,
    sub: &Pin<Rc<SubComponentInstance>>,
) {
    let a: MemberReference = twb.prop1.clone().into();
    let field_path: &[smol_str::SmolStr] = &twb.field_access;
    let Some(pa) = prepare_two_way(&a, sub) else { return };

    if let Some(index_prop) = twb.is_model {
        // `prop2` names the `model_data` property of the enclosing `for`'s
        // body sub-component, `parent_level` hops up from here. Bind the
        // leaf property directly to the model row so writes go through
        // `set_row_data` and reads see external model updates.
        let MemberReference::Relative { parent_level, local_reference } = &twb.prop2 else {
            return;
        };
        let LocalMemberIndex::Property(data_prop) = local_reference.reference else {
            return;
        };
        let body = walk_parent(sub, *parent_level);
        let body_weak = Rc::downgrade(&Pin::into_inner(body));
        let path_get: Vec<smol_str::SmolStr> = field_path.to_vec();
        let path_set = path_get.clone();
        pa.as_ref().link_two_way_to_model_data(
            body_weak,
            move |body_weak: &std::rc::Weak<SubComponentInstance>| {
                let body = Pin::new(body_weak.upgrade()?);
                let data = Pin::as_ref(&body.properties[data_prop]).get();
                if path_get.is_empty() {
                    Some(data)
                } else {
                    extract_field(data, &path_get).filter(|v| !matches!(v, Value::Void))
                }
            },
            move |body_weak: &std::rc::Weak<SubComponentInstance>, value: &Value| {
                let Some(body) = body_weak.upgrade().map(Pin::new) else { return };
                let Some((parent_weak, rep_idx)) = body.repeated_in.get() else { return };
                let Some(parent) = parent_weak.upgrade().map(Pin::new) else { return };
                let index: usize = match Pin::as_ref(&body.properties[index_prop]).get() {
                    Value::Number(n) => n as usize,
                    _ => return,
                };
                // Short-circuit identical writes to avoid spurious change
                // notifications.
                let data = if path_set.is_empty() {
                    let current = Pin::as_ref(&body.properties[data_prop]).get();
                    if &current == value {
                        return;
                    }
                    value.clone()
                } else {
                    let mut data = Pin::as_ref(&body.properties[data_prop]).get();
                    if extract_field(data.clone(), &path_set).as_ref() == Some(value) {
                        return;
                    }
                    replace_field(&mut data, &path_set, value.clone());
                    data
                };
                parent.repeaters[*rep_idx].model_set_row_data(index, data);
            },
        );
        return;
    }

    let Some(pb) = prepare_two_way(&twb.prop2, sub) else { return };

    if field_path.is_empty() {
        Property::link_two_way(pa.as_ref(), pb.as_ref());
        return;
    }

    // `pa` is the leaf property; `pb` is the struct that contains it.
    // Map the struct value through `field_path` when reading and write
    // back into the same field path when the leaf changes.
    let path: Vec<smol_str::SmolStr> = field_path.to_vec();
    let path_get = path.clone();
    let path_set = path.clone();
    // A struct value read before its binding ran (or set with missing
    // fields) must yield the leaf type's default, never Void — native
    // typed properties abort on a Void conversion.
    let leaf_default = member_property_ty(&twb.prop2, sub)
        .map(|ty| crate::eval::default_value_for_type(&field_leaf_ty(ty, field_path)))
        .unwrap_or(Value::Void);
    Property::link_two_way_with_map(
        pb.as_ref(),
        pa.as_ref(),
        move |s| {
            extract_field(s.clone(), &path_get)
                .filter(|v| !matches!(v, Value::Void))
                .unwrap_or_else(|| leaf_default.clone())
        },
        move |s, v| {
            replace_field(s, &path_set, v.clone());
        },
    );
}

/// Walk `s.path[0].path[1]...` and return the leaf field.
fn extract_field(value: Value, path: &[smol_str::SmolStr]) -> Option<Value> {
    let mut current = value;
    for p in path {
        let Value::Struct(s) = current else { return None };
        current = s.get_field(p.as_str()).cloned()?;
    }
    Some(current)
}

/// Walk `s.path[0].path[1]...` and write `new_leaf` into the leaf field.
fn replace_field(s: &mut Value, path: &[smol_str::SmolStr], new_leaf: Value) {
    if path.is_empty() {
        *s = new_leaf;
        return;
    }
    let Value::Struct(top) = s else { return };
    let head = path[0].as_str();
    let mut child = top.get_field(head).cloned().unwrap_or(Value::Void);
    replace_field(&mut child, &path[1..], new_leaf);
    top.set_field(head.to_string(), child);
}

/// The LLR-declared type of a member reference (properties only).
fn member_property_ty(
    mr: &MemberReference,
    sub: &Pin<Rc<SubComponentInstance>>,
) -> Option<i_slint_compiler::langtype::Type> {
    match mr {
        MemberReference::Relative { parent_level, local_reference } => {
            let base = walk_parent(sub, *parent_level);
            let instance = walk_sub_path(base, &local_reference.sub_component_path);
            let cu = &instance.compilation_unit;
            let sc = &cu.sub_components[instance.sub_component_idx];
            match &local_reference.reference {
                LocalMemberIndex::Property(idx) => Some(sc.properties[*idx].ty.clone()),
                LocalMemberIndex::Native { item_index, prop_name, .. } => {
                    sc.items[*item_index].ty.lookup_property(prop_name).cloned()
                }
                _ => None,
            }
        }
        MemberReference::Global { global_index, member } => {
            let root = sub.root.get().and_then(|w| w.upgrade())?;
            let cu = root.root_sub_component.compilation_unit.clone();
            let global = &cu.globals[*global_index];
            if let LocalMemberIndex::Property(idx) = member {
                Some(global.properties[*idx].ty.clone())
            } else {
                None
            }
        }
    }
}

/// Descend `path` through struct field types, starting at `ty`.
fn field_leaf_ty(
    mut ty: i_slint_compiler::langtype::Type,
    path: &[smol_str::SmolStr],
) -> i_slint_compiler::langtype::Type {
    for f in path {
        let i_slint_compiler::langtype::Type::Struct(s) = &ty else { break };
        match s.fields.get(f.as_str()) {
            Some(t) => ty = t.clone(),
            None => break,
        }
    }
    ty
}

fn prepare_two_way(
    mr: &MemberReference,
    sub: &Pin<Rc<SubComponentInstance>>,
) -> Option<Pin<Rc<Property<Value>>>> {
    match mr {
        MemberReference::Relative { parent_level, local_reference } => {
            let base = walk_parent(sub, *parent_level);
            let instance = walk_sub_path(base, &local_reference.sub_component_path);
            match &local_reference.reference {
                LocalMemberIndex::Property(idx) => Some(instance.properties[*idx].clone()),
                LocalMemberIndex::Native { item_index, prop_name, .. } => {
                    Pin::as_ref(&instance.items[*item_index])
                        .prepare_property_for_two_way_binding(prop_name)
                }
                _ => None,
            }
        }
        MemberReference::Global { global_index, member } => {
            // A `data <=> Glo.x` two-way binding lands a `Global` reference
            // here; return the matching `GlobalInstance`'s property storage
            // so both sides can be linked.
            let root = sub.root.get().and_then(|w| w.upgrade())?;
            let global_inst = root.globals.get(*global_index)?;
            if let LocalMemberIndex::Property(idx) = member {
                if let Some(native) = &global_inst.native {
                    let g = &global_inst.compilation_unit.globals[global_inst.global_idx];
                    return native
                        .as_ref()
                        .prepare_property_for_two_way_binding(&g.properties[*idx].name);
                }
                Some(global_inst.properties[*idx].clone())
            } else {
                None
            }
        }
    }
}

fn run_init_code(sub: &Pin<Rc<SubComponentInstance>>) {
    // Nested sub-components run their init code first, matching the rust
    // codegen's user_init order, so the outer component's use-site overrides
    // (in its own init_code) observe the inner component's initialization.
    for nested in &sub.sub_components {
        run_init_code(nested);
    }
    let cu = sub.compilation_unit.clone();
    let sc = &cu.sub_components[sub.sub_component_idx];
    for e in &sc.init_code {
        let expr = mut_expression_clone(e);
        let mut ctx = EvalContext::new(sub.clone());
        eval_expression(&mut ctx, &expr);
    }
}

use crate::eval::{walk_parent, walk_sub_path};

fn mut_expression_clone(e: &MutExpression) -> Expression {
    e.borrow().clone()
}

/// A handler that evaluates `expr` with the call's arguments in `weak`'s
/// scope; `Value::Void` once the owner is gone.
fn make_callback_handler(
    weak: Weak<SubComponentInstance>,
    expr: Expression,
    arg_types: Vec<i_slint_compiler::langtype::Type>,
) -> impl Fn(&[Value]) -> Value + 'static {
    move |args| {
        let Some(owner) = weak.upgrade() else { return Value::Void };
        let mut ctx = EvalContext::with_arguments(Pin::new(owner), args.to_vec());
        ctx.function_arg_types = arg_types.clone();
        eval_expression(&mut ctx, &expr)
    }
}
