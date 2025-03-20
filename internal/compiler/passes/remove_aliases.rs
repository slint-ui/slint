// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass removes the property used in a two ways bindings

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::object_tree::*;
use std::cell::RefCell;
use std::collections::{btree_map::Entry, HashMap, HashSet};
use std::rc::Rc;

// The property in the key is to be removed, and replaced by the property in the value
type Mapping = HashMap<NamedReference, NamedReference>;

#[derive(Default, Debug)]
struct PropertySets {
    map: HashMap<NamedReference, Rc<RefCell<HashSet<NamedReference>>>>,
    all_sets: Vec<Rc<RefCell<HashSet<NamedReference>>>>,
}

impl PropertySets {
    fn add_link(&mut self, p1: NamedReference, p2: NamedReference) {
        let (e1, e2) = (p1.element(), p2.element());
        if !std::rc::Weak::ptr_eq(
            &e1.borrow().enclosing_component,
            &e2.borrow().enclosing_component,
        ) {
            if !(e1.borrow().enclosing_component.upgrade().unwrap().is_global()
                && !e2.borrow().change_callbacks.contains_key(p2.name()))
                && !(e2.borrow().enclosing_component.upgrade().unwrap().is_global()
                    && !e1.borrow().change_callbacks.contains_key(p1.name()))
            {
                // We can only merge aliases if they are in the same Component. (unless one of them is global if the other one don't have change event)
                // TODO: actually we could still merge two alias in a component pointing to the same
                // property in a parent component
                return;
            }
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

pub fn remove_aliases(doc: &Document, diag: &mut BuildDiagnostics) {
    // collect all sets that are linked together
    let mut property_sets = PropertySets::default();

    let mut process_element = |e: &ElementRc| {
        'bindings: for (name, binding) in &e.borrow().bindings {
            for nr in &binding.borrow().two_way_bindings {
                let other_e = nr.element();
                if name == nr.name() && Rc::ptr_eq(e, &other_e) {
                    diag.push_error("Property cannot alias to itself".into(), &*binding.borrow());
                    continue 'bindings;
                }
                property_sets.add_link(NamedReference::new(e, name.clone()), nr.clone());
            }
        }
    };

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components(component, &(), &mut |e, &()| process_element(e))
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
                best = best_property(best.clone(), candidate.clone());
            }
            for x in set.iter() {
                if *x != best {
                    aliases_to_remove.insert(x.clone(), best.clone());
                }
            }
        }
    }

    doc.visit_all_used_components(|component| {
        // Do the replacements
        visit_all_named_references(component, &mut |nr: &mut NamedReference| {
            if let Some(new) = aliases_to_remove.get(nr) {
                *nr = new.clone();
            }
        })
    });

    // Remove the properties
    for (remove, to) in aliases_to_remove {
        let elem = remove.element();
        let to_elem = to.element();

        // adjust the bindings
        let old_binding = elem.borrow_mut().bindings.remove(remove.name());
        let mut old_binding = old_binding.map(RefCell::into_inner).unwrap_or_else(|| {
            // ensure that we set an expression, because the right hand side of a binding always wins,
            // and if that was not set, we must still kee the default then
            let mut b = BindingExpression::from(Expression::default_value_for_type(&to.ty()));
            b.priority = to_elem
                .borrow_mut()
                .bindings
                .get(to.name())
                .map_or(i32::MAX, |x| x.borrow().priority.saturating_add(1));
            b
        });

        remove_from_binding_expression(&mut old_binding, &to);

        let same_component = std::rc::Weak::ptr_eq(
            &elem.borrow().enclosing_component,
            &to_elem.borrow().enclosing_component,
        );
        match to_elem.borrow_mut().bindings.entry(to.name().clone()) {
            Entry::Occupied(mut e) => {
                let b = e.get_mut().get_mut();
                remove_from_binding_expression(b, &to);
                if !same_component || b.priority < old_binding.priority || !b.has_binding() {
                    b.merge_with(&old_binding);
                } else {
                    old_binding.merge_with(b);
                    *b = old_binding;
                }
            }
            Entry::Vacant(e) => {
                if same_component && old_binding.has_binding() {
                    e.insert(old_binding.into());
                }
            }
        };

        // Adjust the change callbacks
        {
            let mut elem = elem.borrow_mut();
            if let Some(old_change_callback) = elem.change_callbacks.remove(remove.name()) {
                drop(elem);
                let mut old_change_callback = old_change_callback.into_inner();
                to_elem
                    .borrow_mut()
                    .change_callbacks
                    .entry(to.name().clone())
                    .or_default()
                    .borrow_mut()
                    .append(&mut old_change_callback);
            }
        }

        // Remove the declaration
        {
            let mut elem = elem.borrow_mut();
            let used_externally = elem
                .property_analysis
                .borrow()
                .get(remove.name())
                .is_some_and(|v| v.is_read_externally || v.is_set_externally);
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
                            .entry(to.name().clone())
                            .or_default()
                            .merge(&analysis);
                    };
                }
            } else {
                // This is not a declaration, we must re-create the binding
                elem.bindings.insert(
                    remove.name().clone(),
                    BindingExpression::new_two_way(to.clone()).into(),
                );
                drop(elem);
                if remove.is_externally_modified() {
                    to.mark_as_set();
                }
            }
        }
    }
}

fn is_declaration(x: &NamedReference) -> bool {
    x.element().borrow().property_declarations.contains_key(x.name())
}

/// Out of two named reference, return the one which is the best to keep.
fn best_property(p1: NamedReference, p2: NamedReference) -> NamedReference {
    // Try to find which is the more canonical property
    macro_rules! canonical_order {
        ($x: expr) => {{
            (
                !$x.element().borrow().enclosing_component.upgrade().unwrap().is_global(),
                is_declaration(&$x),
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
