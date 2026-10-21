// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass removes the properties that two-way bindings make redundant.
//!
//! Properties linked by two-way bindings (`a <=> b`) hold one value at runtime.
//! The pass groups them into sets, keeps one property of each set, its master,
//! and replaces the other properties of the set by the master.
//! See [`remove_aliases`] for the steps.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression, NamedReference, TwoWayBinding};
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Maps each property to remove to the master that replaces it, and to the set they're both in.
type Mapping = HashMap<NamedReference, (NamedReference, PropertySet)>;
type PropertySet = Rc<RefCell<HashSet<NamedReference>>>;

/// Properties grouped into sets that two-way bindings link into one property.
#[derive(Default, Debug)]
struct PropertySets {
    /// The set of each property that is in a set.
    map: HashMap<NamedReference, PropertySet>,
    /// Every set, in creation order. A set that `add_link` merged into another one is empty.
    all_sets: Vec<PropertySet>,
    /// The declarations that an instance binds two ways to a struct field or a model value.
    /// Only a property of the component can host such a link, so they stay local, see [`stays_local`].
    bound_to_field_or_model: HashSet<NamedReference>,
}

impl PropertySets {
    /// Puts `p1` and `p2` in one set, merging the sets they're already in,
    /// unless [`Self::can_merge`] rejects the link.
    /// A property of an instance joins as a global when its declaration in a base component
    /// is bound two ways to that global, and doesn't [`stays_local`].
    fn add_link(&mut self, p1: NamedReference, p2: NamedReference) {
        let global_of_base_declaration = |property: &NamedReference| -> Option<NamedReference> {
            let declaration = base_declaration(&property.element().borrow(), property.name())?;
            let global = declaration
                .element()
                .borrow()
                .binding(declaration.name())?
                .two_way_bindings
                .iter()
                .find_map(|twb| match twb {
                    TwoWayBinding::Property { property: target_property, field_access }
                        if field_access.is_empty() && is_in_global(target_property) =>
                    {
                        Some(target_property.clone())
                    }
                    _ => None,
                })?;
            let is_callback = matches!(property.ty(), Type::Callback(..));
            (!is_callback && !stays_local(&declaration, &self.bound_to_field_or_model))
                .then_some(global)
        };
        let p1 = global_of_base_declaration(&p1).unwrap_or(p1);
        let p2 = global_of_base_declaration(&p2).unwrap_or(p2);
        if p1 == p2 || !self.can_merge(&p1, &p2) {
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

    /// Whether the link `p1 <=> p2` can merge the sets of `p1` and `p2`.
    /// A rejected link stays a two-way binding.
    fn can_merge(&self, p1: &NamedReference, p2: &NamedReference) -> bool {
        let mut members = vec![p1.clone(), p2.clone()];
        for p in [p1, p2] {
            if let Some(s) = self.map.get(p) {
                members.extend(s.borrow().iter().cloned());
            }
        }
        let initial_component = members[0].element().borrow().enclosing_component.clone();
        let link_is_same_component = std::rc::Weak::ptr_eq(
            &p1.element().borrow().enclosing_component,
            &p2.element().borrow().enclosing_component,
        );
        let contains_changed_handlers = members.iter().any(|property| {
            property.element().borrow().change_callbacks.contains_key(property.name())
        });
        let set_is_same_component = members.iter().all(|property| {
            std::rc::Weak::ptr_eq(
                &initial_component,
                &property.element().borrow().enclosing_component,
            )
        });
        let link_involves_global = is_in_global(p1) || is_in_global(p2);

        if !link_is_same_component {
            // We can only add a new link across components if the link involves a global and none
            // of the involved bindings (including earlier aliases) have a changed handler.
            !contains_changed_handlers && link_involves_global
        } else {
            // the new link is within the same component,
            // but the previously processed aliases may already contain another component (which must be a
            // global due to the `if` path.
            // If that is the case, only alias if no changed handlers are involved.
            !contains_changed_handlers || set_is_same_component
        }
    }
}

/// The links that [`collect_links`] finds in the two-way bindings of a document.
#[derive(Default)]
struct Links {
    property_sets: PropertySets,
    /// For each callback, the callbacks that are bound two ways to it.
    callback_aliases: HashMap<NamedReference, Vec<NamedReference>>,
}

/// Removes the properties that two-way bindings make redundant, in these steps:
/// 1. [`collect_links`] groups the properties that two-way bindings link into sets.
/// 2. [`check_callback_aliases`] warns about callbacks that alias one callback without a handler.
/// 3. [`choose_masters`] picks the master of each set, and maps the properties to remove to it.
/// 4. Every reference to a property to remove is replaced by its master.
/// 5. [`remove_alias`] moves the binding and `changed` handlers of each property to remove
///    onto its master, and removes its declaration.
/// 6. [`move_links_off_global_declarations`] moves the remaining links off the properties of
///    instances that are a global.
///
/// A link that [`PropertySets::can_merge`] rejects, and a property that [`choose_masters`] doesn't map,
/// stay two-way bindings.
pub fn remove_aliases(doc: &Document, diag: &mut BuildDiagnostics) {
    let Links { property_sets, callback_aliases } = collect_links(doc, diag);
    check_callback_aliases(&property_sets, &callback_aliases, diag);
    let aliases_to_remove = choose_masters(property_sets, diag);

    doc.visit_all_used_components(|component| {
        visit_all_named_references(component, &mut |nr: &mut NamedReference| {
            if let Some((new, _set)) = aliases_to_remove.get(nr) {
                *nr = new.clone();
            }
        })
    });

    // Process the removal in a deterministic order to ensure that changed callbacks
    // are always merged in the same order.
    let mut aliases_to_remove = aliases_to_remove.into_iter().collect::<Vec<_>>();
    aliases_to_remove.sort_by_cached_key(|(remove, _)| {
        (remove.element().borrow().id.clone(), remove.name().clone())
    });
    for (remove, (to, set)) in aliases_to_remove {
        remove_alias(&remove, &to, &set, diag);
    }

    move_links_off_global_declarations(doc);
}

/// Collects the two-way bindings between properties of all used components, without field access.
/// Reports a property that aliases itself.
fn collect_links(doc: &Document, diag: &mut BuildDiagnostics) -> Links {
    let mut links = Links::default();
    links.property_sets.bound_to_field_or_model = collect_bound_to_field_or_model(doc);

    let mut process_element = |e: &ElementRc| {
        'bindings: for (name, binding) in e.borrow().real_bindings() {
            for twb in &binding.borrow().two_way_bindings {
                if let TwoWayBinding::Property { property: target_property, field_access } = twb {
                    if !field_access.is_empty() {
                        // Don't optimize two way bindings to fields for now
                        continue;
                    }
                    let other_e = target_property.element();
                    if name == target_property.name() && Rc::ptr_eq(e, &other_e) {
                        diag.push_error(
                            "Property cannot alias to itself".into(),
                            &*binding.borrow(),
                        );
                        continue 'bindings;
                    }
                    let source = NamedReference::new(e, name.clone());
                    if matches!(source.ty(), Type::Callback(..)) {
                        let bucket =
                            links.callback_aliases.entry(target_property.clone()).or_default();
                        if !bucket.contains(&source) {
                            bucket.push(source.clone());
                        }
                    }
                    links.property_sets.add_link(source, target_property.clone());
                }
            }
        }
    };

    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components(component, &(), &mut |e, &()| process_element(e))
    });

    links
}

