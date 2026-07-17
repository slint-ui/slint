// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The candidate bookkeeping of the layout search: measures, decision lists,
//! and Pareto frontiers of measures.
//!
//! Everything here is `MAX_LINE_WIDTH_DESIGN.md`'s "The algorithm, briefly"
//! made concrete; the resolver that produces these values lives alongside in
//! this module's sibling files.

// Everything in this file is exercised by the width tests until the search is
// wired into the pipeline.
#![allow(dead_code)]

use super::cost::Cost;
use super::{GroupId, Variant};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The decisions accumulated along one candidate layout, as a persistent
/// binary tree so that combining two candidates' decisions is O(1) and never
/// copies. Flattened into a map once, for the single winning candidate.
#[derive(Debug, Clone, Default)]
pub enum DecisionList {
    #[default]
    Empty,
    Decision(GroupId, Variant),
    Both(Rc<DecisionList>, Rc<DecisionList>),
}

/// The derived recursive drop would overflow the stack on the long chains a
/// big document accumulates (one link per decided group); dismantle the tree
/// iteratively instead.
impl Drop for DecisionList {
    fn drop(&mut self) {
        let DecisionList::Both(first_left, first_right) = self else { return };
        // Detach children into a worklist, leaving shared shallow sentinels
        // behind (a nested drop then finds only sentinels and stays shallow).
        // A child shared with other owners is left to them: `Rc::into_inner`
        // dismantles only sole ownership.
        let sentinel = Rc::new(DecisionList::Empty);
        let mut pending = vec![
            std::mem::replace(first_left, sentinel.clone()),
            std::mem::replace(first_right, sentinel.clone()),
        ];
        while let Some(child) = pending.pop() {
            if let Some(mut child) = Rc::into_inner(child) {
                if let DecisionList::Both(left, right) = &mut child {
                    pending.push(std::mem::replace(left, sentinel.clone()));
                    pending.push(std::mem::replace(right, sentinel.clone()));
                }
            }
        }
    }
}

impl DecisionList {
    /// Concatenate two decision lists in O(1). The two sides must be
    /// disjoint — they always are by construction, because every group's
    /// choice is recorded exactly once, at its own `Choice` node.
    pub fn append(self, other: DecisionList) -> DecisionList {
        match (self, other) {
            (DecisionList::Empty, other) => other,
            (this, DecisionList::Empty) => this,
            (this, other) => DecisionList::Both(Rc::new(this), Rc::new(other)),
        }
    }

    /// Flatten into a map, debug-asserting the disjointness `append` assumes.
    pub fn flatten_to_map(&self) -> HashMap<GroupId, Variant> {
        let mut decisions = HashMap::new();
        let mut pending = vec![self];
        while let Some(list) = pending.pop() {
            match list {
                DecisionList::Empty => {}
                DecisionList::Decision(group, variant) => {
                    let previous = decisions.insert(*group, *variant);
                    debug_assert!(previous.is_none(), "group {group:?} decided twice");
                }
                DecisionList::Both(left, right) => {
                    pending.push(left);
                    pending.push(right);
                }
            }
        }
        decisions
    }
}

/// One surviving candidate layout, summarized without rendering it: what it
/// costs, where its last line ends (the column the suffix starts from), and
/// the group decisions it took.
#[derive(Debug, Clone)]
pub struct Measure {
    pub last_line_width: u32,
    pub cost: Cost,
    pub decisions: DecisionList,
}

impl Measure {
    /// Combine a prefix and its suffix into one candidate.
    pub fn combine(prefix: &Measure, suffix: &Measure) -> Measure {
        Measure {
            last_line_width: suffix.last_line_width,
            cost: prefix.cost + suffix.cost,
            decisions: prefix.decisions.clone().append(suffix.decisions.clone()),
        }
    }
}

/// A lazily computed measure. Tainted branches of the search must not do any
/// work unless they are the only option left — that laziness is what bounds
/// the running time past the computation width — so the thunk stays
/// unevaluated until [`LazyMeasure::force`], and is shared by reference.
pub struct LazyMeasure(RefCell<LazyState>);

