// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Builds the choice document the width search runs on out of the engine's
//! annotated token slots.
//!
//! The first half — this file's forest — turns the measured softlines into a
//! nesting tree of groups: every distinct measure span is one group, and the
//! spans nest or are disjoint, so they form a forest by containment. Each
//! token and gap belongs to the innermost group that contains it, which is
//! the group whose chosen variant resolves that gap.

// Not wired into the pipeline yet; exercised by the width tests.
#![allow(dead_code)]

use super::{GroupId, Variant};
use crate::fmt::atoms::{Annotations, Atom};
use crate::fmt::engine::TokenSlot;
use i_slint_compiler::parser::TextRange;

/// One group: a measure span, the run of token slots it covers, and its place
/// in the forest.
pub struct Group {
    pub span: TextRange,
    /// The slot range `[first_slot, last_slot]` the span covers — its first
    /// and last significant tokens.
    pub first_slot: usize,
    pub last_slot: usize,
    pub parent: Option<GroupId>,
    /// Direct children, in source order.
    pub children: Vec<GroupId>,
    /// Whether the span was multiline in the input — the variant the author
    /// chose, which the search keeps unless width forces otherwise.
    pub input_multiline: bool,
}

impl Group {
    /// Which variant deviates from the author's input layout, and so pays the
    /// deviation penalty in the search.
    pub fn penalized_variant(&self) -> Variant {
        if self.input_multiline { Variant::SingleLine } else { Variant::Multiline }
    }
}

/// The groups of one document, arranged as a containment forest.
pub struct GroupForest {
    groups: Vec<Group>,
    roots: Vec<GroupId>,
}

impl GroupForest {
    /// Collect the measured-softline spans into a forest. `slots` and
    /// `source` come from the engine's linearization and annotation phases.
    pub fn build(slots: &[TokenSlot], annotations: &Annotations, source: &str) -> GroupForest {
        let mut spans = measure_spans(annotations);
        // Outer-first, so a parent is always seen before its children.
        spans.sort_by_key(|span| (span.start(), std::cmp::Reverse(span.end())));
        spans.dedup();

        let mut groups: Vec<Group> = spans
            .into_iter()
            .map(|span| {
                let (first_slot, last_slot) = slot_range(slots, span);
                Group {
                    span,
                    first_slot,
                    last_slot,
                    parent: None,
                    children: Vec::new(),
                    input_multiline: source[span].contains('\n'),
                }
            })
            .collect();

        let roots = link_forest(&mut groups);
        GroupForest { groups, roots }
    }

    pub fn get(&self, group: GroupId) -> &Group {
        &self.groups[group.0 as usize]
    }

