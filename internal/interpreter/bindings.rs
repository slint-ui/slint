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
use vtable::{VRc, VWeak};

use i_slint_core::item_tree::ItemTreeVTable;

/// Install every binding declared by the LLR on `instance`. Skips
/// `init_code`; call [`run_init_code_for_instance`] separately when ready.
/// Used by the listview row factory (the listview virtualization measures
/// row heights *before* calling `RepeatedItemTree::init`, so the row's
/// bindings have to be in place by the time the factory closure returns)
/// and by the regular `finalize_instance` flow which runs both phases
/// back-to-back.
pub fn install_bindings_only(instance: &VRc<ItemTreeVTable, Instance>) {
    let weak_instance = VRc::downgrade(instance);
    // Two passes: first install property bindings and two-way links across
    // the whole sub-component tree; then install change trackers and
    // timers. Change trackers read the *current* value of their target on
    // `init()`, so they have to run after every use-site override has
    // landed. If they ran interleaved with the recursive property-init
    // walk, an inner component's tracker would see its default value and
    // then fire on the next event-loop tick when the outer component's
    // `property_init` supersedes it.
    install_property_bindings(&instance.root_sub_component, &weak_instance);
    install_trackers_and_timers(&instance.root_sub_component);
}

pub fn run_init_code_for_instance(instance: &VRc<ItemTreeVTable, Instance>) {
    let weak_instance = VRc::downgrade(instance);
    run_init_code(&instance.root_sub_component, &weak_instance);
}

fn install_property_bindings(
    sub: &Pin<Rc<SubComponentInstance>>,
    weak_instance: &VWeak<ItemTreeVTable, Instance>,
) {
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
        install_property_bindings(nested, weak_instance);
    }

    for (a, b, field_path) in &sc.two_way_bindings {
        install_two_way_binding(a, b, field_path, sub);
    }

    for (target, binding) in &sc.property_init {
        install_property_init(target, binding, sub, &weak_sub, weak_instance);
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

/// Configure each `Timer` declared on `sub` by evaluating its LLR
/// `interval` / `running` / `triggered` expressions on change, and starting
/// / stopping the `i_slint_core::timers::Timer` instance we allocate in
/// `SubComponentInstance::timers`. Mirrors the rust-codegen's
/// `update_timers` entry point.
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
        let target_for_set = target.clone();
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
                let _ = target_for_set;
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
    _weak_instance: &VWeak<ItemTreeVTable, Instance>,
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
            let expr = mut_expression_clone(&binding.expression);
            let weak_sub = weak_sub.clone();
            let callback = Pin::as_ref(&instance.callbacks[*idx]);
            callback.set_handler(move |(args,): &(Vec<Value>,)| -> Value {
                let Some(owner) = weak_sub.upgrade() else { return Value::Void };
                let mut ctx = EvalContext::with_arguments(Pin::new(owner), args.clone());
                eval_expression(&mut ctx, &expr)
            });
        }
        LocalMemberIndex::Property(idx) => {
            let prop = Pin::as_ref(&instance.properties[*idx]);
            install_property_binding(prop, binding, weak_sub);
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
                    let handler: Box<dyn Fn(&[Value]) -> Value> = Box::new(move |args| {
                        let Some(owner) = cb_weak.upgrade() else { return Value::Void };
                        let mut ctx = EvalContext::with_arguments(Pin::new(owner), args.to_vec());
                        eval_expression(&mut ctx, &expr)
                    });
                    let _ = item.set_callback_handler(prop_name, handler);
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
            // Function bodies were lowered into `SubComponent::functions[*].code` in
            // `lower_to_item_tree`; `invoke_function` reads them directly.
        }
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
            let prop = Pin::as_ref(&global.properties[*idx]);
            install_property_binding(prop, binding, weak_sub);
        }
        LocalMemberIndex::Callback(idx) => {
            let expr = mut_expression_clone(&binding.expression);
            let weak_sub = weak_sub.clone();
            let cb = Pin::as_ref(&global.callbacks[*idx]);
            cb.set_handler(move |(args,): &(Vec<Value>,)| -> Value {
                let Some(owner) = weak_sub.upgrade() else { return Value::Void };
                let mut ctx = EvalContext::with_arguments(Pin::new(owner), args.clone());
                eval_expression(&mut ctx, &expr)
            });
        }
        LocalMemberIndex::Function(_) | LocalMemberIndex::Native { .. } => {}
    }
}

