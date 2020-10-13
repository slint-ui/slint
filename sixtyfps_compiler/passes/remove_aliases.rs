/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This pass removes the property used in a two ways bindings

use crate::{
    diagnostics::BuildDiagnostics,
    expression_tree::{Expression, NamedReference},
    object_tree::*,
    passes::ExpressionFieldsVisitor,
};
use std::collections::{hash_map::Entry, HashMap};
use std::rc::Rc;

type Mapping = HashMap<NamedReference, NamedReference>;
type PropertyReference<'a> = (&'a ElementRc, &'a str);

pub fn remove_aliases(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    // The key will be removed and replaced by the named reference
    let mut aliases_to_remove = Mapping::new();
    // The key key's binding need to take the binding from the other property
    let mut aliases_to_invert = Mapping::new();

    // Detects all aliases
    recurse_elem_including_sub_components(&component.root_element, &(), &mut |e, _| {
        for (name, expr) in &e.borrow().bindings {
            if let Expression::TwoWayBinding(nr) = &expr.expression {
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
                    &mut aliases_to_invert,
                )
            }
        }
    });

    // Do the inversions
    for (from, to) in aliases_to_invert {
        // Move the binding from the `from` to the `to`
        debug_assert!(aliases_to_remove.contains_key(&from));
        let old = from.element.upgrade().unwrap().borrow_mut().bindings.remove(&from.name);
        let _x = if let Some(old) = old {
            to.element.upgrade().unwrap().borrow_mut().bindings.insert(to.name, old)
        } else {
            to.element.upgrade().unwrap().borrow_mut().bindings.remove(&to.name)
        };
        debug_assert!(matches!(_x.unwrap().expression, Expression::TwoWayBinding(_)));
    }

    // Do the replacements
    let mut replace = |nr: &mut NamedReference| {
        if let Some(new) = aliases_to_remove.get(nr) {
            *nr = new.clone();
        }
    };
    recurse_elem_including_sub_components_no_borrow(
        &component.root_element,
        &(),
        &mut |elem, _| visit_all_named_references(elem, replace),
    );
    component.layout_constraints.borrow_mut().visit_expressions(&mut |e| {
        recurse_expression(e, &mut replace);
    });

    // Remove the properties
    for (remove, to) in aliases_to_remove {
        let elem = remove.element.upgrade().unwrap();
        let mut elem = elem.borrow_mut();
        elem.bindings.remove(&remove.name);
        if elem.property_declarations[&remove.name].expose_in_public_api {
            elem.property_declarations.get_mut(&remove.name).unwrap().is_alias = Some(to);
        } else {
            elem.property_declarations.remove(&remove.name);
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
    aliases_to_invert: &mut Mapping,
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
        if !is_declaration(&from) {
            // Cannot remove if this is not a declaration
            return;
        }
        let k = NamedReference { element: Rc::downgrade(&from.0), name: from.1.to_string() };
        match aliases_to_invert.entry(k) {
            Entry::Occupied(_) => {
                // TODO: maybe there are still way to optimize (three way bindings)
                return;
            }
            Entry::Vacant(e) => {
                e.insert(NamedReference { element: Rc::downgrade(&to.0), name: to.1.to_string() });
            }
        }
    } else if !is_declaration(&from) {
        // Cannot remove if this is not a declaration
        return;
    }

    let k = NamedReference { element: Rc::downgrade(&from.0), name: from.1.to_string() };
    match aliases_to_remove.entry(k) {
        Entry::Occupied(_) => {
            todo!("TODO: what to do in that case? Does that mean it is an impossible relation")
        }
        Entry::Vacant(e) => {
            e.insert(NamedReference { element: Rc::downgrade(&to.0), name: to.1.to_string() });
        }
    }
}

/// Visit the NamedReference recursively in expressions
fn recurse_expression(expr: &mut Expression, vis: &mut impl FnMut(&mut NamedReference)) {
    expr.visit_mut(|sub| recurse_expression(sub, vis));
    match expr {
        Expression::PropertyReference(r) | Expression::SignalReference(r) => vis(r),
        _ => {}
    }
}
