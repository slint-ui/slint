// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Builds the choice document the width search runs on out of the engine's
//! annotated token slots, in two halves.
//!
//! The forest ([`GroupForest`]) turns the measured softlines into a nesting
//! tree of groups: every distinct measure span is one group, and the spans
//! nest or are disjoint, so they form a forest by containment. Each token and
//! gap belongs to the innermost group that contains it — the group whose
//! chosen variant resolves that gap.
//!
//! The builder ([`DocumentBuilder`]) walks that forest and emits a [`Doc`] per
//! token, gap and group: each group becomes a choice between its flat and
//! multiline bodies, with every gap resolved through the engine's own gap
//! resolver.

// Not wired into the pipeline yet; exercised by the width tests.
#![allow(dead_code)]

use super::doc::{Doc, DocId, DocumentArena, line_width};
use super::{GroupId, Variant};
use crate::fmt::atoms::{
    Annotations, Atom, AtomInstance, Condition, INDENT, Instruction, Marker, Whitespace,
};
use crate::fmt::engine::{
    DocumentEdges, SoftlineMode, TokenSlot, condition_active, containing_leaf, delete_ranges,
    literals, net_indentation, normalize_leaf_ranges, resolve_gap,
};
use crate::fmt::render::shift_continuation_lines;
use i_slint_compiler::parser::{SyntaxKind, TextRange, TextSize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// One group: a measure span, the run of token slots it covers, and its place
/// in the forest.
pub struct Group {
    pub span: TextRange,
    /// The slot range `[first_slot, last_slot]` the span covers — its first
    /// and last significant tokens.
    pub first_slot: usize,
    pub last_slot: usize,
    /// The end of the group's *extent*: `last_slot`, widened over a trailing
    /// separator the group conditionally deletes. The separator sits textually
    /// after the measure span, but it must be laid out — and its width
    /// counted — inside the group whose variant decides it.
    pub extent_last_slot: usize,
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
    group_of_span: HashMap<TextRange, GroupId>,
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
                    extent_last_slot: last_slot,
                    parent: None,
                    children: Vec::new(),
                    input_multiline: source[span].contains('\n'),
                }
            })
            .collect();

        let roots = link_forest(&mut groups);
        let group_of_span = groups
            .iter()
            .enumerate()
            .map(|(index, group)| (group.span, GroupId(index as u32)))
            .collect();
        let mut forest = GroupForest { groups, roots, group_of_span };
        forest.widen_extents(slots, annotations);
        forest
    }

    /// Widen each group's extent over the trailing separator it conditionally
    /// deletes (see [`Group::extent_last_slot`]). The separator token directly
    /// follows the group and precedes the enclosing delimiter, so widening
    /// never reaches a sibling group or escapes the parent.
    fn widen_extents(&mut self, slots: &[TokenSlot], annotations: &Annotations) {
        for (range, marker) in &annotations.markers {
            let Marker::Delete(Some(condition)) = marker else { continue };
            // A condition with no group is reported by the grouped-conditions
            // debug assertion instead.
            let Some(group) = self.by_span(condition.span) else { continue };
            let Some(slot) = slots.iter().position(|slot| slot.token.text_range() == *range) else {
                continue;
            };
            if slot <= self.get(group).extent_last_slot {
                continue; // Already inside the group.
            }
            debug_assert_eq!(
                slot,
                self.get(group).extent_last_slot + 1,
                "a conditionally deleted token must directly follow its group"
            );
            let parent_last_slot = self.get(group).parent.map(|parent| self.get(parent).last_slot);
            debug_assert!(
                parent_last_slot.is_none_or(|last_slot| slot < last_slot),
                "a widened extent must stay inside the parent group"
            );
            debug_assert!(
                self.groups.iter().all(|other| other.first_slot != slot),
                "a widened extent must not swallow another group's first slot"
            );
            self.groups[group.0 as usize].extent_last_slot = slot;
        }
    }

    pub fn get(&self, group: GroupId) -> &Group {
        &self.groups[group.0 as usize]
    }

    /// The group whose measure span is exactly `span`, if any.
    pub fn by_span(&self, span: TextRange) -> Option<GroupId> {
        self.group_of_span.get(&span).copied()
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

/// The choice document for one format run, plus what the emitter needs to
/// replay the search's decisions: the arena, its root, and the innermost
/// group controlling each gap.
pub struct BuiltDocument {
    pub arena: DocumentArena,
    pub root: DocId,
    /// Indexed by slot: the group whose chosen variant resolves the gap
    /// before that slot, or `None` for a gap no group controls.
    pub gap_controller: Vec<Option<GroupId>>,
    /// The group each measure span identifies, so the emitter can resolve a
    /// conditional literal or delete by its own group's decision. A widened
    /// atom sits in a gap another group controls, so the gap's mode is not
    /// enough there.
    pub group_of_span: HashMap<TextRange, GroupId>,
    /// Groups with no single-line body (a newline forbids flattening), so
    /// they never became a choice — the emitter resolves them multiline.
    pub flatten_forbidden: HashSet<GroupId>,
}

/// Build the choice document from the engine's annotated token slots.
pub fn build_document(
    slots: &[TokenSlot],
    annotations: &Annotations,
    source: &str,
) -> BuiltDocument {
    let forest = GroupForest::build(slots, annotations, source);
    #[cfg(debug_assertions)]
    debug_assert_conditions_grouped(annotations, &forest);
    let mut builder = DocumentBuilder::new(slots, annotations, source, &forest);
    let root = builder.build_root();
    // Every group is built once through the all-multiline expansion, so the
    // flat-body memo has an entry per group; the `None`s are flatten-forbidden.
    let flatten_forbidden = builder
        .flat_body
        .iter()
        .filter(|(_, body)| body.is_none())
        .map(|(&group, _)| group)
        .collect();
    let group_of_span = forest.group_of_span.clone();
    BuiltDocument {
        arena: builder.arena,
        root,
        gap_controller: builder.gap_controller,
        group_of_span,
        flatten_forbidden,
    }
}

/// Every conditional literal or delete must share its span — hence its
/// choice — with a group (a measured softline). Verify that up front so a
/// condition targeting a span no group owns is a loud bug in debug rather
/// than silently following the input layout.
#[cfg(debug_assertions)]
fn debug_assert_conditions_grouped(annotations: &Annotations, forest: &GroupForest) {
    let group_spans: HashSet<TextRange> =
        (0..forest.len()).map(|index| forest.get(GroupId(index as u32)).span).collect();
    let boundary = &annotations.boundary;
    let literal_conditions =
        boundary.before.values().chain(boundary.after.values()).flatten().filter_map(|instance| {
            match &instance.atom {
                Atom::Literal { condition, .. } => *condition,
                _ => None,
            }
        });
    let delete_conditions = annotations.markers.iter().filter_map(|(_, marker)| match marker {
        Marker::Delete(condition) => *condition,
        Marker::Leaf => None,
    });
    for condition in literal_conditions.chain(delete_conditions) {
        debug_assert!(
            group_spans.contains(&condition.span),
            "a condition's span {:?} is shared with no group; pair it with a softline",
            condition.span
        );
    }
}

/// Walks the group forest and turns each token, gap and group into a [`Doc`].
struct DocumentBuilder<'a> {
    slots: &'a [TokenSlot],
    annotations: &'a Annotations,
    source: &'a str,
    forest: &'a GroupForest,
    arena: DocumentArena,
    /// Which leaf range (if any) each slot's token sits inside.
    leaf_of_slot: Vec<Option<usize>>,
    /// The indentation level entering the gap before each slot. Indent atoms
    /// are unconditional, so this is fixed independent of any variant.
    indent_before: Vec<i32>,
    gap_controller: Vec<Option<GroupId>>,
    /// Token ranges a rule marked for deletion, with their condition — so a
    /// token deleted in the variant being built contributes no width.
    delete_ranges: Vec<(TextRange, Option<Condition>)>,
    /// Per group, the widths of its edge literals (see
    /// [`DocumentBuilder::emit_edge_literals`]), in attachment order.
    edge_literals: HashMap<GroupId, Vec<(u32, Condition)>>,
    /// Each group's built docs, so a group referenced by both its own choice
    /// and an ancestor's flat body is built once.
    group_doc: HashMap<GroupId, DocId>,
    flat_body: HashMap<GroupId, Option<DocId>>,
}