/// Collects [`PropertySets::bound_to_field_or_model`].
fn collect_bound_to_field_or_model(doc: &Document) -> HashSet<NamedReference> {
    let mut declarations = HashSet::new();
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components(component, &(), &mut |e, &()| {
            let e = e.borrow();
            for (name, binding) in e.real_bindings() {
                let bound_to_field_or_model = binding.borrow().two_way_bindings.iter().any(|twb| {
                    !matches!(twb, TwoWayBinding::Property { field_access, .. } if field_access.is_empty())
                });
                if bound_to_field_or_model && let Some(declaration) = base_declaration(&e, name) {
                    declarations.insert(declaration);
                }
            }
        })
    });
    declarations
}

/// Warns about callbacks bound two ways to the same callback when none of their set has a handler:
/// they share one handler slot, so only one of them can get an implementation.
fn check_callback_aliases(
    property_sets: &PropertySets,
    callback_aliases: &HashMap<NamedReference, Vec<NamedReference>>,
    diag: &mut BuildDiagnostics,
) {
    for (target, sources) in callback_aliases {
        if sources.len() < 2 {
            continue;
        }
        // if a callback has an implementation, the rest are alternate names for invoking it
        let has_implementation = property_sets.map.get(target).is_some_and(|set_rc| {
            set_rc.borrow().iter().any(|nr| {
                let elem = nr.element();
                let elem = elem.borrow();
                elem.binding(nr.name())
                    .is_some_and(|b| !matches!(b.value_expression(), Expression::Invalid))
            })
        });
        if has_implementation {
            continue;
        }
        for nr in sources {
            let other_names = sources
                .iter()
                .filter(|other| *other != nr)
                .map(|other| format!("'{}'", other.name()))
                .collect::<Vec<_>>()
                .join(", ");
            let elem = nr.element();
            let elem = elem.borrow();
            if let Some(b) = elem.binding(nr.name()) {
                // Only a warning: this compiled fine up to 1.17 and code relies on it.
                diag.push_warning(
                    format!(
                        "Callback '{}' shares a handler slot with {other_names}, so only one of them can have an implementation",
                        nr.name()
                    ),
                    &*b,
                );
            }
        }
    }
}

