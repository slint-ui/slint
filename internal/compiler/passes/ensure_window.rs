// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Make sure that the top level element of the component is always a Window

use crate::expression_tree::{BindingExpression, Expression};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, Element};
use crate::typeregister::TypeRegister;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub fn ensure_window(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    style_metrics: &Rc<Component>,
) {
    if component.root_element.borrow().builtin_type().map_or(true, |b| {
        matches!(b.name.as_str(), "Window" | "Dialog" | "WindowItem" | "PopupWindow")
    }) {
        return; // already a window, nothing to do
    }

    let window_type = type_register.lookup_element("Window").unwrap();

    let win_elem = component.root_element.clone();

    // the old_root becomes the Window
    let mut win_elem_mut = win_elem.borrow_mut();
    let new_root = Element {
        id: std::mem::replace(&mut win_elem_mut.id, "root_window".into()),
        base_type: std::mem::replace(&mut win_elem_mut.base_type, window_type),
        bindings: Default::default(),
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
        accessibility_props: Default::default(),
        is_flickable_viewport: false,
        item_index: Default::default(),
        item_index_of_first_children: Default::default(),
        node: win_elem_mut.node.clone(),
        inline_depth: 0,
    };
    let new_root = Rc::new(RefCell::new(new_root));
    win_elem_mut.children.push(new_root.clone());
    drop(win_elem_mut);

    let make_two_way = |name: &str| {
        new_root.borrow_mut().bindings.insert(
            name.into(),
            RefCell::new(BindingExpression::new_two_way(NamedReference::new(&win_elem, name))),
        );
    };
    make_two_way("width");
    make_two_way("height");

    let mut must_update = HashSet::new();

    let mut base_props: HashSet<String> =
        new_root.borrow().base_type.property_list().into_iter().map(|x| x.0).collect();
    base_props.extend(win_elem.borrow().bindings.keys().cloned());
    for prop in base_props {
        if prop == "width" || prop == "height" {
            continue;
        }

        if win_elem.borrow().property_declarations.contains_key(&prop) {
            continue;
        }

        must_update.insert(NamedReference::new(&win_elem, &prop));

        if let Some(b) = win_elem.borrow_mut().bindings.remove(&prop) {
            new_root.borrow_mut().bindings.insert(prop.clone(), b);
        }
        if let Some(a) = win_elem.borrow().property_analysis.borrow_mut().remove(&prop) {
            new_root.borrow().property_analysis.borrow_mut().insert(prop.clone(), a);
        }
    }

    crate::object_tree::visit_all_named_references(component, &mut |nr| {
        if must_update.contains(nr) {
            *nr = NamedReference::new(&new_root, nr.name());
        }
    });

    component.root_element.borrow_mut().set_binding_if_not_set("background".into(), || {
        Expression::Cast {
            from: Expression::PropertyReference(NamedReference::new(
                &style_metrics.root_element,
                "window-background",
            ))
            .into(),
            to: Type::Brush,
        }
    });
}
