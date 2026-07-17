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
use crate::fmt::atoms::{Annotations, Atom, AtomInstance, INDENT, Instruction, Whitespace};
use crate::fmt::engine::{
    DocumentEdges, SoftlineMode, TokenSlot, containing_leaf, literal_texts, net_indentation,
    normalize_leaf_ranges, resolve_gap,
};
use crate::fmt::render::shift_continuation_lines;
use i_slint_compiler::parser::{SyntaxKind, TextRange};
use std::collections::HashMap;

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

/// The choice document for one format run, plus what the emitter needs to
/// replay the search's decisions: the arena, its root, and the innermost
/// group controlling each gap.
pub struct BuiltDocument {
    pub arena: DocumentArena,
    pub root: DocId,
    /// Indexed by slot: the group whose chosen variant resolves the gap
    /// before that slot, or `None` for a gap no group controls.
    pub gap_controller: Vec<Option<GroupId>>,
}

/// Build the choice document from the engine's annotated token slots.
pub fn build_document(
    slots: &[TokenSlot],
    annotations: &Annotations,
    source: &str,
) -> BuiltDocument {
    let forest = GroupForest::build(slots, annotations, source);
    let mut builder = DocumentBuilder::new(slots, annotations, source, &forest);
    let root = builder.build_root();
    BuiltDocument { arena: builder.arena, root, gap_controller: builder.gap_controller }
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
        DocumentBuilder {
            leaf_of_slot,
            indent_before: indent_before_each_gap(slots, annotations),
            gap_controller: gap_controllers(slots.len(), forest),
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
        let (first, last, children, penalized) =
            (node.first_slot, node.last_slot, node.children.clone(), node.penalized_variant());
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
        let (first, last, children) = (node.first_slot, node.last_slot, node.children.clone());
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
    /// controls, each token it owns, and each child group in source order.
    /// `flatten` forces child groups single-line and returns `None` if any
    /// newline slips in.
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
            if self.gap_controller[slot] == region_group {
                self.emit_gap(slot, gap_mode, &mut parts);
            }
            let child = children.get(next_child).copied();
            let child_first = child.map(|child| self.forest.get(child).first_slot);
            if child_first == Some(slot) {
                let child = child.unwrap();
                let child_last = self.forest.get(child).last_slot;
                let child_doc =
                    if flatten { self.build_flat_body(child)? } else { self.build_group(child) };
                parts.push(child_doc);
                slot = child_last + 1;
                next_child += 1;
            } else {
                self.emit_token(slot, &mut parts);
                slot += 1;
            }
        }
        if flatten && parts.iter().any(|&doc| matches!(self.arena.get(doc), Doc::Newline { .. })) {
            return None;
        }
        Some(self.arena.alloc(Doc::Concat(parts)))
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
        // engine's own resolver emits them in.
        let mut pieces = Vec::new();
        pieces.extend(literal_texts(append).map(|text| Piece::Width(line_width(text))));
        for instruction in &resolution.instructions {
            self.instruction_piece(instruction, slot, &mut pieces);
        }
        pieces.extend(literal_texts(prepend).map(|text| Piece::Width(line_width(text))));
        pieces
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
/// controls the gaps strictly inside its slot range; processing groups
/// outer-first lets an inner group overwrite its ancestors.
fn gap_controllers(slot_count: usize, forest: &GroupForest) -> Vec<Option<GroupId>> {
    let mut controllers = vec![None; slot_count];
    for index in 0..forest.len() {
        let group = GroupId(index as u32);
        let node = forest.get(group);
        for gap in (node.first_slot + 1)..=node.last_slot {
            controllers[gap] = Some(group);
        }
    }
    controllers
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
