// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Passes that fills the root component used_types.structs

use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::object_tree::*;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Fill the root_component's used_types.structs
pub fn collect_structs_and_enums(doc: &Document) {
    let mut hash = BTreeMap::new();

    for (_, exp) in doc.exports.iter() {
        if let Some(ty) = exp.as_ref().right() {
            maybe_collect_object(ty, &mut hash);
        }
    }

    for component in (doc.root_component.used_types.borrow().sub_components.iter())
        .chain(std::iter::once(&doc.root_component))
    {
        collect_types_in_component(component, &mut hash)
    }

    let mut used_types = doc.root_component.used_types.borrow_mut();
    let used_struct_and_enums = &mut used_types.structs_and_enums;
    *used_struct_and_enums = Vec::with_capacity(hash.len());
    while let Some(next) = hash.iter().next() {
        // Here, using BTreeMap::pop_first would be great when it is stable
        let key = next.0.clone();
        sort_types(&mut hash, used_struct_and_enums, &key);
    }
}

fn maybe_collect_object(ty: &Type, hash: &mut BTreeMap<String, Type>) {
    visit_declared_type(ty, &mut |name, sub_ty| {
        hash.entry(name.clone()).or_insert_with(|| sub_ty.clone());
    });
}

fn collect_types_in_component(root_component: &Rc<Component>, hash: &mut BTreeMap<String, Type>) {
    recurse_elem_including_sub_components_no_borrow(root_component, &(), &mut |elem, _| {
        for x in elem.borrow().property_declarations.values() {
            maybe_collect_object(&x.property_type, hash);
        }
    });

    visit_all_expressions(root_component, |expr, _| {
        expr.visit_recursive(&mut |expr| match expr {
            Expression::Struct { ty, .. } => maybe_collect_object(ty, hash),
            Expression::Array { element_ty, .. } => maybe_collect_object(element_ty, hash),
            Expression::EnumerationValue(ev) => {
                maybe_collect_object(&Type::Enumeration(ev.enumeration.clone()), hash)
            }
            _ => (),
        })
    });
}

/// Move the object named `key` from hash to vector, making sure that all object used by
/// it are placed before in the vector
fn sort_types(hash: &mut BTreeMap<String, Type>, vec: &mut Vec<Type>, key: &str) {
    let ty = if let Some(ty) = hash.remove(key) { ty } else { return };
    if let Type::Struct { fields, name: Some(name), .. } = &ty {
        if name.contains("::") {
            // This is a builtin type.
            // FIXME! there should be a better way to handle builtin struct
            return;
        }

        for sub_ty in fields.values() {
            visit_declared_type(sub_ty, &mut |name, _| sort_types(hash, vec, name));
        }
    }
    vec.push(ty)
}

/// Will call the `visitor` for every named struct or enum that is not builtin
fn visit_declared_type(ty: &Type, visitor: &mut impl FnMut(&String, &Type)) {
    match ty {
        Type::Struct { fields, name, node, .. } => {
            if node.is_some() {
                if let Some(struct_name) = name.as_ref() {
                    visitor(struct_name, ty);
                }
            }
            for sub_ty in fields.values() {
                visit_declared_type(sub_ty, visitor);
            }
        }
        Type::Array(x) => visit_declared_type(x, visitor),
        Type::Callback { return_type, args } => {
            if let Some(rt) = return_type {
                visit_declared_type(rt, visitor);
            }
            for a in args {
                visit_declared_type(a, visitor);
            }
        }
        Type::Function { return_type, args } => {
            visit_declared_type(return_type, visitor);
            for a in args {
                visit_declared_type(a, visitor);
            }
        }
        Type::Enumeration(en) => {
            if en.node.is_some() {
                visitor(&en.name, ty)
            }
        }
        _ => {}
    }
}
