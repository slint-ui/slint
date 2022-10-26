// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Pass that lowers synthetic `clip` properties to Clip element

use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::langtype::NativeClass;
use crate::object_tree::{Component, Element, ElementRc};
use crate::typeregister::TypeRegister;

pub fn handle_clip(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let native_clip =
        type_register.lookup_element("Clip").unwrap().as_builtin().native_class.clone();

    crate::object_tree::recurse_elem_including_sub_components(
        component,
        &(),
        &mut |elem_rc: &ElementRc, _| {
            let elem = elem_rc.borrow();
            if elem.native_class().map_or(false, |n| Rc::ptr_eq(&n, &native_clip)) {
                return;
            }
            if elem.bindings.contains_key("clip")
                || elem.property_analysis.borrow().get("clip").map_or(false, |a| a.is_set)
            {
                match elem.builtin_type().as_ref().map(|ty| ty.name.as_str()) {
                    Some("Rectangle") => {}
                    Some("Path") => {
                        // it's an actual property, so keep it as is
                        return;
                    }
                    _ => {
                        diag.push_error(
                            "The 'clip' property can only be applied to a Rectangle or a Path for now".into(),
                            &elem.bindings.get("clip").and_then(|x| x.borrow().span.clone()).or_else(|| elem.node.as_ref().map(|e| e.to_source_location())),
                        );
                        return;
                    }
                }
                drop(elem);
                create_clip_element(elem_rc, &native_clip);
            }
        },
    );
}

fn create_clip_element(parent_elem: &ElementRc, native_clip: &Rc<NativeClass>) {
    let mut parent = parent_elem.borrow_mut();
    let clip = Rc::new(RefCell::new(Element {
        id: format!("{}-clip", parent.id),
        base_type: crate::langtype::ElementType::Native(native_clip.clone()),
        children: std::mem::take(&mut parent.children),
        enclosing_component: parent.enclosing_component.clone(),
        ..Element::default()
    }));

    parent.children.push(clip.clone());
    drop(parent); // NamedReference::new will borrow() the parent, so we can't hold a mutable ref
    clip.borrow_mut().bindings = ["width", "height"]
        .iter()
        .map(|prop| {
            (
                (*prop).to_owned(),
                RefCell::new(
                    Expression::PropertyReference(NamedReference::new(parent_elem, prop)).into(),
                ),
            )
        })
        .collect();
    for optional_binding in ["border-radius", "border-width"].iter() {
        if parent_elem.borrow().bindings.contains_key(*optional_binding) {
            clip.borrow_mut().bindings.insert(
                optional_binding.to_string(),
                RefCell::new(
                    Expression::PropertyReference(NamedReference::new(
                        parent_elem,
                        optional_binding,
                    ))
                    .into(),
                ),
            );
        }
    }
    clip.borrow_mut().bindings.insert(
        "clip".to_owned(),
        BindingExpression::new_two_way(NamedReference::new(parent_elem, "clip")).into(),
    );
}
