/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This pass removes the property used in a two ways bindings

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, NamedReference};
use crate::object_tree::*;
use std::cell::RefCell;
use std::collections::{btree_map::Entry, HashMap, HashSet};
use std::rc::Rc;

type Mapping = HashMap<NamedReference, NamedReference>;

#[derive(Default, Debug)]
struct PropertySets {
    map: HashMap<NamedReference, Rc<RefCell<HashSet<NamedReference>>>>,
    all_sets: Vec<Rc<RefCell<HashSet<NamedReference>>>>,
}

impl PropertySets {
    fn add_link(&mut self, p1: NamedReference, p2: NamedReference) {
        if !std::rc::Weak::ptr_eq(
            &p1.element().borrow().enclosing_component,
            &p2.element().borrow().enclosing_component,
        ) && (p1.element().borrow().enclosing_component.upgrade().unwrap().is_global()
            == p2.element().borrow().enclosing_component.upgrade().unwrap().is_global())
        {
            // We can  only merge aliases if they are in the same Component. (unless one of them is global)
            // TODO: actually we could still merge two alias in a component pointing to the same
            // property in a parent component
            return;
        }
        if let Some(s1) = self.map.get(&p1).cloned() {
            if let Some(s2) = self.map.get(&p2).cloned() {
                if Rc::ptr_eq(&s1, &s2) {
                    return;
                }
                for x in s1.borrow().iter() {
                    self.map.insert(x.clone(), s2.clone());
                    s2.borrow_mut().insert(x.clone());
                }
                *s1.borrow_mut() = HashSet::new();
            } else {
                s1.borrow_mut().insert(p2.clone());
                self.map.insert(p2, s1);
            }
        } else if let Some(s2) = self.map.get(&p2).cloned() {
            s2.borrow_mut().insert(p1.clone());
            self.map.insert(p1, s2);
        } else {
            let mut set = HashSet::new();
            set.insert(p1.clone());
            set.insert(p2.clone());
            let set = Rc::new(RefCell::new(set));
            self.map.insert(p1, set.clone());
            self.map.insert(p2, set.clone());
            self.all_sets.push(set)
        }
    }
}

pub fn remove_aliases(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    // collect all sets that are linked together
    let mut property_sets = PropertySets::default();
    recurse_elem_including_sub_components(component, &(), &mut |e, _| {
        'bindings: for (name, binding) in &e.borrow().bindings {
            for nr in &binding.two_way_bindings {
                let other_e = nr.element();
                if name == nr.name() && Rc::ptr_eq(e, &other_e) {
                    diag.push_error("Property cannot alias to itself".into(), binding);
                    continue 'bindings;
                }
                property_sets.add_link(NamedReference::new(e, name), nr.clone());
            }
        }
    });

    // The key will be removed and replaced by the named reference
    let mut aliases_to_remove = Mapping::new();

    // For each set, find a "master" property. Only reference to this master property will be kept,
    // and only the master property will keep its binding
    for set in property_sets.all_sets {
        let set = set.borrow();
        let mut set_iter = set.iter();
        if let Some(mut best) = set_iter.next().cloned() {
            for candidate in set_iter {
                best = best_property(component, best.clone(), candidate.clone());
            }
            for x in set.iter() {
                if *x != best {
                    aliases_to_remove.insert(x.clone(), best.clone());
                }
            }
        }
    }

    // Do the replacements
    visit_all_named_references(component, &mut |nr: &mut NamedReference| {
        if let Some(new) = aliases_to_remove.get(nr) {
            *nr = new.clone();
        }
    });

    // Remove the properties
    for (remove, to) in aliases_to_remove {
        let elem = remove.element();

        // adjust the bindings
        let old_binding = elem.borrow_mut().bindings.remove(remove.name());
        let must_simplify = if let Some(mut binding) = old_binding {
            remove_from_binding_expression(&mut binding, &to);
            if binding.has_binding() {
                let to_elem = to.element();
                match to_elem.borrow_mut().bindings.entry(to.name().to_owned()) {
                    Entry::Occupied(mut e) => {
                        remove_from_binding_expression(e.get_mut(), &to);
                        if e.get().priority < binding.priority || !e.get().has_binding() {
                            e.get_mut().merge_with(&binding);
                        } else {
                            binding.merge_with(e.get());
                            *e.get_mut() = binding;
                        }
                    }
                    Entry::Vacant(e) => {
                        e.insert(binding);
                    }
                };
                false
            } else {
                true
            }
        } else {
            true
        };

        if must_simplify {
            let to_elem = to.element();
            let mut to_elem = to_elem.borrow_mut();
            if let Some(b) = to_elem.bindings.get_mut(to.name()) {
                remove_from_binding_expression(b, &to);
                if !b.has_binding() {
                    to_elem.bindings.remove(to.name());
                }
            }
        }

        // Remove the declaration
        {
            let mut elem = elem.borrow_mut();
            let used_externally = elem
                .property_analysis
                .borrow()
                .get(remove.name())
                .map_or(false, |v| v.is_read_externally || v.is_set_externally);
            if let Some(d) = elem.property_declarations.get_mut(remove.name()) {
                if d.expose_in_public_api || used_externally {
                    d.is_alias = Some(to.clone());
                    drop(elem);
                    // one must mark the aliased property as settable from outside
                    to.mark_as_set();
                } else {
                    elem.property_declarations.remove(remove.name());
                    let analysis = elem.property_analysis.borrow().get(remove.name()).cloned();
                    if let Some(analysis) = analysis {
                        drop(elem);
                        to.element()
                            .borrow()
                            .property_analysis
                            .borrow_mut()
                            .entry(to.name().to_owned())
                            .or_default()
                            .merge(&analysis);
                    };
                }
            } else {
                // This is not a declaration, we must re-create the binding
                elem.bindings.insert(remove.name().to_owned(), BindingExpression::new_two_way(to));
            }
        }
    }
}

fn is_declaration(x: &NamedReference) -> bool {
    x.element().borrow().property_declarations.contains_key(x.name())
}

/// Out of two named reference, return the one which is the best to keep.
fn best_property(
    component: &Rc<Component>,
    p1: NamedReference,
    p2: NamedReference,
) -> NamedReference {
    // Try to find which is the more canonical property
    macro_rules! canonical_order {
        ($x: expr) => {{
            (
                !$x.element().borrow().enclosing_component.upgrade().unwrap().is_global(),
                is_declaration(&$x),
                !Rc::ptr_eq(&component.root_element, &$x.element()),
                $x.element().borrow().id.clone(),
                $x.name(),
            )
        }};
    }

    if canonical_order!(p1) < canonical_order!(p2) {
        p1
    } else {
        p2
    }
}

/// Remove the `to` from the two_way_bindings
fn remove_from_binding_expression(expression: &mut BindingExpression, to: &NamedReference) {
    expression.two_way_bindings.retain(|x| x != to);
}
