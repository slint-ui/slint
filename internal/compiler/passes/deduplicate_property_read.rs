// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Do not read twice the same property, store in a local variable instead

use crate::expression_tree::*;
use crate::langtype::Type;
use crate::object_tree::*;
use smol_str::{format_smolstr, SmolStr};
use std::cell::RefCell;
use std::collections::HashMap;

pub fn deduplicate_property_read(component: &Component) {
    visit_all_expressions(component, |expr, ty| {
        if matches!(ty(), Type::Callback { .. }) {
            // Callback handler can't be optimizes because they can have side effect.
            // But that's fine as they also do not register dependencies
            return;
        }
        process_expression(expr, &DedupPropState::default());
    });
}

struct ReadCount {
    count: usize,
    has_been_mapped: bool,
}

#[derive(Default)]
struct PropertyReadCounts {
    counts: HashMap<NamedReference, ReadCount>,
    /// If at least one element of the map has duplicates
    has_duplicate: bool,
    /// if there is an assignment of a property we currently disable this optimization
    has_set: bool,
}

#[derive(Default)]
struct DedupPropState<'a> {
    parent_state: Option<&'a DedupPropState<'a>>,
    counts: RefCell<PropertyReadCounts>,
}

impl<'a> DedupPropState<'a> {
    fn add(&self, nr: &NamedReference) {
        if self.parent_state.map_or(false, |pc| pc.add_from_children(nr)) {
            return;
        }
        let mut use_counts = self.counts.borrow_mut();
        let use_counts = &mut *use_counts;
        let has_duplicate = &mut use_counts.has_duplicate;
        use_counts
            .counts
            .entry(nr.clone())
            .and_modify(|c| {
                if c.count == 1 {
                    *has_duplicate = true;
                }
                c.count += 1;
            })
            .or_insert(ReadCount { count: 1, has_been_mapped: false });
    }

    fn add_from_children(&self, nr: &NamedReference) -> bool {
        if self.parent_state.map_or(false, |pc| pc.add_from_children(nr)) {
            return true;
        }
        let mut use_counts = self.counts.borrow_mut();
        let use_counts = &mut *use_counts;
        if let Some(c) = use_counts.counts.get_mut(nr) {
            if c.count == 1 {
                use_counts.has_duplicate = true;
            }
            c.count += 1;
            true
        } else {
            false
        }
    }

    fn get_mapping(&self, nr: &NamedReference) -> Option<SmolStr> {
        self.parent_state.and_then(|pr| pr.get_mapping(nr)).or_else(|| {
            self.counts.borrow_mut().counts.get_mut(nr).filter(|c| c.count > 1).map(|c| {
                c.has_been_mapped = true;
                map_nr(nr)
            })
        })
    }
}

fn map_nr(nr: &NamedReference) -> SmolStr {
    format_smolstr!("tmp_{}_{}", nr.element().borrow().id, nr.name())
}

fn process_expression(expr: &mut Expression, old_state: &DedupPropState) {
    if old_state.counts.borrow().has_set {
        return;
    }
    let new_state = DedupPropState { parent_state: Some(old_state), ..DedupPropState::default() };
    collect_unconditional_read_count(expr, &new_state);
    process_conditional_expressions(expr, &new_state);
    if new_state.counts.borrow().has_set {
        old_state.counts.borrow_mut().has_set = true;
    } else {
        do_replacements(expr, &new_state);
    }

    if new_state.counts.borrow().has_duplicate {
        let mut stores = vec![];
        for (nr, c) in &new_state.counts.borrow().counts {
            if c.has_been_mapped {
                stores.push(Expression::StoreLocalVariable {
                    name: map_nr(nr),
                    value: Box::new(Expression::PropertyReference(nr.clone())),
                });
            }
        }
        stores.push(std::mem::take(expr));
        *expr = Expression::CodeBlock(stores);
    }
}

// Collect all use of variable and their count, only in non conditional expression
fn collect_unconditional_read_count(expr: &Expression, result: &DedupPropState) {
    if result.counts.borrow().has_set {
        return;
    }
    match expr {
        Expression::PropertyReference(nr) => {
            result.add(nr);
        }
        //Expression::RepeaterIndexReference { element } => {}
        //Expression::RepeaterModelReference { element } => {}
        Expression::BinaryExpression { lhs, rhs: _, op: '|' | '&' } => {
            lhs.visit(|sub| collect_unconditional_read_count(sub, result))
        }
        Expression::Condition { condition, .. } => {
            condition.visit(|sub| collect_unconditional_read_count(sub, result))
        }
        Expression::SelfAssignment { .. } => {
            result.counts.borrow_mut().has_set = true;
        }
        _ => expr.visit(|sub| collect_unconditional_read_count(sub, result)),
    }
}

fn process_conditional_expressions(expr: &mut Expression, state: &DedupPropState) {
    if state.counts.borrow().has_set {
        return;
    }
    match expr {
        Expression::BinaryExpression { lhs, rhs, op: '|' | '&' } => {
            lhs.visit_mut(|sub| process_conditional_expressions(sub, state));
            process_expression(rhs, state);
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            condition.visit_mut(|sub| process_conditional_expressions(sub, state));
            process_expression(true_expr, state);
            process_expression(false_expr, state);
        }
        Expression::SelfAssignment { .. } => {
            state.counts.borrow_mut().has_set = true;
        }
        _ => expr.visit_mut(|sub| process_conditional_expressions(sub, state)),
    }
}

fn do_replacements(expr: &mut Expression, state: &DedupPropState) {
    match expr {
        Expression::PropertyReference(nr) => {
            if let Some(name) = state.get_mapping(nr) {
                let ty = expr.ty();
                *expr = Expression::ReadLocalVariable { name, ty };
            }
        }
        Expression::BinaryExpression { lhs, rhs: _, op: '|' | '&' } => {
            lhs.visit_mut(|sub| do_replacements(sub, state));
        }
        Expression::Condition { condition, .. } => {
            condition.visit_mut(|sub| do_replacements(sub, state));
        }
        _ => expr.visit_mut(|sub| do_replacements(sub, state)),
    }
}