/// One piece of a gap's or token's output, gathered while the read-only
/// inputs are borrowed and allocated into the arena afterwards — so the
/// arena's `&mut` never collides with the input borrows.
enum Piece {
    /// A single-line run of the given character width.
    Width(u32),
    Newline(u32),
    /// Verbatim text that may span lines (a comment, a leaf interior).
    Verbatim(String),
}

impl<'a> DocumentBuilder<'a> {
    fn new(
        slots: &'a [TokenSlot],
        annotations: &'a Annotations,
        source: &'a str,
        forest: &'a GroupForest,
    ) -> DocumentBuilder<'a> {
        let leaf_ranges = normalize_leaf_ranges(&annotations.markers);
        let leaf_of_slot = slots
            .iter()
            .map(|slot| containing_leaf(slot.token.text_range().start(), &leaf_ranges))
            .collect();
        let gap_controller = gap_controllers(slots.len(), forest);
        DocumentBuilder {
            leaf_of_slot,
            indent_before: indent_before_each_gap(slots, annotations),
            edge_literals: edge_literals(slots, annotations, forest, &gap_controller),
            gap_controller,
            delete_ranges: delete_ranges(&annotations.markers),
            slots,
            annotations,
            source,
            forest,
            arena: DocumentArena::default(),
            group_doc: HashMap::new(),
            flat_body: HashMap::new(),
        }
    }

    /// The whole document, as a concatenation with fixed top-level decisions
    /// (no group controls a document-level gap).
    fn build_root(&mut self) -> DocId {
        let children = self.forest.roots().to_vec();
        let last_slot = self.slots.len() - 1;
        self.build_region(0, last_slot, &children, None, SoftlineMode::FromInput, false)
            .expect("the root body never forbids a layout")
    }

    /// A group's doc: a choice between its flat and multiline bodies, or —
    /// when flattening is forbidden — just the multiline body.
    fn build_group(&mut self, group: GroupId) -> DocId {
        if let Some(&doc) = self.group_doc.get(&group) {
            return doc;
        }
        let node = self.forest.get(group);
        let (first, last, children, penalized) = (
            node.first_slot,
            node.extent_last_slot,
            node.children.clone(),
            node.penalized_variant(),
        );
        let multiline = self
            .build_region(
                first,
                last,
                &children,
                Some(group),
                SoftlineMode::Decided(Variant::Multiline),
                false,
            )
            .expect("a multiline body never forbids a layout");
        let doc = match self.build_flat_body(group) {
            Some(single_line) => {
                self.arena.alloc(Doc::Choice { group, single_line, multiline, penalized })
            }
            None => multiline,
        };
        self.group_doc.insert(group, doc);
        doc
    }

    /// A group's single-line body, with inner groups forced single-line too.
    /// `None` when the body contains any newline, of whatever origin — then
    /// flattening the group is forbidden.
    fn build_flat_body(&mut self, group: GroupId) -> Option<DocId> {
        if let Some(&doc) = self.flat_body.get(&group) {
            return doc;
        }
        let node = self.forest.get(group);
        let (first, last, children) =
            (node.first_slot, node.extent_last_slot, node.children.clone());
        let doc = self.build_region(
            first,
            last,
            &children,
            Some(group),
            SoftlineMode::Decided(Variant::SingleLine),
            true,
        );
        self.flat_body.insert(group, doc);
        doc
    }

    /// Concatenate the slots `[first, last]`: emit each gap this region
    /// controls, each token it owns, each child group in source order, and
    /// the region group's edge literals at the end. `flatten` forces child
    /// groups single-line and returns `None` if any newline slips in.
    fn build_region(
        &mut self,
        first: usize,
        last: usize,
        children: &[GroupId],
        region_group: Option<GroupId>,
        gap_mode: SoftlineMode,
        flatten: bool,
    ) -> Option<DocId> {
        let mut parts = Vec::new();
        let mut next_child = 0;
        let mut slot = first;
        while slot <= last {
            let child = children.get(next_child).copied();
            let child_first = child.map(|child| self.forest.get(child).first_slot);
            // A group boundary is never a deleted separator.
            let deleted = child_first != Some(slot) && self.is_deleted(slot, gap_mode);
            if deleted {
                self.debug_assert_deletable(slot, region_group);
            }
            if self.gap_controller[slot] == region_group {
                if deleted {
                    self.emit_collapsed_gap(slot, &mut parts);
                } else {
                    self.emit_gap(slot, gap_mode, &mut parts);
                }
            }
            if child_first == Some(slot) {
                let child = child.unwrap();
                let child_last = self.forest.get(child).extent_last_slot;
                let child_doc =
                    if flatten { self.build_flat_body(child)? } else { self.build_group(child) };
                parts.push(child_doc);
                slot = child_last + 1;
                next_child += 1;
            } else {
                // A deleted token emits nothing; the emitter drops it the same
                // way (its gap collapsed above).
                if !deleted {
                    self.emit_token(slot, &mut parts);
                }
                slot += 1;
            }
        }
        self.emit_edge_literals(region_group, gap_mode, &mut parts);
        if flatten && parts.iter().any(|&doc| matches!(self.arena.get(doc), Doc::Newline { .. })) {
            return None;
        }
        Some(self.arena.alloc(Doc::Concat(parts)))
    }

    /// Append the region group's edge literals — conditional literals whose
    /// gap lies just past the group's extent (a trailing separator added to a
    /// broken list). They belong to this group's choice, so they are laid out
    /// at the end of its bodies rather than in the parent's gap; the emitter
    /// produces the same order, literal before the gap's whitespace.
    fn emit_edge_literals(
        &mut self,
        region_group: Option<GroupId>,
        gap_mode: SoftlineMode,
        parts: &mut Vec<DocId>,
    ) {
        let Some(group) = region_group else { return };
        // Cloned so the arena's `&mut self` borrow stays free.
        let edge_literals = self.edge_literals.get(&group).cloned().unwrap_or_default();
        for (width, condition) in edge_literals {
            if condition_active(Some(condition), gap_mode, self.source) {
                parts.push(self.arena.alloc(Doc::Text { width }));
            }
        }
    }

    /// Append the docs for the gap before `slot`, resolved with `mode`. The
    /// pieces are gathered first (read-only borrows), then allocated — so the
    /// input borrows never collide with the arena's `&mut`.
    fn emit_gap(&mut self, slot: usize, mode: SoftlineMode, parts: &mut Vec<DocId>) {
        for piece in self.gap_pieces(slot, mode) {
            match piece {
                Piece::Width(width) => parts.push(self.arena.alloc(Doc::Text { width })),
                Piece::Newline(indent_width) => {
                    parts.push(self.arena.alloc(Doc::Newline { indent_width }))
                }
                Piece::Verbatim(text) => parts.extend(self.arena.verbatim(&text)),
            }
        }
    }

    /// Append the docs for `slot`'s significant token (its verbatim text).
    fn emit_token(&mut self, slot: usize, parts: &mut Vec<DocId>) {
        let text = self.slots[slot].token.text().to_string();
        parts.extend(self.arena.verbatim(&text));
    }

    /// The gap before a deleted token: the emitter collapses it to nothing, or
    /// keeps a comment verbatim.
    fn emit_collapsed_gap(&mut self, slot: usize, parts: &mut Vec<DocId>) {
        if self.slots[slot].has_comment() {
            let text = self.full_gap_text(slot);
            parts.extend(self.arena.verbatim(&text));
        }
    }

    /// The invariants a deleted token must satisfy for the builder to match the
    /// emitter, since the builder does not replay the emitter's carrying of a
    /// dropped token's atoms across the collapsed gap. The trailing-separator
    /// idiom satisfies all of these; a rule that breaks one is a bug.
    #[cfg(debug_assertions)]
    fn debug_assert_deletable(&self, slot: usize, region_group: Option<GroupId>) {
        // The deleted token's variant must be this region's, so builder and
        // emitter resolve it the same way.
        debug_assert_eq!(
            self.gap_controller[slot], region_group,
            "a deleted token must sit inside the extent of the group that gates it"
        );
        // A conditional delete must be gated by the region's own group; a
        // condition on some other group would need that group's undecided
        // variant here.
        let start = self.slots[slot].token.text_range().start();
        for (range, condition) in &self.delete_ranges {
            if !range.contains(start) {
                continue;
            }
            let condition_group =
                condition.and_then(|condition| self.forest.by_span(condition.span));
            debug_assert!(
                condition.is_none() || condition_group == region_group,
                "a conditional delete must be gated by the group whose region holds the token"
            );
        }
        // The emitter carries the previous surviving token's append atoms past
        // a dropped token and discards the dropped token's own; the builder
        // does neither, so both must be empty.
        let (before, _) = gap_atoms(self.slots, self.annotations, slot);
        let own_start = self.slots[slot].token.text_range().start();
        let own_appends = self.annotations.boundary.after.get(&own_start);
        debug_assert!(before.is_empty(), "append atoms before a deleted token would be lost");
        debug_assert!(
            own_appends.is_none_or(|atoms| atoms.is_empty()),
            "a deleted token's own append atoms would be miscounted"
        );
    }

    /// The pieces of the gap before `slot`, gathered while the inputs are
    /// borrowed. A gap inside a leaf range is kept verbatim; otherwise the
    /// engine's own gap resolution decides it, with `mode` substituted for
    /// the measured softlines.
    fn gap_pieces(&self, slot: usize, mode: SoftlineMode) -> Vec<Piece> {
        if slot > 0
            && self.leaf_of_slot[slot].is_some()
            && self.leaf_of_slot[slot] == self.leaf_of_slot[slot - 1]
        {
            return vec![Piece::Verbatim(self.full_gap_text(slot))];
        }

        let (append, prepend) = gap_atoms(self.slots, self.annotations, slot);

        let edges = DocumentEdges {
            start: slot == 0,
            end: self.slots[slot].token.kind() == SyntaxKind::Eof,
        };
        let resolution = resolve_gap(
            &self.slots[slot],
            slot,
            append,
            prepend,
            edges,
            self.source,
            self.indent_before[slot],
            mode,
        );

        // Append literals hug the left token, then the gap whitespace and
        // comments, then prepend literals hug this token — the order the
        // engine's own resolver emits them in. A conditional literal counts
        // only in the variant it names.
        let mut pieces = Vec::new();
        pieces.extend(self.active_literals(append, mode, slot));
        for instruction in &resolution.instructions {
            self.instruction_piece(instruction, slot, &mut pieces);
        }
        pieces.extend(self.active_literals(prepend, mode, slot));
        pieces
    }

    /// The width pieces of the literals in `atoms` that fire in `mode`, for
    /// the gap before `gap_slot`. Edge literals are excluded — they belong to
    /// their own group's body, past whose extent this gap lies.
    fn active_literals(
        &self,
        atoms: &[AtomInstance],
        mode: SoftlineMode,
        gap_slot: usize,
    ) -> Vec<Piece> {
        literals(atoms)
            .filter(|(_, condition)| !self.is_edge_literal(*condition, gap_slot))
            .filter(|(_, condition)| condition_active(*condition, mode, self.source))
            .map(|(text, _)| Piece::Width(line_width(text)))
            .collect()
    }

    /// Whether a literal's condition points at a group whose extent ends
    /// before `gap_slot` — then the literal is one of that group's edge
    /// literals, emitted at the end of its bodies instead of in this gap.
    fn is_edge_literal(&self, condition: Option<Condition>, gap_slot: usize) -> bool {
        let Some(group) = condition.and_then(|condition| self.forest.by_span(condition.span))
        else {
            return false;
        };
        gap_slot > self.forest.get(group).extent_last_slot
    }

    /// Whether the token at `slot` is deleted in the variant `mode` names. A
    /// delete inside a leaf is ignored, as the leaf keeps its interior
    /// verbatim.
    fn is_deleted(&self, slot: usize, mode: SoftlineMode) -> bool {
        if self.leaf_of_slot[slot].is_some() {
            return false;
        }
        let start = self.slots[slot].token.text_range().start();
        self.delete_ranges.iter().any(|(range, condition)| {
            range.contains(start) && condition_active(*condition, mode, self.source)
        })
    }

    fn instruction_piece(&self, instruction: &Instruction, slot: usize, pieces: &mut Vec<Piece>) {
        match instruction {
            Instruction::ReplaceGap { whitespace, .. }
            | Instruction::ReplaceSubGap { whitespace, .. } => {
                if let Some(piece) = whitespace_piece(*whitespace) {
                    pieces.push(piece);
                }
            }
            &Instruction::EmitComment { trivia_index, column_shift, .. } => {
                // The renderer re-indents continuation lines by `column_shift`,
                // so the search must measure the shifted text, not the input.
                let comment = self.slots[slot].gap_before[trivia_index].text();
                let shifted = shift_continuation_lines(comment, column_shift);
                pieces.push(Piece::Verbatim(shifted.unwrap_or_else(|| comment.to_string())));
            }
            // A gap resolves only into whitespace and comment instructions.
            _ => debug_assert!(false, "unexpected gap instruction {instruction:?}"),
        }
    }

    /// The full input trivia of the gap before `slot`, for a leaf interior.
    fn full_gap_text(&self, slot: usize) -> String {
        self.slots[slot].gap_before.iter().map(|trivia| trivia.text()).collect()
    }
}

