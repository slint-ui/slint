// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passe that transform the Timer element into a timer in the Component

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::ElementType;
use crate::object_tree::*;
use smol_str::SmolStr;
use std::rc::Rc;

pub fn lower_timers(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    recurse_elem_including_sub_components_no_borrow(
        component,
        &None,
        &mut |elem, parent_element: &Option<ElementRc>| {
            let is_timer = matches!(&elem.borrow().base_type, ElementType::Builtin(base_type) if base_type.name == "Timer");
            if is_timer {
                lower_timer(elem, parent_element.as_ref(), diag);
            }
            Some(elem.clone())
        },
    )
}

fn lower_timer(
    timer_element: &ElementRc,
    parent_element: Option<&ElementRc>,
    diag: &mut BuildDiagnostics,
) {
    let parent_component = timer_element.borrow().enclosing_component.upgrade().unwrap();
    let Some(parent_element) = parent_element else {
        diag.push_error("A component cannot inherit from Timer".into(), &*timer_element.borrow());
        return;
    };

    if Rc::ptr_eq(&parent_component.root_element, timer_element) {
        diag.push_error(
            "Timer cannot be directly repeated or conditional".into(),
            &*timer_element.borrow(),
        );
        return;
    }

    if !timer_element.borrow().is_binding_set("interval", true) {
        diag.push_error(
            "Timer must have a binding set for its 'interval' property".into(),
            &*timer_element.borrow(),
        );
        return;
    }

    // Remove the timer_element from its parent
    let mut parent_element_borrowed = parent_element.borrow_mut();
    let index = parent_element_borrowed
        .children
        .iter()
        .position(|child| Rc::ptr_eq(child, timer_element))
        .expect("Timer must be a child of its parent");
    let removed = parent_element_borrowed.children.remove(index);
    parent_component.optimized_elements.borrow_mut().push(removed);
    drop(parent_element_borrowed);
    if let Some((p, idx, _)) = &mut *parent_component.child_insertion_point.borrow_mut() {
        if Rc::ptr_eq(p, parent_element) && *idx > index {
            *idx -= 1;
        }
    }

    parent_component.timers.borrow_mut().push(Timer {
        interval: NamedReference::new(timer_element, SmolStr::new_static("interval")),
        running: NamedReference::new(timer_element, SmolStr::new_static("running")),
        triggered: NamedReference::new(timer_element, SmolStr::new_static("triggered")),
    });
    let update_timers = Expression::FunctionCall {
        function: BuiltinFunction::UpdateTimers.into(),
        arguments: vec![],
        source_location: None,
    };
    let change_callbacks = &mut timer_element.borrow_mut().change_callbacks;
    change_callbacks.entry("running".into()).or_default().borrow_mut().push(update_timers.clone());
    change_callbacks.entry("interval".into()).or_default().borrow_mut().push(update_timers);
}
