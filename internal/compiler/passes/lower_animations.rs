// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass validates and lowers Animation elements (TweenAnimation, DelayAnimation, etc.)

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::BindingExpression;
use crate::langtype::ElementType;
use crate::namedreference::NamedReference;
use crate::object_tree::*;
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
    // TODO replace <animation>.start/end with <animation>.running = true/false

    // Walk all elements and validate animation components
    recurse_elem_including_sub_components_no_borrow(
        component,
        &None,
        &mut |elem, parent_element: &Option<ElementRc>| {
            let elem_borrowed = elem.borrow();
            let anim_type = get_anim_type(&elem.borrow().base_type);

            if anim_type.is_some() {
                validate_animation_properties(&elem_borrowed.bindings.clone(), type_register, diag);
                lower_animation(elem, anim_type.unwrap(), parent_element.as_ref(), diag);
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
            } else {
                None
            }
        },
        _ => None
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

    // TODO actually change the type to the actual type
    if let Some(target_binding) = target {
        if let Ok(target_expr) = target_binding.try_borrow() {
            let target_type = target_expr.ty();

            // Validate that target type is animatable
            let type_register_ref = type_register.borrow();
            let elem_type = type_register_ref.property_animation_type_for_property(target_type.clone());
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
                    if from_type != target_type {
                        let msg = format!("'from' type {:?} doesn't match 'target' type {:?}", from_type, target_type);
                        diag.push_error(msg, &from_expr.span);
                    }
                }
            }

            // Validate to property matches target type
            if let Some(to_binding) = to_prop {
                if let Ok(to_expr) = to_binding.try_borrow() {
                    let to_type = to_expr.ty();
                    if to_type != target_type {
                        let msg = format!("'to' type {:?} doesn't match 'target' type {:?}", to_type, target_type);
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
    diag: &mut BuildDiagnostics
) {
    let parent_component = animation_element.borrow().enclosing_component.upgrade().unwrap();
    let Some(parent_element) = parent_element else {
        diag.push_error("A component cannot inherit from Animation".into(), &*animation_element.borrow());
        return;
    };

    if Rc::ptr_eq(&parent_component.root_element, animation_element) {
        diag.push_error(
            "Animation cannot be directly repeated or conditional".into(),
            &*animation_element.borrow(),
        );
        return;
    }

    let target = if animation_element.borrow().is_binding_set("target", true) {
        animation_element.borrow().bindings.get("target").map(|_| NamedReference::new(animation_element, SmolStr::new_static("target")))
    } else if anim_type == AnimationType::Tween {
        diag.push_error(
            "TweenAnimation must have a binding set for its 'target' property".into(),
            &*animation_element.borrow(),
        );
        return;
    } else {
        None
    };
    let from = if animation_element.borrow().is_binding_set("from", true) {
        animation_element.borrow().bindings.get("from").map(|_| NamedReference::new(animation_element, SmolStr::new_static("from")))
    } else {
        None
    };
    let to = if animation_element.borrow().is_binding_set("to", true) {
        animation_element.borrow().bindings.get("to").map(|_| NamedReference::new(animation_element, SmolStr::new_static("to")))
    } else {
        None
    };
    let duration = if animation_element.borrow().is_binding_set("duration", true) {
        animation_element.borrow().bindings.get("duration").map(|_| NamedReference::new(animation_element, SmolStr::new_static("duration")))
    } else if anim_type == AnimationType::Tween {
        diag.push_error(
            "TweenAnimation must have a binding set for its 'duration' property".into(),
            &*animation_element.borrow(),
        );
        return;
    } else {
        None
    };

    let easing = if animation_element.borrow().is_binding_set("easing", true) {
        animation_element.borrow().bindings.get("easing").map(|_| NamedReference::new(animation_element, SmolStr::new_static("easing")))
    } else if anim_type == AnimationType::Tween {
        diag.push_error(
            "TweenAnimation must have a binding set for its 'easing' property".into(),
            &*animation_element.borrow(),
        );
        return;
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

    parent_component.animations.borrow_mut().push(Animation {
        animation_type: anim_type,
        target,
        running,
        from,
        to,
        duration,
        easing,
        // TODO implement children (only necessary for future animation types)
        children: Vec::new(),
        element: Rc::downgrade(animation_element),
    });
}