/// Picks the master of each set with [`best_property`], and maps the other properties of the set to it.
/// Only the references to the master are kept, and only the master keeps its binding.
fn choose_masters(property_sets: PropertySets, diag: &mut BuildDiagnostics) -> Mapping {
    let mut aliases_to_remove = Mapping::new();

    for set_rc in property_sets.all_sets {
        let set = set_rc.borrow();
        check_global_callback_implementations(&set, diag);

        let Some(best) = set.iter().cloned().reduce(best_property) else { continue };
        let best_is_global = is_in_global(&best);
        for x in set.iter() {
            if *x != best {
                // The replacement in `remove_aliases` leaves a property that `stays_local`
                // two-way bound to `best`.
                if best_is_global
                    && !is_in_global(x)
                    && stays_local(x, &property_sets.bound_to_field_or_model)
                {
                    continue;
                }
                aliases_to_remove.insert(x.clone(), (best.clone(), Rc::clone(&set_rc)));
            }
        }
    }
    aliases_to_remove
}

/// Reports the callbacks of `set` that more than one global implements.
fn check_global_callback_implementations(
    set: &HashSet<NamedReference>,
    diag: &mut BuildDiagnostics,
) {
    // Globals are singletons, so a callback aliased across globals must have at most
    // one implementation. More than one handler in the set is an ambiguous conflict.
    // (For non-global elements multiple handlers are fine: instances legitimately
    // override a base's handler, resolved in `merge_binding` by binding priority.)
    let implementers: Vec<NamedReference> = set
        .iter()
        .filter(|nr| {
            if !matches!(nr.ty(), Type::Callback(..)) {
                return false;
            }
            is_in_global(nr)
                && nr
                    .element()
                    .borrow()
                    .binding(nr.name())
                    .is_some_and(|b| !matches!(b.value_expression(), Expression::Invalid))
        })
        .cloned()
        .collect();
    if implementers.len() > 1 {
        for nr in &implementers {
            let elem = nr.element();
            let elem = elem.borrow();
            if let Some(b) = elem.binding(nr.name()) {
                diag.push_error(
                    format!("Callback '{}' is implemented in more than one global", nr.name()),
                    &*b,
                );
            }
        }
    }
}

/// Removes `remove`, whose references already name its master `to` instead.
/// `set` is the set of both.
fn remove_alias(
    remove: &NamedReference,
    to: &NamedReference,
    set: &PropertySet,
    diag: &mut BuildDiagnostics,
) {
    let mut old_binding = take_binding(remove, to);
    if is_in_global(to) {
        move_links_outside_set(&mut old_binding, to, set);
    }
    merge_binding(remove, to, old_binding, diag);
    move_change_callbacks(remove, to);
    remove_declaration(remove, to);
}

/// Takes the binding of `remove`, without its link to `to`.
fn take_binding(remove: &NamedReference, to: &NamedReference) -> BindingExpression {
    let old_binding = remove.element().borrow_mut().take_binding(remove.name());
    let mut old_binding = old_binding.unwrap_or_else(|| {
        // ensure that we set an expression, because the right hand side of a binding always wins,
        // and if that was not set, we must still keep the default then
        let mut b = BindingExpression::from(Expression::default_value_for_type(&to.ty()));
        b.priority = to
            .element()
            .borrow()
            .binding(to.name())
            .map_or(i32::MAX, |to_binding| to_binding.priority.saturating_add(1));
        b
    });
    remove_from_binding_expression(&mut old_binding, to);
    old_binding
}

