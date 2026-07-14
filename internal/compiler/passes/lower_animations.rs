// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass validates and lowers Animation elements (TweenAnimation, DelayAnimation, etc.)

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{
    BindingExpression, BuiltinFunction, Callable, Expression, NamedReference,
};
use crate::langtype::ElementType;
use crate::object_tree::*;
use crate::symbol_counters::SymbolCounters;
use crate::typeregister::TypeRegister;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Lower and validate animation elements
pub fn lower_animations(
    component: &Rc<Component>,
    type_register: &Rc<RefCell<TypeRegister>>,
    diag: &mut BuildDiagnostics,
) {
    // replace <animation>.start/end with <animation>.running = true/false, followed by a
    // synchronous `update_animations()` so updates happen on this tick
    visit_all_expressions(component, |e, _| {
        e.visit_recursive_mut(&mut |e| {
            if let Expression::FunctionCall { function, arguments, .. } = e
                && let Callable::Builtin(
                    BuiltinFunction::StartAnimation | BuiltinFunction::StopAnimation,
                ) = function
                && let [Expression::ElementReference(animation)] = arguments.as_slice()
            {
                let assign_running = Expression::SelfAssignment {
                    lhs: Box::new(Expression::PropertyReference(NamedReference::new(
                        &animation.upgrade().unwrap(),
                        SmolStr::new_static("running"),
                    ))),
                    rhs: Box::new(Expression::BoolLiteral(matches!(
                        function,
                        Callable::Builtin(BuiltinFunction::StartAnimation)
                    ))),
                    op: '=',
                    node: None,
                };
                let update_animations = Expression::FunctionCall {
                    function: BuiltinFunction::UpdateAnimations.into(),
                    arguments: Vec::new(),
                    source_location: None,
                };
                *e = Expression::CodeBlock(vec![assign_running, update_animations]);
            }
        });
    });

    // validate and lower animations
    recurse_elem_including_sub_components_no_borrow(
        component,
        &None,
        &mut |elem, parent_element: &Option<ElementRc>| {
            let anim_type = get_anim_type(&elem.borrow().base_type);

            if anim_type.is_some() {
                lower_animation(
                    elem,
                    anim_type.unwrap(),
                    parent_element.as_ref(),
                    type_register,
                    diag,
                );
            }
            Some(elem.clone())
        },
    )
}