/// A gap's whitespace as a doc piece: a space is one column, a newline
/// carries its baked indentation, and nothing produces no piece. A blank
/// line counts as the single newline it visually adds width-wise.
fn whitespace_piece(whitespace: Whitespace) -> Option<Piece> {
    match whitespace {
        Whitespace::None => None,
        Whitespace::Space => Some(Piece::Width(1)),
        Whitespace::Newline { indentation_level, .. } => {
            Some(Piece::Newline(indentation_level * INDENT.len() as u32))
        }
    }
}

/// The indentation level entering each gap, mirroring the engine resolver's
/// running counter: every gap's own indent atoms shift the level for the
/// gaps that follow.
fn indent_before_each_gap(slots: &[TokenSlot], annotations: &Annotations) -> Vec<i32> {
    let mut levels = Vec::with_capacity(slots.len());
    let mut level = 0;
    for slot in 0..slots.len() {
        levels.push(level);
        let (append, prepend) = gap_atoms(slots, annotations, slot);
        level += net_indentation(append) + net_indentation(prepend);
    }
    levels
}

/// The two atom lists that meet in the gap before `slot`: the previous token's
/// append atoms and this token's prepend atoms (empty where a token has none,
/// and no append atoms before the first slot).
fn gap_atoms<'a>(
    slots: &[TokenSlot],
    annotations: &'a Annotations,
    slot: usize,
) -> (&'a [AtomInstance], &'a [AtomInstance]) {
    let empty: &[AtomInstance] = &[];
    let append = if slot > 0 {
        let previous = slots[slot - 1].token.text_range().start();
        annotations.boundary.after.get(&previous).map_or(empty, Vec::as_slice)
    } else {
        empty
    };
    let prepend = annotations
        .boundary
        .before
        .get(&slots[slot].token.text_range().start())
        .map_or(empty, Vec::as_slice);
    (append, prepend)
}

