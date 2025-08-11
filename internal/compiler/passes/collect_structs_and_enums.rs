// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass fills the root component's used_types.structs_and_enums

use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Fill the root_component's used_types.structs
pub fn collect_structs_and_enums(doc: &Document) {
    let mut hash = BTreeMap::new();

    for (_, exp) in doc.exports.iter() {
        if let Some(ty) = exp.as_ref().right() {
            maybe_collect_struct(ty, &mut hash);
        }
    }

    doc.visit_all_used_components(|component| collect_types_in_component(component, &mut hash));

    let mut used_types = doc.used_types.borrow_mut();
    let used_struct_and_enums = &mut used_types.structs_and_enums;
    *used_struct_and_enums = Vec::with_capacity(hash.len());
    while let Some(next) = hash.iter().next() {
        // Here, using BTreeMap::pop_first would be great when it is stable
        let key = next.0.clone();
        sort_types(&mut hash, used_struct_and_enums, &key);
    }
}

fn maybe_collect_struct(ty: &Type, hash: &mut BTreeMap<SmolStr, Type>) {
    visit_declared_type(ty, &mut |name, sub_ty| {
        hash.entry(name.clone()).or_insert_with(|| sub_ty.clone());
    });
}

fn collect_types_in_component(root_component: &Rc<Component>, hash: &mut BTreeMap<SmolStr, Type>) {
    recurse_elem_including_sub_components_no_borrow(root_component, &(), &mut |elem, _| {
        for x in elem.borrow().property_declarations.values() {
            maybe_collect_struct(&x.property_type, hash);
        }
    });

    visit_all_expressions(root_component, |expr, _| {
        expr.visit_recursive(&mut |expr| match expr {
            Expression::Struct { ty, .. } => maybe_collect_struct(&Type::Struct(ty.clone()), hash),
            Expression::Array { element_ty, .. } => maybe_collect_struct(element_ty, hash),
            Expression::EnumerationValue(ev) => {
                maybe_collect_struct(&Type::Enumeration(ev.enumeration.clone()), hash)
            }
            _ => (),
        })
    });
}

/// Move the object named `key` from hash to vector, making sure that all object used by
/// it are placed before in the vector
fn sort_types(hash: &mut BTreeMap<SmolStr, Type>, vec: &mut Vec<Type>, key: &str) {
    let ty = if let Some(ty) = hash.remove(key) { ty } else { return };
    if let Type::Struct(s) = &ty {
        if let Some(name) = &s.name {
            if name.contains("::") {
                // This is a builtin type.
                // FIXME! there should be a better way to handle builtin struct
                return;
            }

            for sub_ty in s.fields.values() {
                visit_declared_type(sub_ty, &mut |name, _| sort_types(hash, vec, name));
            }
        }
    }
    vec.push(ty)
}

/// Will call the `visitor` for every named struct or enum that is not builtin
fn visit_declared_type(ty: &Type, visitor: &mut impl FnMut(&SmolStr, &Type)) {
    match ty {
        Type::Struct(s) => {
            if s.node.is_some() {
                if let Some(struct_name) = s.name.as_ref() {
                    visitor(struct_name, ty);
                }
            }
            for sub_ty in s.fields.values() {
                visit_declared_type(sub_ty, visitor);
            }
        }
        Type::Array(x) => visit_declared_type(x, visitor),
        Type::Function(function) | Type::Callback(function) => {
            visit_declared_type(&function.return_type, visitor);
            for a in &function.args {
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
