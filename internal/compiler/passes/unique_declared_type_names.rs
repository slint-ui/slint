// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass gives a unique internal name to declared structs and enums that would otherwise
//! share a name with a different declaration.
//!
//! `collect_structs_and_enums` and the code generators identify a declared type by its name.
//! When two different files declare a `struct` or `enum` with the same name, they would collapse
//! into one type, so a value of one silently becomes a value of the other (see #6880, #9358).
//! Renaming the extra declarations here keeps each one distinct.

use crate::expression_tree::Expression;
use crate::langtype::{DeclNode, Struct, StructName, Type, visit_declared_types};
use crate::object_tree::*;
use smol_str::{SmolStr, format_smolstr};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

/// Identifies a declaration by its source location, so two references to the same declared type
/// share an identity while two same-named declarations in different places do not.
type Identity = (SmolStr, u32, u32);

fn identity(node: &DeclNode) -> Identity {
    let range = node.text_range();
    (node.source_file().path().to_string_lossy().into(), range.start().into(), range.end().into())
}

type Renames = HashMap<Identity, SmolStr>;

pub fn assign_unique_declared_type_names(doc: &mut Document) {
    // 1. Gather every declared struct/enum, grouped by name, and the set of all used names.
    let by_name: RefCell<BTreeMap<SmolStr, BTreeSet<Identity>>> = RefCell::new(BTreeMap::new());
    let all_names: RefCell<BTreeSet<SmolStr>> = RefCell::new(BTreeSet::new());
    let record = |name: &SmolStr, ty: &Type| {
        all_names.borrow_mut().insert(name.clone());
        if let Some((_, id)) = declared_name_and_identity(ty) {
            by_name.borrow_mut().entry(name.clone()).or_default().insert(id);
        }
    };
    let collect = |ty: &Type| visit_declared_types(ty, &mut |name, ty| record(name, ty));
    for (_, export) in doc.exports.iter() {
        if let Some(ty) = export.as_ref().right() {
            collect(ty);
        }
    }
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
            for pd in elem.borrow().property_declarations.values() {
                collect(&pd.property_type);
            }
        });
        visit_all_expressions(component, |expr, _| {
            expr.visit_recursive(&mut |e| collect_expression_types(e, &collect));
        });
    });

    // 2. For each colliding name, keep one declaration with the original name (preferring an
    //    exported one, so the public type keeps its name) and give the others a fresh unique name.
    let exported_ids: BTreeSet<Identity> = doc
        .exports
        .iter()
        .filter_map(|(_, component_or_type)| match component_or_type {
            itertools::Either::Right(ty) => declared_name_and_identity(ty).map(|(_, id)| id),
            _ => None,
        })
        .collect();
    let mut renames = Renames::new();
    let mut all_names = all_names.into_inner();
    let mut collision_renamed_names = BTreeSet::new();
    for (name, ids) in by_name.into_inner() {
        if ids.len() <= 1 {
            continue;
        }
        let ids: Vec<Identity> = ids.into_iter().collect();
        let keep = ids.iter().position(|id| exported_ids.contains(id)).unwrap_or(0);
        for (i, id) in ids.into_iter().enumerate() {
            if i == keep {
                continue;
            }
            let mut n = 2;
            let new_name = loop {
                let candidate = format_smolstr!("{name}{n}");
                if all_names.insert(candidate.clone()) {
                    break candidate;
                }
                n += 1;
            };
            collision_renamed_names.insert(new_name.clone());
            renames.insert(id, new_name);
        }
    }
    doc.used_types.borrow_mut().collision_renamed_names = collision_renamed_names;

    // A type exported only under a different name (`export { X as Y }`, with `X` not exported on
    // its own) is renamed to that export name, so `Y` becomes its real name and `X` is kept only
    // as a deprecated alias.
    let export_names: BTreeSet<SmolStr> =
        doc.exports.iter().map(|(name, _)| name.name.clone()).collect();
    let mut deprecated_type_aliases = Vec::new();
    for (export_name, component_or_type) in doc.exports.iter() {
        if let itertools::Either::Right(ty) = component_or_type
            && let Some((type_name, id)) = declared_name_and_identity(ty)
            && type_name != export_name.name
            && !export_names.contains(&type_name)
        {
            renames.insert(id, export_name.name.clone());
            deprecated_type_aliases.push((type_name, export_name.name.clone()));
        }
    }
    doc.used_types.borrow_mut().deprecated_type_aliases = deprecated_type_aliases;

    if renames.is_empty() {
        return;
    }

    // 3. Rewrite every reference to a renamed declaration.
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
            for pd in elem.borrow_mut().property_declarations.values_mut() {
                rename_type(&mut pd.property_type, &renames);
            }
        });
        visit_all_expressions(component, |expr, _| {
            expr.visit_recursive_mut(&mut |e| rewrite_expression_types(e, &renames));
        });
    });

    // The exported types too, so a re-export points at the renamed type and not the one that
    // happens to keep the original name.
    doc.exports.retain(|(_, component_or_type)| {
        if let itertools::Either::Right(ty) = component_or_type {
            rename_type(ty, &renames);
        }
        true
    });
}