/// The innermost group controlling each gap (indexed by slot). A group
/// controls the gaps strictly inside its extent; processing groups
/// outer-first lets an inner group overwrite its ancestors.
fn gap_controllers(slot_count: usize, forest: &GroupForest) -> Vec<Option<GroupId>> {
    let mut controllers = vec![None; slot_count];
    for index in 0..forest.len() {
        let group = GroupId(index as u32);
        let node = forest.get(group);
        for gap in (node.first_slot + 1)..=node.extent_last_slot {
            controllers[gap] = Some(group);
        }
    }
    controllers
}

/// Each group's edge literals: conditional literals attached past the group's
/// extent, i.e. a trailing separator a broken list gains. They are emitted at
/// the end of the group's bodies (see `emit_edge_literals`); an edge literal
/// anywhere but directly at the extent's end would come out reordered, so
/// that placement is asserted.
fn edge_literals(
    slots: &[TokenSlot],
    annotations: &Annotations,
    forest: &GroupForest,
    gap_controller: &[Option<GroupId>],
) -> HashMap<GroupId, Vec<(u32, Condition)>> {
    let mut edges: HashMap<GroupId, Vec<(u32, Condition)>> = HashMap::new();
    for (text, condition, group, anchor_slot) in
        grouped_literals(&annotations.boundary.after, slots, forest)
    {
        let extent_last_slot = forest.get(group).extent_last_slot;
        if anchor_slot < extent_last_slot {
            // In place: the literal's gap is controlled by its own group.
            debug_assert_eq!(gap_controller[anchor_slot + 1], Some(group));
            continue;
        }
        debug_assert_eq!(
            anchor_slot, extent_last_slot,
            "an edge literal must be attached to its group's last token"
        );
        edges.entry(group).or_default().push((line_width(text), condition));
    }
    // A conditional prepend literal outside its group's controlled gaps has no
    // defined place; no idiom produces one.
    #[cfg(debug_assertions)]
    for (_, _, group, anchor_slot) in grouped_literals(&annotations.boundary.before, slots, forest)
    {
        debug_assert_eq!(
            gap_controller[anchor_slot],
            Some(group),
            "a conditional prepend literal must sit in a gap its group controls"
        );
    }
    edges
}