/// Takes the two-way bindings to properties outside `set` off `binding`,
/// and puts each on its other end as `target <=> to` instead. `to` must be a global.
///
/// Such a link only remains because `PropertySets::can_merge` rejected it,
/// for example because it would pull a `changed` handler into a global.
/// Merging `binding` onto the global `to` would leave the singleton referencing
/// an instance element it can't resolve ("accessing deleted parent" at runtime).
/// `target` can always resolve `to`, because a reference to a global is a direct global lookup.
///
/// The reverse doesn't hold for an instance `to`: a reference to it is resolved by walking up
/// the parent chain, which `target` may not reach, for example when `to` is in a nested
/// sub-component (the `todo` demo panics so).
/// Then `merge_binding` keeps the binding on `to`, in the same direction as `remove <=> target`.
fn move_links_outside_set(binding: &mut BindingExpression, to: &NamedReference, set: &PropertySet) {
    binding.two_way_bindings.retain(|twb| {
        let TwoWayBinding::Property { property, field_access } = twb else { return true };
        if !field_access.is_empty() || set.borrow().contains(property) {
            // Field-access bindings aren't aliased (see `collect_links`), and a target that *is* in
            // the set was folded into `to` already, so `merge_binding` handles it.
            return true;
        }
        add_two_way_link(property, to);
        false
    });
}

/// Merges `old_binding`, the binding that `remove` had, into the binding of `to`.
/// Within a component, the binding that a state changes wins, and then the one with the
/// higher priority; across components, the binding of `to` wins.
/// Reports when states change both ends.
fn merge_binding(
    remove: &NamedReference,
    to: &NamedReference,
    mut old_binding: BindingExpression,
    diag: &mut BuildDiagnostics,
) {
    let elem = remove.element();
    let to_elem = to.element();
    let same_component = std::rc::Weak::ptr_eq(
        &elem.borrow().enclosing_component,
        &to_elem.borrow().enclosing_component,
    );
    // Globals are singletons, so a handler an aliasing global provides for another
    // global's callback must be carried over to the master, just like within a component.
    let both_global = is_in_global(remove) && is_in_global(to);

    let mut to_elem = to_elem.borrow_mut();
    // use if let else here instead of match so that to_elem is no longer borrowed
    // in the else path.
    if let Some(mut b) = to_elem.binding_mut(to.name()) {
        let b = &mut *b;
        remove_from_binding_expression(b, to);
        if same_component
            && let Some(location) = b.state_change()
            && let Some(other_location) = old_binding.state_change()
        {
            diag.push_error(
                format!(
                    "'{}' and '{}' are one property because of a two-way binding, so a state can only change one of them",
                    to.name(),
                    remove.name()
                ),
                location,
            );
            diag.push_note_with_span(
                format!("'{}' is changed here", remove.name()),
                other_location.clone(),
            );
        }
        // A state's binding also carries the value the property has while no state
        // applies, so it isn't a competitor here: whichever end of the alias a state
        // changed keeps its binding, or the state change is lost (#1950).
        let rank =
            |binding: &BindingExpression| (binding.state_change().is_none(), binding.priority);
        let keep_master = !same_component || !b.has_binding() || rank(b) < rank(&old_binding);
        if keep_master {
            b.merge_with(&old_binding);
        } else {
            old_binding.merge_with(b);
            *b = old_binding;
        }
    } else {
        if (same_component || both_global)
            && (old_binding.has_binding() || old_binding.animation.is_some())
        {
            to_elem.set_binding(to.name().clone(), old_binding);
        }
    }
}

/// Moves the `changed` handlers of `remove` to `to`.
fn move_change_callbacks(remove: &NamedReference, to: &NamedReference) {
    let old_change_callback = remove.element().borrow_mut().change_callbacks.remove(remove.name());
    if let Some(old_change_callback) = old_change_callback {
        let mut old_change_callback = old_change_callback.into_inner();
        to.element()
            .borrow_mut()
            .change_callbacks
            .entry(to.name().clone())
            .or_default()
            .borrow_mut()
            .append(&mut old_change_callback);
    }
}