enum LazyState {
    Pending(Box<dyn FnOnce() -> Measure>),
    /// The thunk is running. Observable only on a reentrant `force` (a cycle
    /// between lazies — impossible for an acyclic document) or after a
    /// panicked thunk; both must not silently yield a placeholder measure,
    /// whose zero cost would win every merge.
    Evaluating,
    Forced(Measure),
}

impl LazyMeasure {
    pub fn new(compute: impl FnOnce() -> Measure + 'static) -> Rc<LazyMeasure> {
        Rc::new(LazyMeasure(RefCell::new(LazyState::Pending(Box::new(compute)))))
    }

    /// Evaluate the thunk (once; the result is kept).
    pub fn force(&self) -> Measure {
        let taken = std::mem::replace(&mut *self.0.borrow_mut(), LazyState::Evaluating);
        let measure = match taken {
            LazyState::Forced(measure) => measure,
            // Run the thunk without holding the borrow, so a thunk forcing
            // *other* lazies (the tainted chains do) never trips the
            // RefCell.
            LazyState::Pending(compute) => compute(),
            LazyState::Evaluating => {
                panic!("a lazy measure forced itself recursively, or is reused after a panic")
            }
        };
        *self.0.borrow_mut() = LazyState::Forced(measure.clone());
        measure
    }
}

/// The surviving candidates of one (sub-document, start column).
///
/// `Set` is a Pareto frontier: sorted by cost strictly ascending, which on a
/// valid frontier means `last_line_width` strictly descending — a candidate
/// only earns its place by paying more for a shorter last line that a long
/// suffix may need. `Tainted` is the single greedy fallback once the search
/// blew past the computation width; it carries no optimality claim and must
/// stay unevaluated (see [`LazyMeasure`]).
pub enum MeasureSet {
    Set(Vec<Measure>),
    Tainted(Rc<LazyMeasure>),
}

/// Reduce candidates to their Pareto frontier.
///
/// Sort by cost ascending (stable, so on an exact cost-and-width tie the
/// earlier candidate survives — the deterministic tie-break idempotency
/// relies on), then keep a candidate only when its last line is strictly
/// shorter than every already-kept one. Keeping instead the cheapest per
/// width — the tempting backwards variant — would discard exactly the
/// pay-more-for-a-shorter-line candidates the frontier exists for.
pub fn dedup(mut measures: Vec<Measure>) -> Vec<Measure> {
    // The sort's stability is load-bearing: it is what makes the earlier
    // candidate survive exact ties.
    measures.sort_by_key(|measure| (measure.cost, measure.last_line_width));
    let mut frontier: Vec<Measure> = Vec::new();
    for measure in measures {
        // The resolver taints instead of producing wider measures; a wider
        // one here would break the frontier's ≤ W+1 size bound.
        debug_assert!(measure.last_line_width <= super::COMPUTATION_WIDTH);
        // Kept widths decrease strictly, so comparing against the last kept
        // measure compares against all of them.
        if frontier.last().is_none_or(|kept| measure.last_line_width < kept.last_line_width) {
            frontier.push(measure);
        }
    }
    frontier
}

