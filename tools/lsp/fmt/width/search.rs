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
use super::doc::{Doc, DocId, DocumentArena};
use super::{COMPUTATION_WIDTH, GroupId, Variant};
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

    /// Record one group's decision on this candidate.
    fn record_decision(&mut self, group: GroupId, variant: Variant) {
        self.decisions =
            std::mem::take(&mut self.decisions).append(DecisionList::Decision(group, variant));
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

    /// A lazy measure that transforms this one — without forcing anything
    /// now. This is the laziness-preserving shape every tainted-branch
    /// adjustment must use.
    pub fn map(
        self: Rc<LazyMeasure>,
        transform: impl FnOnce(Measure) -> Measure + 'static,
    ) -> Rc<LazyMeasure> {
        LazyMeasure::new(move || transform(self.force()))
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
#[derive(Clone)]
pub enum MeasureSet {
    Set(Vec<Measure>),
    Tainted(Rc<LazyMeasure>),
}

impl MeasureSet {
    /// Record this group's decision on every candidate, adding the deviation
    /// penalty when this is the branch that flips the author's layout. The
    /// penalty must land before the branch is first merged against its
    /// sibling — pruning compares the two with the penalty already in place.
    /// Operates on this branch's own copies; the child body's memo entry
    /// stays untouched and shared.
    fn decided(self, group: GroupId, variant: Variant, penalized: bool) -> MeasureSet {
        let decide = move |mut measure: Measure| {
            if penalized {
                measure.cost = measure.cost + Cost::DEVIATION;
            }
            measure.record_decision(group, variant);
            measure
        };
        match self {
            MeasureSet::Set(measures) => {
                MeasureSet::Set(measures.into_iter().map(decide).collect())
            }
            MeasureSet::Tainted(lazy) => MeasureSet::Tainted(lazy.map(decide)),
        }
    }
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

/// The search itself: finds the cheapest assignment of a variant to every
/// group of a choice document.
pub struct Resolver {
    /// Shared with the tainted thunks, which resolve greedily on demand.
    arena: Rc<DocumentArena>,
    /// Memoized per (doc, start column). Untainted columns are bounded by
    /// the computation width and sub-documents are shared, so the table
    /// stays linear in the document.
    memo: HashMap<(DocId, u32), MeasureSet>,
}

impl Resolver {
    pub fn new(arena: Rc<DocumentArena>) -> Resolver {
        Resolver { arena, memo: HashMap::new() }
    }

    /// The winning variant per group. A group can be absent from the map:
    /// its choice was never explicit (flattening forbidden), or it sat
    /// inside an ancestor's chosen single-line body — the emitter's lookup
    /// rule covers both.
    pub fn resolve_widths(&mut self, root: DocId) -> HashMap<GroupId, Variant> {
        match self.resolve(root, 0) {
            // Frontier costs ascend strictly: the optimum is the first entry.
            MeasureSet::Set(measures) => measures[0].decisions.flatten_to_map(),
            // Every layout blew past the computation width. Force the greedy
            // fallback — best effort, still valid.
            MeasureSet::Tainted(lazy) => lazy.force().decisions.flatten_to_map(),
        }
    }

    fn resolve(&mut self, doc: DocId, column: u32) -> MeasureSet {
        if let Some(known) = self.memo.get(&(doc, column)) {
            return known.clone();
        }
        let arena = self.arena.clone();
        let result = match arena.get(doc) {
            &Doc::Text { width } => leaf_set(text_measure(column, width)),
            &Doc::Newline { indent_width } => leaf_set(newline_measure(indent_width)),
            Doc::Concat(parts) => self.resolve_concat(parts, column),
            &Doc::Choice { group, single_line, multiline, penalized } => {
                let flat = self.resolve(single_line, column).decided(
                    group,
                    Variant::SingleLine,
                    penalized == Variant::SingleLine,
                );
                let broken = self.resolve(multiline, column).decided(
                    group,
                    Variant::Multiline,
                    penalized == Variant::Multiline,
                );
                // The input-matching (unpenalized) side goes left: merge
                // keeps the left side when both are tainted, so a hopeless
                // overflow degrades to the author's layout.
                match penalized {
                    Variant::SingleLine => merge(broken, flat),
                    Variant::Multiline => merge(flat, broken),
                }
            }
        };
        self.memo.insert((doc, column), result.clone());
        result
    }

    fn resolve_concat(&mut self, parts: &[DocId], column: u32) -> MeasureSet {
        let mut result = MeasureSet::Set(vec![empty_measure(column)]);
        for (index, part) in parts.iter().enumerate() {
            match result {
                MeasureSet::Set(prefixes) => result = self.concat_measures(prefixes, *part),
                MeasureSet::Tainted(lazy) => {
                    // Once the prefix is tainted, bundle every remaining part
                    // into the one thunk: the work happens only if this
                    // branch is the last resort, and forcing must not
                    // recurse per part.
                    let remaining: Vec<DocId> = parts[index..].to_vec();
                    let arena = self.arena.clone();
                    result = MeasureSet::Tainted(
                        lazy.map(move |prefix| greedy_concat(&arena, prefix, &remaining)),
                    );
                    break;
                }
            }
        }
        result
    }

    /// Resolve `right` once per distinct prefix end column and merge the
    /// per-prefix outcomes. Merging (rather than collecting) is what keeps
    /// one prefix's tainted suffix from tainting the others; the whole
    /// concat taints only when every prefix's suffix did.
    fn concat_measures(&mut self, prefixes: Vec<Measure>, right: DocId) -> MeasureSet {
        let mut merged: Option<MeasureSet> = None;
        // Frontier order — cheapest first — makes the cheapest prefix the
        // "left" side of every merge, which is also the tie-break winner.
        for prefix in prefixes {
            let combined = match self.resolve(right, prefix.last_line_width) {
                MeasureSet::Set(suffixes) => MeasureSet::Set(dedup(
                    suffixes.iter().map(|suffix| Measure::combine(&prefix, suffix)).collect(),
                )),
                // Nothing is forced here: the suffix stays lazy inside the
                // combined thunk.
                MeasureSet::Tainted(lazy) => {
                    MeasureSet::Tainted(lazy.map(move |suffix| Measure::combine(&prefix, &suffix)))
                }
            };
            merged = Some(match merged {
                None => combined,
                Some(others) => merge(others, combined),
            });
        }
        merged.expect("a frontier is never empty")
    }
}

/// Greedy resolution for tainted branches: take the author's variant at
/// every choice — no frontier, no optimality claim, just a valid layout.
/// Runs only when a tainted thunk is forced, and only the `decisions` of the
/// forced result are ever consumed — a tainted measure's cost and width are
/// never compared against anything.
fn resolve_greedy(arena: &DocumentArena, doc: DocId, column: u32) -> Measure {
    match arena.get(doc) {
        &Doc::Text { width } => text_measure(column, width),
        &Doc::Newline { indent_width } => newline_measure(indent_width),
        Doc::Concat(parts) => greedy_concat(arena, empty_measure(column), parts),
        &Doc::Choice { group, single_line, multiline, penalized } => {
            let (variant, body) = match penalized {
                // The penalized variant deviates from the input; greedy
                // keeps the author's layout.
                Variant::SingleLine => (Variant::Multiline, multiline),
                Variant::Multiline => (Variant::SingleLine, single_line),
            };
            let mut measure = resolve_greedy(arena, body, column);
            measure.record_decision(group, variant);
            measure
        }
    }
}

/// Greedily resolve `parts` as the continuation of `measure`. Runs only
/// inside forced tainted thunks.
fn greedy_concat(arena: &DocumentArena, mut measure: Measure, parts: &[DocId]) -> Measure {
    for part in parts {
        let suffix = resolve_greedy(arena, *part, measure.last_line_width);
        measure = Measure::combine(&measure, &suffix);
    }
    measure
}

/// The frontier of a single leaf measure — or the tainted fallback when the
/// leaf already ends past the computation width.
fn leaf_set(measure: Measure) -> MeasureSet {
    if measure.last_line_width > COMPUTATION_WIDTH {
        MeasureSet::Tainted(LazyMeasure::new(move || measure))
    } else {
        MeasureSet::Set(vec![measure])
    }
}

fn text_measure(column: u32, width: u32) -> Measure {
    Measure {
        last_line_width: column + width,
        cost: Cost::text(column, width),
        decisions: DecisionList::Empty,
    }
}

fn newline_measure(indent_width: u32) -> Measure {
    Measure {
        last_line_width: indent_width,
        cost: Cost::newline(indent_width),
        decisions: DecisionList::Empty,
    }
}

fn empty_measure(column: u32) -> Measure {
    Measure { last_line_width: column, cost: Cost::ZERO, decisions: DecisionList::Empty }
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

    /// A choice document builder for resolver tests.
    struct DocBuilder(DocumentArena);

    impl DocBuilder {
        fn new() -> DocBuilder {
            DocBuilder(DocumentArena::default())
        }

        fn text(&mut self, width: u32) -> DocId {
            self.0.alloc(Doc::Text { width })
        }

        fn newline(&mut self, indent_width: u32) -> DocId {
            self.0.alloc(Doc::Newline { indent_width })
        }

        fn concat(&mut self, parts: Vec<DocId>) -> DocId {
            self.0.alloc(Doc::Concat(parts))
        }

        fn choice(
            &mut self,
            group: u32,
            single_line: DocId,
            multiline: DocId,
            penalized: Variant,
        ) -> DocId {
            self.0.alloc(Doc::Choice { group: GroupId(group), single_line, multiline, penalized })
        }

        fn resolve(self, root: DocId) -> HashMap<GroupId, Variant> {
            Resolver::new(Rc::new(self.0)).resolve_widths(root)
        }
    }

    /// `group{ Text(flat_width) | Text(first) Newline(indent) Text(second) }`
    /// — the canonical one-group document.
    fn one_group_document(
        builder: &mut DocBuilder,
        group: u32,
        penalized: Variant,
        flat_width: u32,
        broken_widths: (u32, u32, u32),
    ) -> DocId {
        let single_line = builder.text(flat_width);
        let (first, indent, second) = broken_widths;
        let first = builder.text(first);
        let indent = builder.newline(indent);
        let second = builder.text(second);
        let multiline = builder.concat(vec![first, indent, second]);
        builder.choice(group, single_line, multiline, penalized)
    }

    #[test]
    fn author_layout_wins_while_everything_fits() {
        // Input was single-line and it fits: stays single-line.
        let mut builder = DocBuilder::new();
        let root = one_group_document(&mut builder, 1, Variant::Multiline, 50, (10, 4, 10));
        assert_eq!(builder.resolve(root)[&GroupId(1)], Variant::SingleLine);

        // Input was multiline: stays multiline even though collapsing would
        // save two lines — deviation ranks above height.
        let mut builder = DocBuilder::new();
        let root = one_group_document(&mut builder, 1, Variant::SingleLine, 50, (10, 4, 10));
        assert_eq!(builder.resolve(root)[&GroupId(1)], Variant::Multiline);
    }

    #[test]
    fn overflow_overrides_the_author_layout() {
        // Input was single-line but no longer fits: the search breaks it.
        let mut builder = DocBuilder::new();
        let root =
            one_group_document(&mut builder, 1, Variant::Multiline, PAGE_WIDTH + 20, (60, 4, 60));
        assert_eq!(builder.resolve(root)[&GroupId(1)], Variant::Multiline);
    }

    #[test]
    fn hopeless_overflow_keeps_the_author_layout() {
        // Both variants blow past the computation width; the tainted
        // fallback must keep the input-matching side, in both directions.
        for (penalized, expected) in
            [(Variant::Multiline, Variant::SingleLine), (Variant::SingleLine, Variant::Multiline)]
        {
            let mut builder = DocBuilder::new();
            let root = one_group_document(&mut builder, 1, penalized, 200, (150, 0, 150));
            assert_eq!(builder.resolve(root)[&GroupId(1)], expected);
        }
    }

    #[test]
    fn a_tainted_suffix_for_one_prefix_does_not_taint_the_others() {
        // The flat prefix ends at column 100, so the 30-wide suffix taints
        // there (130 > 125) — but the broken prefix ends at column 12 and
        // keeps a real frontier. A concat that tainted as a whole would fall
        // back to greedy and wrongly answer SingleLine.
        let mut builder = DocBuilder::new();
        let group = one_group_document(&mut builder, 1, Variant::Multiline, 100, (10, 2, 10));
        let suffix = builder.text(30);
        let root = builder.concat(vec![group, suffix]);
        assert_eq!(builder.resolve(root)[&GroupId(1)], Variant::Multiline);
    }

    #[test]
    fn greedy_fallback_keeps_the_author_layout_for_nested_groups() {
        // The outer group is hopeless in both variants, so the greedy
        // fallback walks its multiline body — and must take the author's
        // variant at the nested choice it finds there, recording it.
        let mut builder = DocBuilder::new();
        let inner = one_group_document(&mut builder, 2, Variant::SingleLine, 10, (5, 4, 5));
        let outer_single = builder.text(200);
        let outer_first = builder.text(150);
        let outer_newline = builder.newline(0);
        let outer_multi = builder.concat(vec![outer_first, outer_newline, inner]);
        let root = builder.choice(1, outer_single, outer_multi, Variant::SingleLine);

        let decisions = builder.resolve(root);
        assert_eq!(decisions[&GroupId(1)], Variant::Multiline);
        assert_eq!(decisions[&GroupId(2)], Variant::Multiline);
    }

    #[test]
    fn a_line_ending_exactly_at_the_computation_width_is_not_tainted() {
        // Taint starts strictly past the computation width. The flat variant
        // ends exactly at W — overflowing the page width but still inside
        // the search — while the broken variant taints outright, so the
        // untainted flat side must win. Tainting at exactly W instead would
        // degrade to the fallback's answer, the input layout: Multiline.
        let mut builder = DocBuilder::new();
        let root = one_group_document(
            &mut builder,
            1,
            Variant::SingleLine,
            COMPUTATION_WIDTH,
            (COMPUTATION_WIDTH + 25, 0, 10),
        );
        assert_eq!(builder.resolve(root)[&GroupId(1)], Variant::SingleLine);
    }

    #[test]
    fn a_suffix_can_make_the_search_pay_for_a_shorter_line() {
        // The group alone fits single-line (and that is cheapest there), but
        // the suffix pushes the flat layout past the page width. Only a
        // frontier that kept the pricier short-last-line candidate finds the
        // multiline answer.
        let mut builder = DocBuilder::new();
        let group = one_group_document(&mut builder, 1, Variant::Multiline, 80, (10, 2, 10));
        let suffix = builder.text(30);
        let root = builder.concat(vec![group, suffix]);
        assert_eq!(builder.resolve(root)[&GroupId(1)], Variant::Multiline);
    }

    #[test]
    fn a_group_inside_a_chosen_single_line_body_stays_undecided() {
        // The inner choice exists only inside the outer multiline body; when
        // the outer group flattens, the inner group is implicitly flattened
        // and must be absent from the decision map (the emitter's lookup
        // rule fills it in).
        let mut builder = DocBuilder::new();
        let inner = one_group_document(&mut builder, 2, Variant::Multiline, 10, (5, 4, 5));
        let outer_single = builder.text(20);
        let outer_first = builder.text(5);
        let outer_newline = builder.newline(4);
        let outer_multi = builder.concat(vec![outer_first, outer_newline, inner]);
        let root = builder.choice(1, outer_single, outer_multi, Variant::Multiline);

        let decisions = builder.resolve(root);
        assert_eq!(decisions[&GroupId(1)], Variant::SingleLine);
        assert!(!decisions.contains_key(&GroupId(2)));
    }

    #[test]
    fn tagging_a_shared_body_does_not_leak_into_other_choices() {
        // Two choices share one single-line body doc, resolved at the same
        // column — the second resolution is a memo hit. The recorded
        // decisions must still be per-choice (a leaked tag would make
        // `flatten_to_map` see a group decided twice).
        fn multiline_body(builder: &mut DocBuilder) -> DocId {
            let first = builder.text(1);
            let newline = builder.newline(0);
            let second = builder.text(1);
            builder.concat(vec![first, newline, second])
        }

        let mut builder = DocBuilder::new();
        let shared_body = builder.text(30);
        let first_multi = multiline_body(&mut builder);
        let second_multi = multiline_body(&mut builder);
        let first = builder.choice(1, shared_body, first_multi, Variant::Multiline);
        let second = builder.choice(2, shared_body, second_multi, Variant::Multiline);
        let between = builder.newline(0);
        let root = builder.concat(vec![first, between, second]);

        let decisions = builder.resolve(root);
        assert_eq!(decisions[&GroupId(1)], Variant::SingleLine);
        assert_eq!(decisions[&GroupId(2)], Variant::SingleLine);
    }
}
