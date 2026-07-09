// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass validates and lowers Animation elements (TweenAnimation, DelayAnimation, etc.)

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::BindingExpression;
use crate::langtype::ElementType;
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
    eprintln!("lowering animations");

    // TODO replace <animation>.start/end with <animation>.running = true/false

    // Walk all elements and validate animation components
    recurse_elem_including_sub_components_no_borrow(
        component,
        &None,
        &mut |elem, parent_element: &Option<ElementRc>| {
            let elem_borrowed = elem.borrow();
            let is_animation = matches!(&elem.borrow().base_type, ElementType::Builtin(base_type) if base_type.name == "TweenAnimation");

            if is_animation {
                validate_animation_properties(&elem_borrowed.bindings.clone(), type_register, diag);
                lower_animation(elem, parent_element.as_ref(), diag);
            }
            Some(elem.clone())
        },
    )
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
            let property_animation = match elem_type {
                ElementType::Builtin(p) => Some(p), // returns the PropertyAnimation object
                _ => {
                    let msg = format!("target type {:?} isn't animatable", target_type);
                    diag.push_error(msg, &target_expr.span);
                    None
                }
            };

            // now the property animation type can be passed forwards when this is lowered


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
    parent_element: Option<&ElementRc>,
    diag: &mut BuildDiagnostics
) {

}
