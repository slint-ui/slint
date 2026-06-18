// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass moves all declaration of properties or callback to the root

#![allow(clippy::mutable_key_type)] // ByAddress<ElementRc> keys rely on Rc identity semantics

use crate::expression_tree::{Expression, NamedReference};
use crate::langtype::ElementType;
use crate::object_tree::*;
use by_address::ByAddress;
use core::cell::RefCell;
use smol_str::{SmolStr, format_smolstr};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

/// The root-level name of each declaration that is going to be moved, keyed by the
/// declaring element and the declared name.
/// Concatenating the element id and the declared name alone can give the same name to two
/// different declarations: the ids `a-2` and `a-2-b-4` with the properties `b-4-c` and `c`
/// both concatenate to `a-2-b-4-c`.
type RenameMap = HashMap<(ByAddress<ElementRc>, SmolStr), SmolStr>;

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
    let mut renames = RenameMap::new();
    collect_renames(component, &mut renames);
    do_move_declarations(component, &renames);
}

/// Pick a unique root-level name for every declaration that `do_move_declarations` moves
fn collect_renames(component: &Rc<Component>, renames: &mut RenameMap) {
    let mut elements = Vec::new();
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        if elem.borrow().repeated.is_some() {
            if let ElementType::Component(base) = &elem.borrow().base_type {
                collect_renames(base, renames);
            }
        } else if !Rc::ptr_eq(elem, &component.root_element) {
            elements.push(elem.clone());
        }
    });
    elements.extend(component.optimized_elements.borrow().iter().cloned());

    let mut used: HashSet<SmolStr> =
        component.root_element.borrow().property_declarations.keys().cloned().collect();
    for elem in elements {
        for prop in elem.borrow().property_declarations.keys() {
            let base = map_name(&elem, prop);
            let mut name = base.clone();
            let mut suffix = 1;
            while !used.insert(name.clone()) {
                name = format_smolstr!("{base}-{suffix}");
                suffix += 1;
            }
            renames.insert((ByAddress(elem.clone()), prop.clone()), name);
        }
    }

    component.popup_windows.borrow().iter().for_each(|p| collect_renames(&p.component, renames));
    component.menu_item_tree.borrow().iter().for_each(|c| collect_renames(c, renames));
}