/// Merge two candidate sets, keeping the Pareto frontier. An untainted set
/// always beats a tainted one; two tainted sets keep the *left* one, so a
/// hopeless overflow degrades to the input-matching variant the caller puts
/// on the left.
pub fn merge(left: MeasureSet, right: MeasureSet) -> MeasureSet {
    // An empty Set would beat a Tainted fallback and lose the only viable
    // candidate; the resolver never produces one.
    debug_assert!(!matches!(&left, MeasureSet::Set(measures) if measures.is_empty()));
    debug_assert!(!matches!(&right, MeasureSet::Set(measures) if measures.is_empty()));
    match (left, right) {
        (MeasureSet::Set(mut left_measures), MeasureSet::Set(right_measures)) => {
            // Stable sort in `dedup` — left before right wins exact ties.
            left_measures.extend(right_measures);
            MeasureSet::Set(dedup(left_measures))
        }
        (MeasureSet::Set(measures), MeasureSet::Tainted(_))
        | (MeasureSet::Tainted(_), MeasureSet::Set(measures)) => MeasureSet::Set(measures),
        (left @ MeasureSet::Tainted(_), MeasureSet::Tainted(_)) => left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmt::width::PAGE_WIDTH;

    /// A measure with a distinguishable cost: `newlines` sets the height
    /// component, `width` the last-line width.
    fn measure(width: u32, newlines: u32) -> Measure {
        let mut cost = Cost::ZERO;
        for _ in 0..newlines {
            cost = cost + Cost::newline(0);
        }
        Measure { last_line_width: width, cost, decisions: DecisionList::Empty }
    }

    fn widths(measures: &[Measure]) -> Vec<u32> {
        measures.iter().map(|measure| measure.last_line_width).collect()
    }

    #[test]
    fn dedup_keeps_the_pay_more_for_a_shorter_line_candidates() {
        // Cheap-but-long and pricey-but-short both survive; anything that is
        // beaten on both axes is pruned. A backwards implementation keeping
        // only the cheapest candidate returns just [80].
        let frontier = dedup(vec![
            measure(80, 0), // cheapest, longest — survives
            measure(40, 1), // pays one newline for a shorter line — survives
            measure(60, 2), // costs more than the shorter 40 — pruned
            measure(10, 3), // shortest of all — survives
            measure(90, 2), // longer AND pricier than 80/0 — pruned
        ]);
        assert_eq!(widths(&frontier), [80, 40, 10]);
        // The frontier invariant: cost strictly ascending, width strictly
        // descending.
        for pair in frontier.windows(2) {
            assert!(pair[0].cost < pair[1].cost);
            assert!(pair[0].last_line_width > pair[1].last_line_width);
        }
    }

    #[test]
    fn dedup_drops_equal_cost_duplicates_keeping_the_earlier() {
        let mut first = measure(50, 1);
        first.decisions = DecisionList::Decision(GroupId(1), Variant::SingleLine);
        let mut second = measure(50, 1);
        second.decisions = DecisionList::Decision(GroupId(2), Variant::Multiline);

        let frontier = dedup(vec![first, second]);
        assert_eq!(frontier.len(), 1);
        // The earlier measure survived the exact tie.
        assert!(matches!(frontier[0].decisions, DecisionList::Decision(GroupId(1), _)));
    }

    #[test]
    fn dedup_on_equal_cost_prefers_the_shorter_line() {
        // Equal cost, different widths: the shorter last line dominates
        // (same price, strictly more room for the suffix).
        let frontier = dedup(vec![measure(70, 1), measure(30, 1)]);
        assert_eq!(widths(&frontier), [30]);
    }

    #[test]
    fn merge_prefers_untainted_sets_and_does_not_force_the_tainted_side() {
        let tainted = || {
            MeasureSet::Tainted(LazyMeasure::new(|| {
                panic!("a tainted branch must stay unevaluated while a set survives")
            }))
        };
        for merged in [
            merge(MeasureSet::Set(vec![measure(10, 0)]), tainted()),
            merge(tainted(), MeasureSet::Set(vec![measure(10, 0)])),
        ] {
            match merged {
                MeasureSet::Set(measures) => assert_eq!(widths(&measures), [10]),
                MeasureSet::Tainted(_) => panic!("the untainted set must win"),
            }
        }
    }

    #[test]
    fn merging_two_tainted_sets_keeps_the_left_one() {
        let left = MeasureSet::Tainted(LazyMeasure::new(|| measure(11, 0)));
        let right = MeasureSet::Tainted(LazyMeasure::new(|| measure(22, 0)));
        match merge(left, right) {
            MeasureSet::Tainted(lazy) => assert_eq!(lazy.force().last_line_width, 11),
            MeasureSet::Set(_) => panic!("two tainted sides stay tainted"),
        }
    }

    #[test]
    fn merge_interleaves_both_frontiers() {
        // Both sides contribute survivors, in interleaved cost order; a
        // merge that discards either whole side cannot produce this.
        let left = MeasureSet::Set(vec![measure(80, 0), measure(20, 3)]);
        let right = MeasureSet::Set(vec![measure(50, 1), measure(10, 4)]);
        match merge(left, right) {
            MeasureSet::Set(measures) => assert_eq!(widths(&measures), [80, 50, 20, 10]),
            MeasureSet::Tainted(_) => unreachable!(),
        }

        // A right-side measure can dominate a left-side one.
        let left = MeasureSet::Set(vec![measure(80, 2)]);
        let right = MeasureSet::Set(vec![measure(30, 1)]);
        match merge(left, right) {
            MeasureSet::Set(measures) => assert_eq!(widths(&measures), [30]),
            MeasureSet::Tainted(_) => unreachable!(),
        }
    }

    #[test]
    fn deep_decision_lists_flatten_and_drop_without_recursion() {
        // A decision per group accumulates long chains on big documents;
        // both the flattening and the drop must not recurse per link.
        let mut decisions = DecisionList::Empty;
        let count = 100_000;
        for index in 0..count {
            decisions =
                decisions.append(DecisionList::Decision(GroupId(index), Variant::SingleLine));
        }
        assert_eq!(decisions.flatten_to_map().len(), count as usize);
        drop(decisions);
    }

    #[test]
    fn merge_keeps_the_left_side_on_exact_ties() {
        let mut left = measure(50, 1);
        left.decisions = DecisionList::Decision(GroupId(1), Variant::SingleLine);
        let mut right = measure(50, 1);
        right.decisions = DecisionList::Decision(GroupId(2), Variant::SingleLine);

        match merge(MeasureSet::Set(vec![left]), MeasureSet::Set(vec![right])) {
            MeasureSet::Set(measures) => {
                assert_eq!(measures.len(), 1);
                assert!(matches!(measures[0].decisions, DecisionList::Decision(GroupId(1), _)));
            }
            MeasureSet::Tainted(_) => unreachable!(),
        }
    }

    #[test]
    fn lazy_measures_evaluate_once_and_remember() {
        use std::cell::Cell;
        let runs = Rc::new(Cell::new(0));
        let counter = runs.clone();
        let lazy = LazyMeasure::new(move || {
            counter.set(counter.get() + 1);
            measure(7, 0)
        });
        assert_eq!(lazy.force().last_line_width, 7);
        assert_eq!(lazy.force().last_line_width, 7);
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn combine_sums_costs_and_takes_the_suffix_width() {
        let prefix = Measure {
            last_line_width: 42,
            cost: Cost::newline(0),
            decisions: DecisionList::Decision(GroupId(1), Variant::Multiline),
        };
        let suffix = Measure {
            last_line_width: 13,
            cost: Cost::text(PAGE_WIDTH, 2),
            decisions: DecisionList::Decision(GroupId(2), Variant::SingleLine),
        };
        let combined = Measure::combine(&prefix, &suffix);
        assert_eq!(combined.last_line_width, 13);
        assert_eq!(combined.cost, Cost::newline(0) + Cost::text(PAGE_WIDTH, 2));
        let decisions = combined.decisions.flatten_to_map();
        assert_eq!(decisions[&GroupId(1)], Variant::Multiline);
        assert_eq!(decisions[&GroupId(2)], Variant::SingleLine);
    }

    #[test]
    #[should_panic(expected = "decided twice")]
    #[cfg(debug_assertions)]
    fn flattening_a_twice_decided_group_is_caught() {
        let once = DecisionList::Decision(GroupId(1), Variant::SingleLine);
        let twice = once.clone().append(once);
        twice.flatten_to_map();
    }
}