fn install_property_binding(
    prop: Pin<&Property<Value>>,
    binding: &llr::BindingExpression,
    weak_sub: &Weak<SubComponentInstance>,
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
        // State binding: the expression returns an i32 (the state index).
        // `set_state_binding` installs a BindingCallable that tracks
        // previous_state and change_time inside the Value::Struct.
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

    match animation_for_binding(binding.animation.as_ref(), weak_sub.clone()) {
        AnimatedBindingKind::NotAnimated => prop.set_binding(move || binding_fn()),
        AnimatedBindingKind::Animation(anim_fn) => {
            prop.set_animated_binding(move || binding_fn(), move || (anim_fn(), None));
        }
        AnimatedBindingKind::Transition(transition_fn) => {
            prop.set_animated_binding(
                move || binding_fn(),
                move || {
                    let (anim, change_time) = transition_fn();
                    (anim, Some(change_time))
                },
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
        anim.easing = curve.clone();
    }
    if let Some(direction) = s.get_field("direction") {
        if let Ok(parsed) = direction.clone().try_into() {
            anim.direction = parsed;
        }
    }
    anim
}

fn install_two_way_binding(
    a: &MemberReference,
    b: &MemberReference,
    field_path: &[smol_str::SmolStr],
    sub: &Pin<Rc<SubComponentInstance>>,
) {
    let Some(pa) = prepare_two_way(a, sub) else { return };
    let Some(pb) = prepare_two_way(b, sub) else { return };

    if field_path.is_empty() {
        Property::link_two_way(pa.as_ref(), pb.as_ref());
        return;
    }

    // `pa` is the leaf property; `pb` is the struct that contains it.
    // Map the struct value through `field_path` when reading and write
    // back into the same field path when the leaf changes.
    let path: Vec<smol_str::SmolStr> = field_path.iter().cloned().collect();
    let path_get = path.clone();
    let path_set = path.clone();
    Property::link_two_way_with_map(
        pb.as_ref(),
        pa.as_ref(),
        move |s| extract_field(s.clone(), &path_get).unwrap_or(Value::Void),
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
            // here. Walk up to the root instance, fetch the matching
            // `GlobalInstance` and return its property storage so that
            // `link_two_ways` can wire both sides together.
            let root = sub.root.get().and_then(|w| w.upgrade())?;
            let global_inst = root.globals.get(*global_index)?;
            if let LocalMemberIndex::Property(idx) = member {
                Some(global_inst.properties[*idx].clone())
            } else {
                None
            }
        }
    }
}

fn run_init_code(
    sub: &Pin<Rc<SubComponentInstance>>,
    _weak_instance: &VWeak<ItemTreeVTable, Instance>,
) {
    // Run nested sub-components' init code first, matching the rust code
    // generator's user_init order: each nested sub-component's user_init is
    // invoked before the enclosing component's own init_code. This lets the
    // outer component's use-site overrides (which live in its own init_code)
    // observe the inner component's default initialization.
    for nested in &sub.sub_components {
        run_init_code(nested, _weak_instance);
    }
    let cu = sub.compilation_unit.clone();
    let sc = &cu.sub_components[sub.sub_component_idx];
    for e in &sc.init_code {
        let expr = mut_expression_clone(e);
        let mut ctx = EvalContext::new(sub.clone());
        eval_expression(&mut ctx, &expr);
    }
}

fn walk_parent(
    start: &Pin<Rc<SubComponentInstance>>,
    level: usize,
) -> Pin<Rc<SubComponentInstance>> {
    let mut current = start.clone();
    for _ in 0..level {
        let parent = current.parent.upgrade().expect("parent vanished during binding install");
        current = Pin::new(parent);
    }
    current
}

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

fn mut_expression_clone(e: &MutExpression) -> Expression {
    e.borrow().clone()
}
