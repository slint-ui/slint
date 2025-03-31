// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Make sure that the top level element of the component is always a Window

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, Element};
use crate::typeregister::TypeRegister;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub fn ensure_window(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    style_metrics: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) {
    if component.inherits_popup_window.get() {
        diag.push_error(
            "PopupWindow cannot be the top level".into(),
            &*component.root_element.borrow(),
        );
    }

    if inherits_window(component) {
        return; // already a window, nothing to do
    }

    let window_type = type_register.lookup_builtin_element("Window").unwrap();

    let win_elem = component.root_element.clone();

    // the old_root becomes the Window
    let mut win_elem_mut = win_elem.borrow_mut();
    let new_root = Element {
        id: std::mem::replace(&mut win_elem_mut.id, "root_window".into()),
        base_type: std::mem::replace(&mut win_elem_mut.base_type, window_type),
        bindings: Default::default(),
        change_callbacks: Default::default(),
        is_component_placeholder: false,
        property_analysis: Default::default(),
        children: std::mem::take(&mut win_elem_mut.children),
        enclosing_component: win_elem_mut.enclosing_component.clone(),
        property_declarations: Default::default(),
        named_references: Default::default(),
        repeated: Default::default(),
        states: Default::default(),
        transitions: Default::default(),
        child_of_layout: false,
        has_popup_child: false,
        layout_info_prop: Default::default(),
        default_fill_parent: Default::default(),
        accessibility_props: Default::default(),
        geometry_props: Default::default(),
        is_flickable_viewport: false,
        item_index: Default::default(),
        item_index_of_first_children: Default::default(),
        debug: std::mem::take(&mut win_elem_mut.debug),

        inline_depth: 0,
        is_legacy_syntax: false,
    };
    let new_root = new_root.make_rc();
    win_elem_mut.children.push(new_root.clone());
    drop(win_elem_mut);

    let make_two_way = |name: &'static str| {
        new_root.borrow_mut().bindings.insert(
            name.into(),
            RefCell::new(BindingExpression::new_two_way(NamedReference::new(
                &win_elem,
                SmolStr::new_static(name),
            ))),
        );
    };
    make_two_way("width");
    make_two_way("height");

    let mut must_update = HashSet::new();

    let mut base_props: HashSet<SmolStr> =
        new_root.borrow().base_type.property_list().into_iter().map(|x| x.0).collect();
    base_props.extend(win_elem.borrow().bindings.keys().cloned());
    for prop in base_props {
        if prop == "width" || prop == "height" {
            continue;
        }

        if win_elem.borrow().property_declarations.contains_key(&prop) {
            continue;
        }

        must_update.insert(NamedReference::new(&win_elem, prop.clone()));

        if let Some(b) = win_elem.borrow_mut().bindings.remove(&prop) {
            new_root.borrow_mut().bindings.insert(prop.clone(), b);
        }
        if let Some(a) = win_elem.borrow().property_analysis.borrow_mut().remove(&prop) {
            new_root.borrow().property_analysis.borrow_mut().insert(prop.clone(), a);
        }
    }

    crate::object_tree::visit_all_named_references(component, &mut |nr| {
        if must_update.contains(nr) {
            *nr = NamedReference::new(&new_root, nr.name().clone());
        }
    });

    // Fix up any ElementReferences for builtin member function calls, to not refer to the WindowItem,
    // as we swapped out the base_type.
    let fixup_element_reference = |expr: &mut Expression| {
        if let Expression::FunctionCall { arguments, .. } = expr {
            for arg in arguments.iter_mut() {
                if matches!(arg, Expression::ElementReference(elr) if elr.upgrade().is_some_and(|elemrc| Rc::ptr_eq(&elemrc, &win_elem)))
                {
                    *arg = Expression::ElementReference(Rc::downgrade(&new_root))
                }
            }
        }
    };

    crate::object_tree::visit_all_expressions(component, |expr, _| {
        expr.visit_recursive_mut(&mut |expr| fixup_element_reference(expr));
        fixup_element_reference(expr)
    });

    component.root_element.borrow_mut().set_binding_if_not_set("background".into(), || {
        Expression::Cast {
            from: Expression::PropertyReference(NamedReference::new(
                &style_metrics.root_element,
                SmolStr::new_static("window-background"),
            ))
            .into(),
            to: Type::Brush,
        }
    });
}

pub fn inherits_window(component: &Rc<Component>) -> bool {
    component.root_element.borrow().builtin_type().map_or(true, |b| {
        matches!(b.name.as_str(), "Window" | "Dialog" | "WindowItem" | "PopupWindow")
    })
}
