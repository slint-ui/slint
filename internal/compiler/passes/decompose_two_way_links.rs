// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Decompose whole-struct two-way links whose class also contains
//! struct-field links into per-field ("cell") links.
//!
//! At runtime, a struct property whose fields participate in two-way
//! binding classes wears a `StructMemberBindings` wrapper binding, while a
//! property linked as a whole carries a plain `TwoWayBinding`; the two
//! cannot coexist on one property. See [`crate::llr::two_way_cuts`] for the
//! analysis. This pass performs the equivalent rewrite on the object tree
//! for backends that do not lower through the LLR (the interpreter); it is
//! enabled by `CompilerConfiguration::decompose_struct_two_way_links`.

use crate::expression_tree::TwoWayBinding;
use crate::llr::two_way_cuts::{MemberCuts, canonical_property};
use crate::namedreference::NamedReference;
use crate::object_tree::{Document, recurse_elem_including_sub_components};

/// Whether the interpreter stores this property as a `Property<Value>`:
/// user-declared properties of struct type are; native item properties and
/// builtin-global properties are natively typed and keep the wide-common
/// machinery.
fn is_declared_property(reference: &NamedReference) -> bool {
    let canonical = canonical_property(reference);
    let element = canonical.element();
    let is_declared = element.borrow().property_declarations.contains_key(canonical.name());
    is_declared
}

pub fn decompose_two_way_links(doc: &Document) {
    let cuts = MemberCuts::analyze(doc);

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
            for (name, binding) in &elem.borrow().bindings {
                if binding.borrow().two_way_bindings.is_empty() {
                    continue;
                }
                let prop1 = NamedReference::new(elem, name.clone());
                let Some(cells) = cuts.decomposed_cells(&prop1) else { continue };
                if !is_declared_property(&prop1) {
                    // natively stored struct property: keep the wide-common
                    // machinery (pre-existing behavior)
                    continue;
                }
                let two_way_bindings = &mut binding.borrow_mut().two_way_bindings;
                *two_way_bindings = two_way_bindings
                    .drain(..)
                    .flat_map(|twb| -> Vec<TwoWayBinding> {
                        match twb {
                            TwoWayBinding::Property { property, field_access, field_access1 }
                                if field_access1.is_empty()
                                    && is_declared_property(&property) =>
                            {
                                cells
                                    .iter()
                                    .map(|cell| {
                                        let mut cell_field_access = field_access.clone();
                                        cell_field_access.extend(cell.iter().cloned());
                                        TwoWayBinding::Property {
                                            property: property.clone(),
                                            field_access: cell_field_access,
                                            field_access1: cell.clone(),
                                        }
                                    })
                                    .collect()
                            }
                            TwoWayBinding::ModelData {
                                repeated_element,
                                field_access,
                                field_access1,
                            } if field_access1.is_empty() => cells
                                .iter()
                                .map(|cell| {
                                    let mut cell_field_access = field_access.clone();
                                    cell_field_access.extend(cell.iter().cloned());
                                    TwoWayBinding::ModelData {
                                        repeated_element: repeated_element.clone(),
                                        field_access: cell_field_access,
                                        field_access1: cell.clone(),
                                    }
                                })
                                .collect(),
                            other => vec![other],
                        }
                    })
                    .collect();
            }
        })
    });
}
