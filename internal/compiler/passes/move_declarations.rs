// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass moves all declaration of properties or callback to the root

use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::ElementType;
use crate::object_tree::*;
use core::cell::RefCell;
use smol_str::{format_smolstr, SmolStr};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

struct Declarations {
    property_declarations: BTreeMap<SmolStr, PropertyDeclaration>,
}
impl Declarations {
    fn take_from_element(e: &mut Element) -> Self {
        Declarations { property_declarations: core::mem::take(&mut e.property_declarations) }
    }
}

pub fn move_declarations(component: &Rc<Component>) {
    simplify_optimized_items_recursive(component);
    do_move_declarations(component);
}

fn do_move_declarations(component: &Rc<Component>) {
    let mut decl = Declarations::take_from_element(&mut component.root_element.borrow_mut());
    component.popup_windows.borrow().iter().for_each(|f| do_move_declarations(&f.component));

    let mut new_root_bindings = HashMap::new();
    let mut new_root_change_callbacks = HashMap::new();
    let mut new_root_property_analysis = HashMap::new();

    let move_bindings_and_animations = &mut |elem: &ElementRc| {
        visit_all_named_references_in_element(elem, fixup_reference);

        if elem.borrow().repeated.is_some() {
            if let ElementType::Component(base) = &elem.borrow().base_type {
                do_move_declarations(base);
            } else {
                panic!("Repeated element should have a component as base because of the repeater_component.rs pass")
            }
            debug_assert!(
                elem.borrow().property_declarations.is_empty() && elem.borrow().children.is_empty(),
                "Repeated element should be empty because of the repeater_component.rs pass"
            );
            return;
        }

        // take the bindings so we do nt keep the borrow_mut of the element
        let bindings = core::mem::take(&mut elem.borrow_mut().bindings);
        let mut new_bindings = BindingsMap::default();
        for (k, e) in bindings {
            let will_be_moved = elem.borrow().property_declarations.contains_key(&k);
            if will_be_moved {
                new_root_bindings.insert(map_name(elem, &k), e);
            } else {
                new_bindings.insert(k, e);
            }
        }
        elem.borrow_mut().bindings = new_bindings;

        let property_analysis = elem.borrow().property_analysis.take();
        let mut new_property_analysis = HashMap::with_capacity(property_analysis.len());
        for (prop, a) in property_analysis {
            let will_be_moved = elem.borrow().property_declarations.contains_key(&prop);
            if will_be_moved {
                new_root_property_analysis.insert(map_name(elem, &prop), a);
            } else {
                new_property_analysis.insert(prop, a);
            }
        }
        *elem.borrow().property_analysis.borrow_mut() = new_property_analysis;

        // Also move the changed callback
        let change_callbacks = core::mem::take(&mut elem.borrow_mut().change_callbacks);
        let mut new_change_callbacks = BTreeMap::<SmolStr, RefCell<Vec<Expression>>>::default();
        for (k, e) in change_callbacks {
            let will_be_moved = elem.borrow().property_declarations.contains_key(&k);
            if will_be_moved {
                new_root_change_callbacks.insert(map_name(elem, &k), e);
            } else {
                new_change_callbacks.insert(k, e);
            }
        }
        elem.borrow_mut().change_callbacks = new_change_callbacks;
    };

    component.optimized_elements.borrow().iter().for_each(|e| move_bindings_and_animations(e));
    recurse_elem(&component.root_element, &(), &mut |e, _| move_bindings_and_animations(e));

    component.root_constraints.borrow_mut().visit_named_references(&mut fixup_reference);
    component.popup_windows.borrow_mut().iter_mut().for_each(|p| {
        fixup_reference(&mut p.x);
        fixup_reference(&mut p.y);
        visit_all_named_references(&p.component, &mut fixup_reference)
    });
    component.timers.borrow_mut().iter_mut().for_each(|t| {
        fixup_reference(&mut t.interval);
        fixup_reference(&mut t.running);
        fixup_reference(&mut t.triggered);
    });
    component.init_code.borrow_mut().iter_mut().for_each(|expr| {
        visit_named_references_in_expression(expr, &mut fixup_reference);
    });
    for pd in decl.property_declarations.values_mut() {
        pd.is_alias.as_mut().map(fixup_reference);
    }

    let move_properties = &mut |elem: &ElementRc| {
        let elem_decl = Declarations::take_from_element(&mut elem.borrow_mut());
        decl.property_declarations.extend(
            elem_decl.property_declarations.into_iter().map(|(p, d)| (map_name(elem, &p), d)),
        );
    };

    recurse_elem(&component.root_element, &(), &mut |elem, _| move_properties(elem));

    component.optimized_elements.borrow().iter().for_each(move_properties);

    {
        let mut r = component.root_element.borrow_mut();
        r.property_declarations = decl.property_declarations;
        r.bindings.extend(new_root_bindings);
        r.property_analysis.borrow_mut().extend(new_root_property_analysis);
        r.change_callbacks.extend(new_root_change_callbacks);
    }
}

fn fixup_reference(nr: &mut NamedReference) {
    let e = nr.element();
    let component = e.borrow().enclosing_component.upgrade().unwrap();
    if !Rc::ptr_eq(&e, &component.root_element)
        && e.borrow().property_declarations.contains_key(nr.name())
    {
        *nr = NamedReference::new(&component.root_element, map_name(&e, nr.name()).into());
    }
}

fn map_name(e: &ElementRc, s: &SmolStr) -> SmolStr {
    format_smolstr!("{}-{}", e.borrow().id, s)
}

fn simplify_optimized_items_recursive(component: &Rc<Component>) {
    simplify_optimized_items(component.optimized_elements.borrow().as_slice());
    component
        .popup_windows
        .borrow()
        .iter()
        .for_each(|f| simplify_optimized_items_recursive(&f.component));
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        if elem.borrow().repeated.is_some() {
            if let ElementType::Component(base) = &elem.borrow().base_type {
                simplify_optimized_items_recursive(base);
            }
        }
    });
}

/// Optimized item are not used for the fact that they are items, but their properties
/// might still be used.  So we must pretend all the properties are declared in the
/// item itself so the move_declaration pass can move the declaration in the component root
fn simplify_optimized_items(items: &[ElementRc]) {
    for elem in items {
        recurse_elem(elem, &(), &mut |elem, _| {
            let base = core::mem::take(&mut elem.borrow_mut().base_type);
            if let ElementType::Builtin(c) = base {
                // This assume that all properties of builtin items are fine with the default value
                elem.borrow_mut().property_declarations.extend(c.properties.iter().map(
                    |(k, v)| {
                        (
                            k.clone(),
                            PropertyDeclaration {
                                property_type: v.ty.clone(),
                                ..Default::default()
                            },
                        )
                    },
                ));
            } else {
                unreachable!("Only builtin items should be optimized")
            }
        })
    }
}