/// The conditional literals in one boundary map (`before` or `after`), each
/// with the group its condition names and the slot of its anchor token.
fn grouped_literals<'a>(
    boundary: &'a BTreeMap<TextSize, Vec<AtomInstance>>,
    slots: &'a [TokenSlot],
    forest: &'a GroupForest,
) -> impl Iterator<Item = (&'a str, Condition, GroupId, usize)> + 'a {
    boundary.iter().flat_map(move |(&anchor, atoms)| {
        literals(atoms).filter_map(move |(text, condition)| {
            let condition = condition?;
            let group = forest.by_span(condition.span)?;
            let anchor_slot =
                slots.iter().position(|slot| slot.token.text_range().start() == anchor)?;
            Some((text, condition, group, anchor_slot))
        })
    })
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
    fn a_conditional_trailing_separator_widens_the_group_extent() {
        // `separated_by` conditionally deletes the trailing comma, which sits
        // after the item list's measure span; the list group's extent must
        // grow over it so the group lays it out and controls its gap.
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::Array, |array| {
            array.node(SyntaxKind::Expression).separated_by(SyntaxKind::Comma);
        });
        let source = "component A { x: [1, 2,]; }";
        let (linearization, forest) = build_forest(source, &rules);
        assert_eq!(forest.len(), 1);
        let group = forest.get(GroupId(0));
        // The raw span ends at the last item; the extent covers the comma.
        assert_eq!(linearization.slots[group.last_slot].token.text(), "2");
        assert_eq!(linearization.slots[group.extent_last_slot].token.text(), ",");
        assert_eq!(group.extent_last_slot, group.last_slot + 1);

        let controllers = gap_controllers(linearization.slots.len(), &forest);
        assert_eq!(controllers[group.extent_last_slot], Some(GroupId(0)));
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

    /// An element-body rule that also indents, so multiline bodies carry a
    /// non-zero baked indent for the tests to observe.
    fn indented_body_rules() -> FormatRules {
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::Element, |element| {
            element
                .token(SyntaxKind::LBrace)
                .append(Atom::IndentStart)
                .append(element.spaced_softline());
            element
                .token(SyntaxKind::RBrace)
                .prepend(Atom::IndentEnd)
                .prepend(element.spaced_softline());
        });
        rules
    }

    fn build(source: &str, rules: &FormatRules) -> BuiltDocument {
        let document = parse(source);
        let linearization = linearize(&document);
        let sink = AtomSink::default();
        annotate(&document, &linearization.slots, rules, &sink, source);
        build_document(&linearization.slots, &sink.finish(), source)
    }

    /// A compact structural rendering of a doc tree: `T<w>` a text run of
    /// width w, `NL<i>` a newline indented i columns, `(...)` a concat, and
    /// `C<g>[flat|multiline]` a group's choice.
    fn dump(arena: &DocumentArena, doc: DocId) -> String {
        match arena.get(doc) {
            Doc::Text { width } => format!("T{width}"),
            Doc::Newline { indent_width } => format!("NL{indent_width}"),
            Doc::Concat(parts) => {
                let inner: Vec<String> = parts.iter().map(|&part| dump(arena, part)).collect();
                format!("({})", inner.join(" "))
            }
            Doc::Choice { group, single_line, multiline, .. } => {
                format!("C{}[{}|{}]", group.0, dump(arena, *single_line), dump(arena, *multiline))
            }
        }
    }

    #[test]
    fn a_fitting_single_line_body_becomes_a_choice() {
        let built = build("component A { Text { } }", &indented_body_rules());
        let rendered = dump(&built.arena, built.root);
        // Both element bodies are choices (outer wraps inner).
        assert!(rendered.contains("C0["), "outer body is a choice: {rendered}");
        assert!(rendered.contains("C1["), "inner body is a choice: {rendered}");
    }

    #[test]
    fn the_flat_body_of_a_choice_has_no_newline() {
        let built = build("component A { Text { } }", &indented_body_rules());
        let Doc::Choice { single_line, .. } = built.arena.get(built.root_choice()) else {
            panic!("root_choice must return a choice");
        };
        assert!(!dump(&built.arena, *single_line).contains("NL"));
    }

    #[test]
    fn multiline_newlines_carry_the_baked_indentation() {
        // Two nested bodies: the outer component body breaks at level 1 (four
        // columns), the inner Text body at level 2 (eight). Both baked indents
        // must appear.
        let built = build("component A { Text {\n    x: 1;\n} }", &indented_body_rules());
        let rendered = dump(&built.arena, built.root);
        assert!(rendered.contains("NL4"), "outer indent: {rendered}");
        assert!(rendered.contains("NL8"), "inner indent: {rendered}");
    }

    #[test]
    fn a_reindented_comment_is_measured_after_shifting() {
        // The renderer dedents this comment's continuation line to follow the
        // body's reduced indentation, so the search must measure the shifted
        // width (`    second */`, 13), not the raw input width (21).
        let source = "component A {\n    Text {\n        /* first\n            second */\n    }\n}";
        let built = build(source, &element_body_rules());
        let rendered = dump(&built.arena, built.root);
        assert!(rendered.contains("T13"), "shifted continuation width: {rendered}");
        assert!(!rendered.contains("T21"), "raw continuation width must not appear: {rendered}");
    }

    #[test]
    fn a_comment_forced_newline_forbids_flattening() {
        // A line comment inside the body forces a newline there, so the body
        // has no single-line variant: no choice, just the multiline body.
        let built = build("component A { Text { } // note\n}", &indented_body_rules());
        let rendered = dump(&built.arena, built.root);
        // The outer body cannot flatten (the comment newline is inside it),
        // so its group id never appears as a choice.
        assert!(!rendered.contains("C0["), "outer body must not be a choice: {rendered}");
    }

    #[test]
    fn gap_controllers_point_at_the_innermost_group() {
        let source = "component A { Text { } }";
        let built = build(source, &indented_body_rules());
        // Some gap is controlled by the inner group (id 1), and the deepest
        // controller is never an ancestor of a more-nested one.
        assert!(built.gap_controller.iter().any(|controller| *controller == Some(GroupId(1))));
    }
}

#[cfg(test)]
impl BuiltDocument {
    /// The outermost choice reachable from the root (test helper).
    fn root_choice(&self) -> DocId {
        fn find(arena: &DocumentArena, doc: DocId) -> Option<DocId> {
            match arena.get(doc) {
                Doc::Choice { .. } => Some(doc),
                Doc::Concat(parts) => parts.iter().find_map(|&part| find(arena, part)),
                _ => None,
            }
        }
        find(&self.arena, self.root).expect("a choice exists")
    }
}
