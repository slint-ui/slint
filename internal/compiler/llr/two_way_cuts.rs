// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Analysis of two-way binding classes that span both whole struct
//! properties and struct *fields*.
//!
//! At runtime, a struct property whose fields participate in two-way binding
//! classes wears a `StructMemberBindings` wrapper binding, while a property
//! linked as a whole carries a plain `TwoWayBinding`. The two cannot coexist
//! on one property, so every link that would install a plain two-way binding
//! on a property that also needs the wrapper is decomposed into per-field
//! ("cell") links at compile time. Classes without any member link keep the
//! plain `link_two_way` — zero overhead in the common case.
//!
//! The analysis is a fixpoint over all two-way links of the program: a link
//! `a <=> b.path` makes `a` (as a whole) and the field of `b` at `path` the
//! same value forever, so a cell boundary ("cut") required on one side is
//! required, suitably translated, on the other side as well. This also
//! splits prefix-overlapping member links (`x <=> s.a` and `y <=> s.a.b`)
//! into disjoint cells.
//!
//! Model-row two-way bindings (`TwoWayBinding::ModelData`) are not
//! decomposed: they install a binding directly on the left-hand side
//! property and do not participate in the wrapper machinery. Mixing a
//! model-row link and a struct member link on one property is not supported
//! (it was not supported by the previous wide-common machinery either).

use crate::langtype::{ElementType, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::Document;
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};

/// Resolve a property reference on a component-instance element to the
/// declaring component's root element, so that references to the same
/// runtime property from inside and outside a component definition compare
/// equal.
pub fn canonical_property(reference: &NamedReference) -> NamedReference {
    let mut element = reference.element();
    loop {
        let base = {
            let borrowed = element.borrow();
            if borrowed.property_declarations.contains_key(reference.name()) {
                break;
            }
            match &borrowed.base_type {
                ElementType::Component(component) => component.root_element.clone(),
                _ => break,
            }
        };
        element = base;
    }
    NamedReference::new(&element, reference.name().clone())
}

/// The set of field paths ("cuts") at which struct properties participate
/// in two-way binding classes. See the module documentation.
pub struct MemberCuts {
    cuts: HashMap<NamedReference, HashSet<Vec<SmolStr>>>,
}

impl MemberCuts {
    pub fn analyze(document: &Document) -> Self {
        // (prop1, prop2, path2): `prop1 <=> prop2.path2` — the left-hand
        // side of a two-way binding is always a whole property.
        let mut links: Vec<(NamedReference, NamedReference, Vec<SmolStr>)> = Vec::new();
        document.visit_all_used_components(|component| {
            crate::object_tree::recurse_elem_including_sub_components(
                component,
                &(),
                &mut |element, _| {
                    for (name, binding) in &element.borrow().bindings {
                        for twb in &binding.borrow().two_way_bindings {
                            if let crate::expression_tree::TwoWayBinding::Property {
                                property,
                                field_access,
                            } = twb
                            {
                                let prop1 = canonical_property(&NamedReference::new(
                                    element,
                                    name.clone(),
                                ));
                                let prop2 = canonical_property(property);
                                links.push((prop1, prop2, field_access.clone()));
                            }
                        }
                    }
                },
            )
        });

        let mut cuts: HashMap<NamedReference, HashSet<Vec<SmolStr>>> = HashMap::new();
        for (_, prop2, path2) in &links {
            if !path2.is_empty() {
                cuts.entry(prop2.clone()).or_default().insert(path2.clone());
            }
        }

        // Propagate the cuts through the links until a fixpoint is reached.
        // This terminates: only valid (finitely many) field paths of each
        // property's type are ever inserted, and the sets grow monotonically.
        let mut changed = true;
        while changed {
            changed = false;
            for (prop1, prop2, path2) in &links {
                // A cut anywhere in prop1 lies at path2 + cut in prop2.
                let prop1_cuts: Vec<Vec<SmolStr>> =
                    cuts.get(prop1).map(|c| c.iter().cloned().collect()).unwrap_or_default();
                for cut in prop1_cuts {
                    let mut translated = path2.clone();
                    translated.extend(cut);
                    changed |= cuts.entry(prop2.clone()).or_default().insert(translated);
                }
                // A cut strictly below path2 in prop2 lies at the
                // corresponding sub-path in prop1. (Cuts at or above path2
                // do not constrain prop1.)
                let deeper: Vec<Vec<SmolStr>> = cuts
                    .get(prop2)
                    .map(|c| {
                        c.iter()
                            .filter(|cut| cut.len() > path2.len() && cut.starts_with(path2))
                            .map(|cut| cut[path2.len()..].to_vec())
                            .collect()
                    })
                    .unwrap_or_default();
                for sub_path in deeper {
                    changed |= cuts.entry(prop1.clone()).or_default().insert(sub_path);
                }
            }
        }
        Self { cuts }
    }

    /// If the two-way link held by `prop1` must be decomposed, return the
    /// cell partition of `prop1`'s type: the (non-empty, disjoint, covering)
    /// field paths whose links replace the original one. `None` means the
    /// link is kept as-is.
    pub fn decomposed_cells(&self, prop1: &NamedReference) -> Option<Vec<Vec<SmolStr>>> {
        let canonical = canonical_property(prop1);
        let cuts = self.cuts.get(&canonical).filter(|c| !c.is_empty())?;
        let mut cells = Vec::new();
        cover(&canonical.ty(), cuts, &mut Vec::new(), &mut cells);
        if cells.len() == 1 && cells[0].is_empty() { None } else { Some(cells) }
    }
}

/// Compute the partition of `ty`'s field tree induced by `cuts`: recurse
/// into every field as long as a cut lies strictly below the current path.
fn cover(
    ty: &Type,
    cuts: &HashSet<Vec<SmolStr>>,
    prefix: &mut Vec<SmolStr>,
    out: &mut Vec<Vec<SmolStr>>,
) {
    let has_deeper_cut = cuts.iter().any(|cut| cut.len() > prefix.len() && cut.starts_with(prefix));
    if !has_deeper_cut {
        out.push(prefix.clone());
        return;
    }
    let Type::Struct(s) = ty else {
        // cuts always originate from field accesses on struct types
        debug_assert!(false, "two-way binding cut below a non-struct type");
        out.push(prefix.clone());
        return;
    };
    for (field_name, field_type) in &s.fields {
        prefix.push(field_name.clone());
        cover(field_type, cuts, prefix, out);
        prefix.pop();
    }
}