/// Removes the declaration of `remove`, or keeps it as an alias of `to` when the public API
/// or another component uses it.
/// When `remove` isn't declared in its element, binds it two ways to `to` instead.
fn remove_declaration(remove: &NamedReference, to: &NamedReference) {
    let elem = remove.element();
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
        elem.set_binding(remove.name().clone(), BindingExpression::new_two_way(to.clone().into()));
        drop(elem);
        if remove.is_externally_modified() {
            to.mark_as_set();
        }
    }
}

/// Drops the links of the properties of instances whose declaration is now an alias of a global
/// (see [`PropertySets::add_link`]), and puts the ones to other properties on their other end
/// as `other <=> global`, which always resolves.
fn move_links_off_global_declarations(doc: &Document) {
    let global_declaration = |e: &ElementRc, name: &SmolStr| -> Option<NamedReference> {
        let declaration = base_declaration(&e.borrow(), name)?;
        let target = declaration
            .element()
            .borrow()
            .property_declarations
            .get(declaration.name())?
            .is_alias
            .clone()?;
        is_in_global(&target).then_some(target)
    };
    let mut moved_links = Vec::new();
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components(component, &(), &mut |e, &()| {
            for (name, binding) in e.borrow().real_bindings() {
                if binding.borrow().two_way_bindings.is_empty() {
                    continue;
                }
                let Some(global) = global_declaration(e, name) else { continue };
                binding.borrow_mut().two_way_bindings.retain(|twb| match twb {
                    TwoWayBinding::Property { property, field_access }
                        if field_access.is_empty() =>
                    {
                        let other = global_declaration(&property.element(), property.name());
                        if other.as_ref().unwrap_or(property) != &global {
                            moved_links.push((property.clone(), global.clone()));
                        }
                        false
                    }
                    _ => true,
                });
                global.mark_as_set();
            }
        })
    });
    for (property, global) in moved_links {
        add_two_way_link(&property, &global);
    }
}

/// The declaration of the property `name` of `e` in a base component.
fn base_declaration(e: &Element, name: &SmolStr) -> Option<NamedReference> {
    let component = e.declaring_base_component(name)?;
    Some(NamedReference::new(&component.root_element, name.clone()))
}

/// Whether `property` keeps its own property instead of being aliased to a global:
/// a global can't run its `changed` handler or animation, or host its partial two-way bindings.
fn stays_local(
    property: &NamedReference,
    bound_to_field_or_model: &HashSet<NamedReference>,
) -> bool {
    let element = property.element();
    let element = element.borrow();
    element.change_callbacks.contains_key(property.name())
        || element.binding(property.name()).is_some_and(|binding| binding.animation.is_some())
        || bound_to_field_or_model.contains(property)
}

/// Binds `property` two ways to `to`, keeping its other bindings.
fn add_two_way_link(property: &NamedReference, to: &NamedReference) {
    let element = property.element();
    let mut element = element.borrow_mut();
    if let Some(mut binding) = element.binding_mut(property.name()) {
        if !binding.two_way_bindings.iter().any(|x| x.property() == Some(to)) {
            binding.two_way_bindings.push(to.clone().into());
        }
        return;
    }
    element.set_binding(property.name().clone(), BindingExpression::new_two_way(to.clone().into()));
}

fn is_in_global(x: &NamedReference) -> bool {
    x.element().borrow().enclosing_component.upgrade().unwrap().is_global()
}

/// Out of two named reference, return the one which is the best to keep.
fn best_property(p1: NamedReference, p2: NamedReference) -> NamedReference {
    // Try to find which is the more canonical property
    let canonical_order = |x: &NamedReference| {
        let element = x.element();
        let element = element.borrow();
        let is_declaration = element.property_declarations.contains_key(x.name());
        (!is_in_global(x), is_declaration, element.id.clone(), x.name().clone())
    };

    if canonical_order(&p1) < canonical_order(&p2) { p1 } else { p2 }
}

/// Remove the `to` from the two_way_bindings
fn remove_from_binding_expression(expression: &mut BindingExpression, to: &NamedReference) {
    expression.two_way_bindings.retain(|x| x.property() != Some(to));
}
