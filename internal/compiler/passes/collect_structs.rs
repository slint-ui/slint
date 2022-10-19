// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Passes that fills the root component used_types.structs

use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::object_tree::*;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Fill the root_component's used_types.structs
pub fn collect_structs(doc: &Document) {
    let mut hash = BTreeMap::new();

    for component in (doc.root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(&doc.root_component))
    {
        collect_structs_in_component(component, &mut hash)
    }

    let mut used_types = doc.root_component.used_types.borrow_mut();
    let used_struct = &mut used_types.structs;
    *used_struct = Vec::with_capacity(hash.len());
    while let Some(next) = hash.iter().next() {
        // Here, using BTreeMap::pop_first would be great when it is stable
        let key = next.0.clone();
        sort_struct(&mut hash, used_struct, &key);
    }
}

fn collect_structs_in_component(root_component: &Rc<Component>, hash: &mut BTreeMap<String, Type>) {
    let mut maybe_collect_object = |ty: &Type| {
        visit_named_object(ty, &mut |name, sub_ty| {
            hash.entry(name.clone()).or_insert_with(|| sub_ty.clone());
        });
    };

    recurse_elem_including_sub_components_no_borrow(root_component, &(), &mut |elem, _| {
        for x in elem.borrow().property_declarations.values() {
            maybe_collect_object(&x.property_type);
        }
    });

    visit_all_expressions(root_component, |expr, _| {
        expr.visit_recursive(&mut |expr| match expr {
            Expression::Struct { ty, .. } => maybe_collect_object(ty),
            Expression::Array { element_ty, .. } => maybe_collect_object(element_ty),
            _ => (),
        })
    });
}

/// Move the object named `key` from hash to vector, making sure that all object used by
/// it are placed before in the vector
fn sort_struct(hash: &mut BTreeMap<String, Type>, vec: &mut Vec<Type>, key: &str) {
    let ty = if let Some(ty) = hash.remove(key) { ty } else { return };
    if let Type::Struct { fields, name: Some(name), .. } = &ty {
        if name.contains("::") {
            // This is a builtin type.
            // FIXME! there should be a better way to handle builtin struct
            return;
        }

        for sub_ty in fields.values() {
            visit_named_object(sub_ty, &mut |name, _| sort_struct(hash, vec, name));
        }
    }
    vec.push(ty)
}

fn visit_named_object(ty: &Type, visitor: &mut impl FnMut(&String, &Type)) {
    match ty {
        Type::Struct { fields, name, .. } => {
            if let Some(struct_name) = name.as_ref() {
                visitor(struct_name, ty);
            }
            for sub_ty in fields.values() {
                visit_named_object(sub_ty, visitor);
            }
        }
        Type::Array(x) => visit_named_object(x, visitor),
        Type::Callback { return_type, args } => {
            if let Some(rt) = return_type {
                visit_named_object(rt, visitor);
            }
            for a in args {
                visit_named_object(a, visitor);
            }
        }
        _ => {}
    }
}