fn do_move_declarations(component: &Rc<Component>, renames: &RenameMap) {
    let mut decl = Declarations::take_from_element(&mut component.root_element.borrow_mut());
    component
        .popup_windows
        .borrow()
        .iter()
        .for_each(|f| do_move_declarations(&f.component, renames));
    component.menu_item_tree.borrow().iter().for_each(|c| do_move_declarations(c, renames));

    let mut new_root_bindings = HashMap::new();
    let mut new_root_change_callbacks = HashMap::new();
    let mut new_root_property_analysis = BTreeMap::new();

    let move_bindings_and_animations = &mut |elem: &ElementRc| {
        visit_all_named_references_in_element(elem, |nr| fixup_reference(nr, renames));

        if elem.borrow().repeated.is_some() {
            if let ElementType::Component(base) = &elem.borrow().base_type {
                do_move_declarations(base, renames);
            } else {
                panic!(
                    "Repeated element should have a component as base because of the repeater_component.rs pass"
                )
            }
            debug_assert!(
                elem.borrow().property_declarations.is_empty() && elem.borrow().children.is_empty(),
                "Repeated element should be empty because of the repeater_component.rs pass"
            );
            return;
        }

        // take the bindings so we do not keep the borrow_mut of the element
        let bindings = core::mem::take(&mut elem.borrow_mut().bindings);
        let mut new_bindings = BindingsMap::default();
        for (k, e) in bindings {
            let will_be_moved = elem.borrow().property_declarations.contains_key(&k);
            if will_be_moved {
                new_root_bindings.insert(moved_name(renames, elem, &k), e);
            } else {
                new_bindings.insert(k, e);
            }
        }
        elem.borrow_mut().bindings = new_bindings;

        let property_analysis = elem.borrow().property_analysis.take();
        let mut new_property_analysis = BTreeMap::new();
        for (prop, a) in property_analysis {
            let will_be_moved = elem.borrow().property_declarations.contains_key(&prop);
            if will_be_moved {
                new_root_property_analysis.insert(moved_name(renames, elem, &prop), a);
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
                new_root_change_callbacks.insert(moved_name(renames, elem, &k), e);
            } else {
                new_change_callbacks.insert(k, e);
            }
        }
        elem.borrow_mut().change_callbacks = new_change_callbacks;
    };

    component.optimized_elements.borrow().iter().for_each(&mut *move_bindings_and_animations);
    recurse_elem(&component.root_element, &(), &mut |e, _| move_bindings_and_animations(e));

    component
        .root_constraints
        .borrow_mut()
        .visit_named_references(&mut |nr| fixup_reference(nr, renames));
    component.popup_windows.borrow_mut().iter_mut().for_each(|p| {
        fixup_reference(&mut p.x, renames);
        fixup_reference(&mut p.y, renames);
        // `is_open` references the synthesized property on this (the parent) component; it must be
        // remapped to the moved-to-root property just like `x`/`y`, otherwise the runtime setter
        // (see the interpreter's `show_popup`) cannot find it once the declaration is hoisted.
        if let Some(is_open) = &mut p.is_open {
            fixup_reference(is_open, renames);
        }
        visit_all_named_references(&p.component, &mut |nr| fixup_reference(nr, renames))
    });
    component.timers.borrow_mut().iter_mut().for_each(|t| {
        fixup_reference(&mut t.interval, renames);
        fixup_reference(&mut t.running, renames);
        fixup_reference(&mut t.triggered, renames);
    });
    component.menu_item_tree.borrow_mut().iter_mut().for_each(|c| {
        visit_all_named_references(c, &mut |nr| fixup_reference(nr, renames));
    });
    component.init_code.borrow_mut().iter_mut().for_each(|expr| {
        visit_named_references_in_expression(expr, &mut |nr| fixup_reference(nr, renames));
    });
    for pd in decl.property_declarations.values_mut() {
        if let Some(nr) = pd.is_alias.as_mut() {
            fixup_reference(nr, renames)
        }
    }

    let move_properties = &mut |elem: &ElementRc| {
        let elem_decl = Declarations::take_from_element(&mut elem.borrow_mut());
        decl.property_declarations.extend(
            elem_decl
                .property_declarations
                .into_iter()
                .map(|(p, d)| (moved_name(renames, elem, &p), d)),
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

/// Map the reference to the previous properties to the new moved property at the root
fn fixup_reference(nr: &mut NamedReference, renames: &RenameMap) {
    let e = nr.element();
    let parent_component = e.borrow().enclosing_component.upgrade().unwrap();
    if !Rc::ptr_eq(&e, &parent_component.root_element)
        && e.borrow().property_declarations.contains_key(nr.name())
    {
        *nr =
            NamedReference::new(&parent_component.root_element, moved_name(renames, &e, nr.name()));
    }
}

fn map_name(e: &ElementRc, s: &SmolStr) -> SmolStr {
    format_smolstr!("{}-{}", e.borrow().id, s)
}

fn moved_name(renames: &RenameMap, e: &ElementRc, s: &SmolStr) -> SmolStr {
    renames.get(&(ByAddress(e.clone()), s.clone())).cloned().unwrap_or_else(|| map_name(e, s))
}

fn simplify_optimized_items_recursive(component: &Rc<Component>) {
    simplify_optimized_items(component.optimized_elements.borrow().as_slice());
    component
        .popup_windows
        .borrow()
        .iter()
        .for_each(|f| simplify_optimized_items_recursive(&f.component));
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        if elem.borrow().repeated.is_some()
            && let ElementType::Component(base) = &elem.borrow().base_type
        {
            simplify_optimized_items_recursive(base);
        }
    });
}

/// Optimized items are not used for the fact that they are items, but their properties
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
