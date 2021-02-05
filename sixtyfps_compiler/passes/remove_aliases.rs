/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This pass removes the property used in a two ways bindings

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{Expression, NamedReference};
use crate::object_tree::*;
use std::collections::{hash_map::Entry, HashMap};
use std::rc::Rc;

type Mapping = HashMap<NamedReference, NamedReference>;
type PropertyReference<'a> = (&'a ElementRc, &'a str);

pub fn remove_aliases(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    // The key will be removed and replaced by the named reference
    let mut aliases_to_remove = Mapping::new();

    // Detects all aliases
    recurse_elem_including_sub_components(component, &(), &mut |e, _| {
        for (name, expr) in &e.borrow().bindings {
            if let Expression::TwoWayBinding(nr, _) = &expr.expression {
                let other_e = nr.element.upgrade().unwrap();
                if name == &nr.name && Rc::ptr_eq(e, &other_e) {
                    diag.push_error("Property cannot alias to itself".into(), expr);
                    continue;
                }
                process_alias(
                    component,
                    (e, name.as_str()),
                    (&other_e, nr.name.as_str()),
                    &mut aliases_to_remove,
                )
            }
        }
    });

    // Make sure that the aliases_to_remove don't map to alias that themselves need to be removed
    let copy = aliases_to_remove.clone();
    for (_, v) in aliases_to_remove.iter_mut() {
        while let Some(other) = copy.get(v) {
            *v = other.clone();
        }
    }

    // Do the replacements
    visit_all_named_references(&component, &mut |nr: &mut NamedReference| {
        if let Some(new) = aliases_to_remove.get(nr) {
            *nr = new.clone();
        }
    });

    // Remove the properties
    for (remove, to) in aliases_to_remove {
        let elem = remove.element.upgrade().unwrap();

        // Remove the declaration
        {
            let mut elem = elem.borrow_mut();
            if elem.property_declarations[&remove.name].expose_in_public_api {
                elem.property_declarations.get_mut(&remove.name).unwrap().is_alias =
                    Some(to.clone());
            } else {
                elem.property_declarations.remove(&remove.name);
            }
        }

        // adjust the bindings
        let old_binding = elem.borrow_mut().bindings.remove(&remove.name);
        if let Some(mut binding) = old_binding {
            simplify_expression(&mut binding.expression, &to);
            if !matches!(binding.expression, Expression::Invalid) {
                let to_elem = to.element.upgrade().unwrap();
                match to_elem.borrow_mut().bindings.entry(to.name.clone()) {
                    Entry::Occupied(mut e) => {
                        simplify_expression(e.get_mut(), &to);
                        if e.get().priority < binding.priority {
                            crate::passes::inlining::maybe_merge_two_ways(
                                &mut e.get_mut().expression,
                                &mut 0,
                                &binding,
                            );
                        } else {
                            crate::passes::inlining::maybe_merge_two_ways(
                                &mut binding.expression,
                                &mut 0,
                                &e.get(),
                            );
                            *e.get_mut() = binding;
                        }
                    }
                    Entry::Vacant(e) => {
                        e.insert(binding);
                    }
                };
                continue;
            }
        }

        let to_elem = to.element.upgrade().unwrap();
        let mut to_elem = to_elem.borrow_mut();
        if let Some(b) = to_elem.bindings.get_mut(&to.name) {
            simplify_expression(&mut b.expression, &to);
            if matches!(b.expression, Expression::Invalid) {
                to_elem.bindings.remove(&to.name);
            }
        }
    }
}

fn is_declaration(x: &PropertyReference) -> bool {
    x.0.borrow().property_declarations.contains_key(x.1)
}

/// `from` is an alias to `to` which may contains further binding.
/// This funciton will fill the aliases_to_remove and aliases_to_invert map
fn process_alias<'a>(
    component: &Rc<Component>,
    mut from: PropertyReference<'a>,
    mut to: PropertyReference<'a>,
    aliases_to_remove: &mut Mapping,
) {
    // Try to find which is the more canical property
    macro_rules! canonical_order {
        ($x: expr) => {
            (
                is_declaration(&$x),
                !Rc::ptr_eq(&component.root_element, $x.0),
                &$x.0.borrow().id,
                $x.1,
            )
        };
    }

    if canonical_order!(from) < canonical_order!(to) {
        std::mem::swap(&mut from, &mut to);
    }

    if !is_declaration(&from) {
        // Cannot remove if this is not a declaration
        return;
    }

    let k = NamedReference { element: Rc::downgrade(&from.0), name: from.1.to_string() };
    let mut to = NamedReference { element: Rc::downgrade(&to.0), name: to.1.to_string() };
    if let Some(other) = aliases_to_remove.get(&to) {
        to = other.clone();
    }

    aliases_to_remove.entry(k).or_insert(to);
}

/// Remove the `TwoWayBinding(to, _)` from the chain of TwoWayBinding
fn simplify_expression(expression: &mut Expression, to: &NamedReference) {
    if let Expression::TwoWayBinding(nr, rest) = expression {
        if let Some(ref mut r) = rest {
            simplify_expression(&mut *r, to);
        }
        if nr == to {
            if let Some(r) = std::mem::take(rest) {
                *expression = *r;
            } else {
                *expression = Expression::Invalid;
            }
        }
    }
}
