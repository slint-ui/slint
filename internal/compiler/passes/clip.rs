// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass that lowers synthetic `clip` properties to Clip element

use smol_str::{SmolStr, format_smolstr};
use std::rc::Rc;
use std::sync::Arc;

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
        type_register.lookup_builtin_element("Clip").unwrap().as_builtin().native_class.clone();

    crate::object_tree::recurse_elem_including_sub_components(
        component,
        &(),
        &mut |elem_rc: &ElementRc, _| {
            let elem = elem_rc.borrow();
            if elem.native_class().is_some_and(|n| Arc::ptr_eq(&n, &native_clip)) {
                return;
            }
            if elem.binding("clip").is_some()
                || elem
                    .property_analysis
                    .borrow()
                    .get("clip")
                    .is_some_and(|a| a.is_set || a.is_linked)
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
                            &elem.binding("clip").and_then(|binding| binding.span.clone()).unwrap_or_else(|| elem.to_source_location()),
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

fn create_clip_element(parent_elem: &ElementRc, native_clip: &Arc<NativeClass>) {
    let mut parent = parent_elem.borrow_mut();
    let clip = Element::make_rc(Element {
        id: format_smolstr!("{}-clip", parent.id),
        base_type: crate::langtype::ElementType::Native(native_clip.clone()),
        children: std::mem::take(&mut parent.children),
        enclosing_component: parent.enclosing_component.clone(),
        ..Element::default()
    });

    parent.children.push(clip.clone());
    drop(parent); // NamedReference::new will borrow() the parent, so we can't hold a mutable ref
    for prop in ["width", "height"] {
        clip.borrow_mut().set_binding(
            SmolStr::new_static(prop),
            Expression::PropertyReference(NamedReference::new(
                parent_elem,
                SmolStr::new_static(prop),
            ))
            .into(),
        );
    }

    copy_optional_binding(parent_elem, "border-width", &clip);
    if super::border_radius::BORDER_RADIUS_PROPERTIES
        .iter()
        .any(|property_name| parent_elem.borrow().is_binding_set(property_name, true))
    {
        for optional_binding in super::border_radius::BORDER_RADIUS_PROPERTIES.iter() {
            copy_optional_binding(parent_elem, optional_binding, &clip);
        }
    } else if parent_elem.borrow().binding("border-radius").is_some() {
        for prop in super::border_radius::BORDER_RADIUS_PROPERTIES.iter() {
            clip.borrow_mut().set_binding(
                SmolStr::new(prop),
                Expression::PropertyReference(NamedReference::new(
                    parent_elem,
                    SmolStr::new_static("border-radius"),
                ))
                .into(),
            );
        }
    }
    clip.borrow_mut().set_binding(
        SmolStr::new_static("clip"),
        BindingExpression::new_two_way(
            NamedReference::new(parent_elem, SmolStr::new_static("clip")).into(),
        ),
    );
}

fn copy_optional_binding(
    parent_elem: &ElementRc,
    optional_binding: &'static str,
    clip: &ElementRc,
) {
    if parent_elem.borrow().binding(optional_binding).is_some() {
        clip.borrow_mut().set_binding(
            optional_binding.into(),
            Expression::PropertyReference(NamedReference::new(
                parent_elem,
                SmolStr::new_static(optional_binding),
            ))
            .into(),
        );
    }
}