fn get_anim_type(anim_base_type: &ElementType) -> Option<AnimationType> {
    match anim_base_type {
        ElementType::Builtin(base) => {
            if base.name == "TweenAnimation" {
                Some(AnimationType::Tween)
            } else if base.name == "DelayAnimation" {
                Some(AnimationType::Delay)
            } else if base.name == "ParallelAnimation" {
                Some(AnimationType::Parallel)
            } else if base.name == "SequentialAnimation" {
                Some(AnimationType::Sequential)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Validate that from/to properties match the target property in animation components
fn validate_animation_properties(
    animatable_props: &BTreeMap<SmolStr, RefCell<BindingExpression>>,
    type_register: &Rc<RefCell<TypeRegister>>,
    diag: &mut BuildDiagnostics,
) {
    let target = animatable_props.get("target");
    let from_prop = animatable_props.get("from");
    let to_prop = animatable_props.get("to");

    if let Some(target_binding) = target {
        if let Ok(target_expr) = target_binding.try_borrow() {
            let target_type = target_expr.ty();

            // Validate that target type is animatable
            let type_register_ref = type_register.borrow();
            let elem_type =
                type_register_ref.property_animation_type_for_property(target_type.clone());
            // this simply checks if it can be animated
            match elem_type {
                ElementType::Builtin(_) => (), // returns the PropertyAnimation object
                _ => {
                    let msg = format!("target type {:?} isn't animatable", target_type);
                    diag.push_error(msg, &target_expr.span);
                }
            };

            // Validate from property matches target type
            if let Some(from_binding) = from_prop {
                if let Ok(from_expr) = from_binding.try_borrow() {
                    let from_type = from_expr.ty();
                    // TODO double check
                    if from_type != target_type && !from_type.can_convert(&target_type) {
                        let msg = format!(
                            "'from' type {:?} doesn't match 'target' type {:?}",
                            from_type, target_type
                        );
                        diag.push_error(msg, &from_expr.span);
                    }
                }
            }

            // Validate to property matches target type
            if let Some(to_binding) = to_prop {
                if let Ok(to_expr) = to_binding.try_borrow() {
                    let to_type = to_expr.ty();
                    if to_type != target_type && !to_type.can_convert(&target_type) {
                        let msg = format!(
                            "'to' type {:?} doesn't match 'target' type {:?}",
                            to_type, target_type
                        );
                        diag.push_error(msg, &to_expr.span);
                    }
                }
            }
        }
    }
}

fn lower_animation(
    animation_element: &ElementRc,
    anim_type: AnimationType,
    parent_element: Option<&ElementRc>,
    type_register: &Rc<RefCell<TypeRegister>>,
    diag: &mut BuildDiagnostics,
) {
    let parent_component = animation_element.borrow().enclosing_component.upgrade().unwrap();
    if let Some(animation) =
        build_animation(animation_element, anim_type, parent_element, type_register, diag)
    {
        parent_component.animations.borrow_mut().push(animation);
    }
}

/// Validate and lower an animation element and its children
fn build_animation(
    animation_element: &ElementRc,
    anim_type: AnimationType,
    parent_element: Option<&ElementRc>,
    type_register: &Rc<RefCell<TypeRegister>>,
    diag: &mut BuildDiagnostics,
) -> Option<Animation> {
    validate_animation_properties(&animation_element.borrow().bindings, type_register, diag);

    let parent_component = animation_element.borrow().enclosing_component.upgrade().unwrap();
    let Some(parent_element) = parent_element else {
        diag.push_error(
            "A component cannot inherit from Animation".into(),
            &*animation_element.borrow(),
        );
        return None;
    };

    if Rc::ptr_eq(&parent_component.root_element, animation_element) {
        diag.push_error(
            "Animation cannot be directly repeated or conditional".into(),
            &*animation_element.borrow(),
        );
        return None;
    }

    let target = if let Some(target_binding) = animation_element.borrow().bindings.get("target") {
        if let Ok(target_expr) = target_binding.try_borrow() {
            if let Expression::PropertyReference(target_ref) = &target_expr.expression {
                Some(target_ref.clone())
            } else {
                diag.push_error(
                    "Animation 'target' must be a property reference".into(),
                    &target_expr.span,
                );
                return None;
            }
        } else {
            diag.push_error(
                "Animation 'target' binding could not be read".into(),
                &*animation_element.borrow(),
            );
            return None;
        }
    } else if anim_type == AnimationType::Tween {
        diag.push_error(
            "TweenAnimation must have a binding set for its 'target' property".into(),
            &*animation_element.borrow(),
        );
        return None;
    } else {
        None
    };

    let target_type = target.as_ref().map(|t| t.ty());
    let symbol_counters = SymbolCounters::default();
    let mut convert_binding_to_target_type = |name: &str| {
        if let (Some(target_type), Some(binding)) =
            (&target_type, animation_element.borrow().bindings.get(name))
        {
            let mut b = binding.borrow_mut();
            let expr = core::mem::replace(&mut b.expression, Expression::Invalid);
            b.expression =
                expr.maybe_convert_to(target_type.clone(), &*b, diag, &symbol_counters);
        }
    };

    let from = if animation_element.borrow().is_binding_set("from", true) {
        convert_binding_to_target_type("from");
        Some(NamedReference::new(animation_element, SmolStr::new_static("from")))
    } else {
        None
    };

    let to = if animation_element.borrow().is_binding_set("to", true) {
        convert_binding_to_target_type("to");
        Some(NamedReference::new(animation_element, SmolStr::new_static("to")))
    } else {
        None
    };

    let duration = if animation_element.borrow().is_binding_set("duration", true) {
        Some(NamedReference::new(animation_element, SmolStr::new_static("duration")))
    } else if anim_type == AnimationType::Tween {
        diag.push_error(
            "TweenAnimation must have a binding set for its 'duration' property".into(),
            &*animation_element.borrow(),
        );
        return None;
    } else {
        None
    };

    let easing = if animation_element.borrow().is_binding_set("easing", true) {
        Some(NamedReference::new(animation_element, SmolStr::new_static("easing")))
    } else if anim_type == AnimationType::Tween {
        diag.push_error(
            "TweenAnimation must have a binding set for its 'easing' property".into(),
            &*animation_element.borrow(),
        );
        return None;
    } else {
        None
    };

    let iteration_count = if animation_element.borrow().is_binding_set("iteration-count", true) {
        Some(NamedReference::new(animation_element, SmolStr::new_static("iteration-count")))
    } else {
        None
    };

    let direction = if animation_element.borrow().is_binding_set("direction", true) {
        Some(NamedReference::new(animation_element, SmolStr::new_static("direction")))
    } else {
        None
    };

    // Remove the animation_element from its parent
    let mut parent_element_borrowed = parent_element.borrow_mut();
    let index = parent_element_borrowed
        .children
        .iter()
        .position(|child| Rc::ptr_eq(child, animation_element))
        .expect("Animation must be a child of its parent");
    let removed = parent_element_borrowed.children.remove(index);
    parent_component.optimized_elements.borrow_mut().push(removed);
    drop(parent_element_borrowed);
    if let Some(parent_cip) = &mut *parent_component.child_insertion_point.borrow_mut()
        && Rc::ptr_eq(&parent_cip.parent, parent_element)
        && parent_cip.insertion_index > index
    {
        parent_cip.insertion_index -= 1;
    }

    let running = NamedReference::new(animation_element, SmolStr::new_static("running"));
    running.mark_as_set();
    if let Some(target_ref) = &target {
        target_ref.mark_as_set();
    }

    // Only Parallel/Sequential animations can have animation children
    let children = if matches!(anim_type, AnimationType::Parallel | AnimationType::Sequential) {
        let nested_children = animation_element.borrow().children.clone();
        nested_children
            .iter()
            .filter_map(|child| {
                let child_anim_type = get_anim_type(&child.borrow().base_type)?;
                build_animation(
                    child,
                    child_anim_type,
                    Some(animation_element),
                    type_register,
                    diag,
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let update_animations = Expression::FunctionCall {
        function: BuiltinFunction::UpdateAnimations.into(),
        arguments: Vec::new(),
        source_location: None,
    };
    let change_callbacks = &mut animation_element.borrow_mut().change_callbacks;
    change_callbacks.entry("running".into()).or_default().borrow_mut().push(update_animations);

    Some(Animation {
        animation_type: anim_type,
        target,
        running,
        from,
        to,
        duration,
        easing,
        iteration_count,
        direction,
        children,
        element: Rc::downgrade(animation_element),
    })
}