    pub fn roots(&self) -> &[GroupId] {
        &self.roots
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// Every distinct span carried by a measured softline (`SpacedSoftline` /
/// `EmptySoftline`). `InputSoftline` carries no span — it resolves from the
/// input and stays outside the search.
fn measure_spans(annotations: &Annotations) -> Vec<TextRange> {
    let boundary = &annotations.boundary;
    boundary
        .before
        .values()
        .chain(boundary.after.values())
        .flatten()
        .filter_map(|instance| match instance.atom {
            // An empty span (a token-less context node) resolves single-line
            // and controls nothing, so it needs no group — and would have no
            // slot range.
            Atom::SpacedSoftline(span) | Atom::EmptySoftline(span) if !span.is_empty() => {
                Some(span)
            }
            _ => None,
        })
        .collect()
}

/// The slot range a significant span covers: its first slot starts at the
/// span's start, its last slot ends at the span's end.
fn slot_range(slots: &[TokenSlot], span: TextRange) -> (usize, usize) {
    let first_slot = slots
        .iter()
        .position(|slot| slot.token.text_range().start() >= span.start())
        .expect("a measure span starts at a significant token");
    // `start < span.end()` excludes the zero-width Eof slot, which sits at the
    // document end: a group ending at the last token (no trailing newline)
    // would otherwise swallow Eof and the file-final gap.
    let last_slot = slots
        .iter()
        .rposition(|slot| {
            let range = slot.token.text_range();
            range.end() <= span.end() && range.start() < span.end()
        })
        .expect("a measure span ends at a significant token");
    debug_assert!(first_slot <= last_slot, "empty measure span {span:?}");
    (first_slot, last_slot)
}

/// Link the sorted groups into a forest by containment, returning the roots.
/// A stack holds the currently open ancestors; the spans nest or are disjoint,
/// so the innermost still-open group is the parent.
fn link_forest(groups: &mut [Group]) -> Vec<GroupId> {
    let mut roots = Vec::new();
    let mut open_ancestors: Vec<usize> = Vec::new();
    for index in 0..groups.len() {
        while let Some(&innermost) = open_ancestors.last() {
            if groups[innermost].last_slot >= groups[index].last_slot {
                break;
            }
            // A partial overlap cannot be represented in a choice tree.
            debug_assert!(
                groups[innermost].last_slot < groups[index].first_slot,
                "measure spans partially overlap: {:?} and {:?}",
                groups[innermost].span,
                groups[index].span
            );
            open_ancestors.pop();
        }
        match open_ancestors.last() {
            Some(&parent) => {
                groups[index].parent = Some(GroupId(parent as u32));
                groups[parent].children.push(GroupId(index as u32));
            }
            None => roots.push(GroupId(index as u32)),
        }
        open_ancestors.push(index);
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmt::atoms::AtomSink;
    use crate::fmt::engine::{FormatRules, Linearization, annotate, linearize};
    use i_slint_compiler::diagnostics::BuildDiagnostics;
    use i_slint_compiler::parser::{SyntaxKind, SyntaxNode};

    fn parse(source: &str) -> SyntaxNode {
        i_slint_compiler::parser::parse(
            String::from(source),
            None,
            &mut BuildDiagnostics::default(),
        )
    }

    /// Build the forest for `source` under `rules`.
    fn build_forest(source: &str, rules: &FormatRules) -> (Linearization, GroupForest) {
        let document = parse(source);
        let linearization = linearize(&document);
        let sink = AtomSink::default();
        annotate(&document, &linearization.slots, rules, &sink, source);
        let annotations = sink.finish();
        let forest = GroupForest::build(&linearization.slots, &annotations, source);
        (linearization, forest)
    }

    /// The token texts of a group's slot range.
    fn slot_texts(linearization: &Linearization, group: &Group) -> Vec<String> {
        (group.first_slot..=group.last_slot)
            .map(|slot| linearization.slots[slot].token.text().to_string())
            .collect()
    }

    /// A ruleset that puts a spaced softline around a braced element body.
    fn element_body_rules() -> FormatRules {
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::Element, |element| {
            element.token(SyntaxKind::LBrace).append(element.spaced_softline());
            element.token(SyntaxKind::RBrace).prepend(element.spaced_softline());
        });
        rules
    }

    #[test]
    fn each_distinct_measure_span_is_one_group() {
        // Two element bodies, so two spans; the softlines within one body
        // share a span and collapse to a single group.
        let source = "component A {\n    Text { }\n    Image { }\n}";
        let (_, forest) = build_forest(source, &element_body_rules());
        // The component body and each of the two child element bodies.
        assert_eq!(forest.len(), 3);
    }

    #[test]
    fn no_measured_softlines_yields_an_empty_forest() {
        let mut rules = FormatRules::default();
        // Only a non-measured atom, so no group is created.
        rules.token(SyntaxKind::Colon, |colon| {
            colon.append(Atom::Space);
        });
        let (_, forest) = build_forest("component A { x: 1; }", &rules);
        assert!(forest.is_empty());
        assert_eq!(forest.len(), 0);
        assert!(forest.roots().is_empty());
    }

    #[test]
    fn a_group_ends_at_its_closing_delimiter_not_at_eof() {
        // The source has no trailing newline, so the closing brace abuts the
        // zero-width Eof slot; the group must still end at the brace.
        let source = "component A { Text { } }";
        let (linearization, forest) = build_forest(source, &element_body_rules());
        let root = forest.get(forest.roots()[0]);
        assert_eq!(slot_texts(&linearization, root).last().unwrap(), "}");
    }

    #[test]
    fn nested_bodies_form_a_parent_child_forest() {
        let source = "component A { Text { } }";
        let (linearization, forest) = build_forest(source, &element_body_rules());
        assert_eq!(forest.roots().len(), 1);

        let root = forest.get(forest.roots()[0]);
        // The outer body spans the whole component brace pair.
        assert_eq!(slot_texts(&linearization, root).first().unwrap(), "{");
        assert_eq!(root.children.len(), 1);

        let child = forest.get(root.children[0]);
        assert_eq!(child.parent, Some(forest.roots()[0]));
        // The inner Text body sits strictly inside the outer one.
        assert!(child.first_slot > root.first_slot);
        assert!(child.last_slot < root.last_slot);
    }

    #[test]
    fn sibling_bodies_are_disjoint_roots_of_their_parent() {
        let source = "component A { Text { } Image { } }";
        let (_, forest) = build_forest(source, &element_body_rules());
        let root = forest.get(forest.roots()[0]);
        assert_eq!(root.children.len(), 2);
        let first = forest.get(root.children[0]);
        let second = forest.get(root.children[1]);
        // Disjoint and in source order.
        assert!(first.last_slot < second.first_slot);
    }

    #[test]
    fn input_multilineness_sets_the_penalized_variant() {
        // A body written on one line prefers to stay single-line...
        let (_, forest) = build_forest("component A { Text { } }", &element_body_rules());
        let inner = forest.get(forest.get(forest.roots()[0]).children[0]);
        assert_eq!(inner.penalized_variant(), Variant::Multiline);

        // ...one spread across lines prefers to stay multiline.
        let (_, forest) =
            build_forest("component A {\n    Text {\n    }\n}", &element_body_rules());
        let inner = forest.get(forest.get(forest.roots()[0]).children[0]);
        assert_eq!(inner.penalized_variant(), Variant::SingleLine);
    }
}