/// The declared name and identity of a user-declared struct or enum, if `ty` is one.
fn declared_name_and_identity(ty: &Type) -> Option<(SmolStr, Identity)> {
    match ty {
        Type::Enumeration(en) => en.node.as_ref().map(|node| (en.name.clone(), identity(node))),
        Type::Struct(s) => match &s.name {
            StructName::User { name, node, .. } => Some((name.clone(), identity(node))),
            _ => None,
        },
        _ => None,
    }
}

fn collect_expression_types(e: &Expression, collect: &impl Fn(&Type)) {
    match e {
        Expression::FunctionParameterReference { ty, .. }
        | Expression::ReadLocalVariable { ty, .. }
        | Expression::Cast { to: ty, .. }
        | Expression::Array { element_ty: ty, .. }
        | Expression::MinMax { ty, .. } => collect(ty),
        Expression::Struct { ty, .. } => collect(&Type::Struct(ty.clone())),
        Expression::EnumerationValue(ev) => collect(&Type::Enumeration(ev.enumeration.clone())),
        _ => {}
    }
}

fn rewrite_expression_types(e: &mut Expression, renames: &Renames) {
    match e {
        Expression::FunctionParameterReference { ty, .. }
        | Expression::ReadLocalVariable { ty, .. }
        | Expression::Cast { to: ty, .. }
        | Expression::Array { element_ty: ty, .. }
        | Expression::MinMax { ty, .. } => rename_type(ty, renames),
        Expression::Struct { ty, .. } => rename_struct(ty, renames),
        Expression::EnumerationValue(ev) => rename_type_enum(&mut ev.enumeration, renames),
        _ => {}
    }
}

fn rename_type(ty: &mut Type, renames: &Renames) {
    match ty {
        Type::Enumeration(en) => rename_type_enum(en, renames),
        Type::Struct(s) => rename_struct(s, renames),
        Type::Array(inner) => {
            let mut new_inner = (**inner).clone();
            rename_type(&mut new_inner, renames);
            if new_inner != **inner {
                *inner = Arc::new(new_inner);
            }
        }
        Type::Callback(function) | Type::Function(function) => {
            let mut f = (**function).clone();
            rename_type(&mut f.return_type, renames);
            for arg in &mut f.args {
                rename_type(arg, renames);
            }
            if f != **function {
                *function = Arc::new(f);
            }
        }
        _ => {}
    }
}

fn rename_type_enum(en: &mut Arc<crate::langtype::Enumeration>, renames: &Renames) {
    let Some(new_name) = en.node.as_ref().and_then(|node| renames.get(&identity(node))) else {
        return;
    };
    if &en.name != new_name {
        let mut new_en = (**en).clone();
        new_en.name = new_name.clone();
        *en = Arc::new(new_en);
    }
}

fn rename_struct(s: &mut Arc<Struct>, renames: &Renames) {
    let mut new_fields: Option<BTreeMap<SmolStr, Type>> = None;
    for (key, field) in &s.fields {
        let mut new_field = field.clone();
        rename_type(&mut new_field, renames);
        if &new_field != field {
            new_fields.get_or_insert_with(|| s.fields.clone()).insert(key.clone(), new_field);
        }
    }

    let new_name = if let StructName::User { name, node, .. } = &s.name {
        renames.get(&identity(node)).filter(|n| *n != name).cloned()
    } else {
        None
    };

    if new_fields.is_none() && new_name.is_none() {
        return;
    }

    let mut new_struct = (**s).clone();
    if let Some(fields) = new_fields {
        new_struct.fields = fields;
    }
    if let Some(name) = new_name
        && let StructName::User { name: struct_name, .. } = &mut new_struct.name
    {
        *struct_name = name;
    }
    *s = Arc::new(new_struct);
}
