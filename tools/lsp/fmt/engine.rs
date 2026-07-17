// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The query-based formatter engine: linearization of the syntax tree into
//! token slots, rule dispatch, and resolution of atoms into a `FormatPlan`.
//!
//! See `API_DESIGN.md` in this directory for the overall design.

use crate::fmt::atoms::{
    Annotations, Atom, AtomInstance, AtomSink, FormatPlan, INDENT, Instruction, Marker, Tier,
    Whitespace,
};
use crate::fmt::writer::TokenWriter;
use i_slint_compiler::parser::{
    NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
};
use std::collections::HashMap;

pub fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
}

/// One significant (non-trivia) token together with the trivia that precedes
/// it in the input.
pub struct TokenSlot {
    /// The Whitespace and Comment tokens between the previous significant
    /// token and `token`, in input order. Within a gap, trivia has the shape
    /// `whitespace? (comment whitespace?)*`.
    pub gap_before: Vec<SyntaxToken>,
    pub token: SyntaxToken,
}

impl TokenSlot {
    /// Total number of newlines in the gap's whitespace.
    // Not used by the shipped ruleset yet; exercised by the engine tests.
    #[allow(dead_code)]
    pub fn gap_newlines(&self) -> usize {
        self.gap_before
            .iter()
            .filter(|token| token.kind() == SyntaxKind::Whitespace)
            .map(|token| token.text().matches('\n').count())
            .sum()
    }

    /// Whether the gap contains at least one blank line (a whitespace token
    /// with two or more newlines).
    // Not used by the shipped ruleset yet; exercised by the engine tests.
    #[allow(dead_code)]
    pub fn has_blank_line(&self) -> bool {
        self.gap_before.iter().any(|token| {
            token.kind() == SyntaxKind::Whitespace && whitespace_has_blank_line(token.text())
        })
    }

    pub fn has_comment(&self) -> bool {
        self.gap_before.iter().any(|token| token.kind() == SyntaxKind::Comment)
    }

    /// The gap's whitespace token, for comment-free gaps. Such gaps have at
    /// most one whitespace token because the lexer merges adjacent
    /// whitespace.
    pub fn single_whitespace_token(&self) -> Option<&SyntaxToken> {
        debug_assert!(!self.has_comment());
        debug_assert!(self.gap_before.len() <= 1);
        self.gap_before.first()
    }

    /// Split the gap's trivia at its comments. The lexer's maximal munch
    /// guarantees the shape `whitespace? (comment whitespace?)*`, so every
    /// sub-gap is zero or one whitespace tokens.
    pub fn split_gap(&self) -> GapStructure {
        let mut structure = GapStructure { sub_gaps: Vec::new(), comments: Vec::new() };
        let mut pending_whitespace = None;
        for (index, token) in self.gap_before.iter().enumerate() {
            match token.kind() {
                SyntaxKind::Whitespace => {
                    debug_assert!(pending_whitespace.is_none(), "adjacent whitespace tokens");
                    pending_whitespace = Some(index);
                }
                SyntaxKind::Comment => {
                    structure.sub_gaps.push(pending_whitespace.take());
                    structure.comments.push(index);
                }
                _ => debug_assert!(false, "gap contains a non-trivia token"),
            }
        }
        structure.sub_gaps.push(pending_whitespace);
        structure
    }

    /// The text of a sub-gap's whitespace (`""` for empty sub-gaps).
    pub fn whitespace_text(&self, whitespace_index: Option<usize>) -> &str {
        whitespace_index.map_or("", |index| self.gap_before[index].text())
    }
}

/// A gap's trivia, split at its comments: `sub_gaps[i]` is the `gap_before`
/// index of the whitespace token before comment `i` (`None` when that
/// sub-gap is empty), and the final entry is the whitespace before the right
/// token. Invariant: `sub_gaps.len() == comments.len() + 1`.
pub struct GapStructure {
    pub sub_gaps: Vec<Option<usize>>,
    pub comments: Vec<usize>,
}

/// The document as a flat list of significant tokens, each carrying its
/// preceding trivia.
pub struct Linearization {
    pub slots: Vec<TokenSlot>,
    /// Trivia after the last significant token. Normally empty: the parser
    /// emits an explicit `Eof` token whose `gap_before` captures the trailing
    /// file trivia. Only error-truncated trees (where the parser stopped
    /// before consuming `Eof`) can leave trivia here.
    pub trailing_trivia: Vec<SyntaxToken>,
}

/// Flatten the tree into significant-token slots by walking the token stream
/// once.
///
/// Trivia cannot be located through tree structure — the parser flushes
/// whitespace and comments into whatever node is open when it peeks ahead, so
/// trailing trivia frequently ends up *inside* the preceding node. The linear
/// walk (via `SyntaxToken::next_token`, which also works around a rowan bug
/// with empty nodes) sidesteps that entirely.
pub fn linearize(document: &SyntaxNode) -> Linearization {
    let mut slots = Vec::new();
    let mut gap = Vec::new();
    let mut visited_length = 0usize;
    let mut current = document.first_token();
    while let Some(token) = current {
        current = token.next_token();
        visited_length += token.text().len();
        if is_trivia(token.kind()) {
            gap.push(token);
        } else {
            slots.push(TokenSlot { gap_before: std::mem::take(&mut gap), token });
        }
    }
    debug_assert_eq!(
        visited_length,
        usize::from(document.text_range().len()),
        "the token walk missed part of the document"
    );
    Linearization { slots, trailing_trivia: gap }
}

/// Whether a token contributes to a node's significant span. `Eof` does not:
/// it is a synthetic zero-length terminator sitting *after* the trailing file
/// trivia, so including it would drag that trivia into the Document's span.
fn is_significant(token: &SyntaxToken) -> bool {
    !is_trivia(token.kind()) && token.kind() != SyntaxKind::Eof
}

/// The first significant token inside `node`, or `None` for token-less nodes
/// (the parser produces e.g. empty `Expression` nodes).
pub fn first_significant_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens().find_map(|child| match child {
        NodeOrToken::Token(token) => is_significant(&token).then_some(token),
        NodeOrToken::Node(child_node) => first_significant_token(&child_node),
    })
}

/// The last non-trivia token inside `node`, or `None` for token-less nodes.
///
/// Implemented by reverse child recursion: `node.last_token()` is usually a
/// Whitespace token (trailing trivia lives inside nodes), and rowan's
/// `prev_token` does not have the empty-node workaround that `next_token`
/// has.
pub fn last_significant_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    let children: Vec<NodeOrToken> = node.children_with_tokens().collect();
    children.into_iter().rev().find_map(|child| match child {
        NodeOrToken::Token(token) => is_significant(&token).then_some(token),
        NodeOrToken::Node(child_node) => last_significant_token(&child_node),
    })
}

/// The input span from the start of `node`'s first significant token to the
/// end of its last one.
///
/// This is the span to measure multilineness on. `node.text_range()` would be
/// wrong: it includes trailing trivia, so `x: 1; // comment` plus its final
/// newline would make a single-line binding look multiline.
pub fn significant_span(node: &SyntaxNode) -> Option<TextRange> {
    let first = first_significant_token(node)?;
    let last = last_significant_token(node)?;
    Some(TextRange::new(first.text_range().start(), last.text_range().end()))
}

/// A set of significant items (nodes or tokens) that a rule attaches atoms
/// to.
///
/// A rule receives a `Selection` holding just its matched node; the
/// navigation methods derive sub-selections from it. Trivia (whitespace and
/// comments) is never selectable — comments are handled by the engine core,
/// not by rules.
///
/// For anything the navigation methods cannot express, [`Selection::iter`]
/// escapes to the plain syntax tree and [`Selection::at`] re-enters.
pub struct Selection<'a> {
    items: Vec<NodeOrToken>,
    /// The rule's node (for token rules: the matched token's parent node).
    /// Softlines attached through this selection (or any selection derived
    /// from it) measure multilineness on this node's significant-token span,
    /// and [`Selection::is_multiline`] reports it.
    context: SyntaxNode,
    tier: Tier,
    sink: &'a AtomSink,
    /// The input text, for multilineness measurement.
    source: &'a str,
}

impl<'a> Selection<'a> {
    fn derived(&self, items: Vec<NodeOrToken>) -> Selection<'a> {
        Selection {
            items,
            context: self.context.clone(),
            tier: self.tier,
            sink: self.sink,
            source: self.source,
        }
    }

    fn child_tokens(&self) -> impl Iterator<Item = SyntaxToken> + '_ {
        self.items
            .iter()
            .filter_map(NodeOrToken::as_node)
            .flat_map(|node| node.children_with_tokens())
            .filter_map(NodeOrToken::into_token)
            // "Trivia is never selectable" is enforced here, structurally,
            // not by each caller's kind filter. Eof is excluded too: it is
            // the synthetic terminator after the trailing file trivia, so
            // atoms anchored to it would resolve past the last real gap.
            .filter(is_significant)
    }

    /// The direct child nodes of the selected nodes with the given kind.
    pub fn node(&self, kind: SyntaxKind) -> Selection<'a> {
        self.derived(
            self.items
                .iter()
                .filter_map(NodeOrToken::as_node)
                .flat_map(|node| node.children())
                .filter(|child| child.kind() == kind)
                .map(NodeOrToken::Node)
                .collect(),
        )
    }

    /// The direct child tokens of the selected nodes with the given kind.
    pub fn token(&self, kind: SyntaxKind) -> Selection<'a> {
        debug_assert!(
            !is_trivia(kind) && kind != SyntaxKind::Eof,
            "rules must not select trivia or Eof"
        );
        self.token_matching(|child_kind| child_kind == kind)
    }

    /// The direct child tokens of the selected nodes whose kind matches the
    /// predicate. For matching a set of kinds, e.g. the operator of a
    /// `BinaryExpression`, where [`Selection::token`] would need one call per
    /// kind.
    pub fn token_matching(&self, predicate: impl Fn(SyntaxKind) -> bool) -> Selection<'a> {
        self.derived(
            self.child_tokens()
                .filter(|token| predicate(token.kind()))
                .map(NodeOrToken::Token)
                .collect(),
        )
    }

    /// All significant direct children (nodes and tokens) of the selected
    /// nodes, in source order. Token-less nodes (the parser produces e.g.
    /// empty `Expression` nodes) are skipped, so pairwise iteration never
    /// sees an item without a boundary to attach atoms to.
    pub fn children(&self) -> Selection<'a> {
        self.derived(
            self.items
                .iter()
                .filter_map(NodeOrToken::as_node)
                .flat_map(|node| node.children_with_tokens())
                .filter(|child| match child {
                    NodeOrToken::Token(token) => is_significant(token),
                    NodeOrToken::Node(node) => first_significant_token(node).is_some(),
                })
                .collect(),
        )
    }

    /// The selected items, for plain-Rust processing (pairwise windows,
    /// position-dependent logic, rowan navigation). [`Selection::at`]
    /// re-enters the selection API from whatever the iteration found.
    pub fn iter(&self) -> impl Iterator<Item = &NodeOrToken> + '_ {
        self.items.iter()
    }

    /// A selection of one arbitrary node or token, inheriting this
    /// selection's rule context (measure span for softlines), tier, and
    /// sink. This is the escape hatch back from plain rowan navigation.
    pub fn at(&self, item: impl Into<NodeOrToken>) -> Selection<'a> {
        let item = item.into();
        // Linear token navigation (`next_token`) is exactly how a rule can
        // run into trivia or the trailing Eof, so the escape hatch checks
        // for both.
        debug_assert!(
            item.as_token().is_none_or(is_significant),
            "rules must not select trivia or Eof"
        );
        self.derived(vec![item])
    }

    /// The direct child `Identifier` tokens with the given text.
    ///
    /// Keywords like `states` or `when` are plain identifiers in the Slint
    /// grammar. Matching direct children only is what keeps this safe:
    /// identifiers in nested expressions are out of reach.
    pub fn keyword(&self, text: &str) -> Selection<'a> {
        self.derived(
            self.child_tokens()
                .filter(|token| token.kind() == SyntaxKind::Identifier && token.text() == text)
                .map(NodeOrToken::Token)
                .collect(),
        )
    }

    /// Whether the rule's node spans multiple lines in the input, measured
    /// over its significant-token span — the same measurement the softline
    /// constructors below use, so annotation-time conditions and softline
    /// resolution can never disagree.
    // Not used by the shipped ruleset yet; exercised by the engine tests.
    #[allow(dead_code)]
    pub fn is_multiline(&self) -> bool {
        self.source[self.measure_span()].contains('\n')
    }

    /// A softline measured on the rule's node: resolves to a newline if the
    /// node was multiline in the input, else to a space.
    pub fn spaced_softline(&self) -> Atom {
        Atom::SpacedSoftline(self.measure_span())
    }

    /// A softline measured on the rule's node: resolves to a newline if the
    /// node was multiline in the input, else to nothing.
    pub fn empty_softline(&self) -> Atom {
        Atom::EmptySoftline(self.measure_span())
    }

    /// The significant-token span of the rule's node. A token-less node has
    /// an empty span, which softlines resolve as single-line.
    fn measure_span(&self) -> TextRange {
        significant_span(&self.context).unwrap_or_else(|| TextRange::empty(TextSize::new(0)))
    }

    /// The boundary token of each selected item: the token itself, or the
    /// significant token `of_node` picks (first or last). Token-less items
    /// (e.g. empty `Expression` nodes) have no boundary and are skipped.
    fn boundary_tokens(
        &self,
        of_node: fn(&SyntaxNode) -> Option<SyntaxToken>,
    ) -> impl Iterator<Item = SyntaxToken> + '_ {
        self.items.iter().filter_map(move |item| match item {
            NodeOrToken::Token(token) => Some(token.clone()),
            NodeOrToken::Node(node) => of_node(node),
        })
    }

    /// Attach an atom to the boundary before each selected item (before a
    /// node's first significant token).
    pub fn prepend(&self, atom: Atom) -> &Self {
        for anchor in self.boundary_tokens(first_significant_token) {
            self.sink.attach_before(
                anchor.text_range().start(),
                AtomInstance { atom: atom.clone(), tier: self.tier },
            );
        }
        self
    }

    /// Attach an atom to the boundary after each selected item (after a
    /// node's last significant token).
    pub fn append(&self, atom: Atom) -> &Self {
        for anchor in self.boundary_tokens(last_significant_token) {
            self.sink.attach_after(
                anchor.text_range().start(),
                AtomInstance { atom: atom.clone(), tier: self.tier },
            );
        }
        self
    }

    /// Mark each selected item as a leaf: its interior is emitted verbatim
    /// and no rule can touch the gaps between its tokens. Use this to shield
    /// spans of foreign syntax whose tokens the global rules would otherwise
    /// mangle — the arbitrary Rust inside `@rust-attr(...)`, or the
    /// expressions interpolated into a string template. Token-less nodes have
    /// nothing to protect and are skipped.
    pub fn leaf(&self) -> &Self {
        self.mark(Marker::Leaf)
    }

    /// Mark each selected item for deletion: its tokens are emitted as
    /// nothing and the whitespace around them collapses into the surrounding
    /// gap. In practice a rule deletes a single token — a now-redundant
    /// trailing comma when a list collapses onto one line. A delete inside a
    /// leaf range is ignored (the leaf keeps its interior verbatim).
    ///
    /// A deleted token's own boundary atoms are discarded, so a rule that
    /// deletes a comma and injects a replacement elsewhere must attach the
    /// injected [`Atom::Literal`] to a *surviving* neighbor (e.g. append it to
    /// the last argument), not to the deleted token.
    // Not used by the shipped ruleset yet; exercised by the engine tests.
    #[allow(dead_code)]
    pub fn delete(&self) -> &Self {
        self.mark(Marker::Delete)
    }

    /// Mark each selected item's range (a token's range, a node's significant
    /// span). Token-less nodes have no range and are skipped.
    fn mark(&self, marker: Marker) -> &Self {
        for item in &self.items {
            let range = match item {
                NodeOrToken::Token(token) => Some(token.text_range()),
                NodeOrToken::Node(node) => significant_span(node),
            };
            if let Some(range) = range {
                self.sink.mark(range, marker);
            }
        }
        self
    }
}

/// The rules of a formatting style, keyed by what they match.
///
/// Rules run once per matched node or token instance during [`annotate`];
/// each receives a [`Selection`] holding the matched item.
#[derive(Default)]
pub struct FormatRules {
    node_rules: HashMap<SyntaxKind, Vec<Box<dyn Fn(&Selection)>>>,
    token_rules: HashMap<SyntaxKind, Vec<Box<dyn Fn(&Selection)>>>,
    wildcard_rules: Vec<Box<dyn Fn(&Selection)>>,
}

impl FormatRules {
    /// Register a rule for every node of the given kind.
    pub fn node(&mut self, kind: SyntaxKind, rule: impl Fn(&Selection) + 'static) {
        self.node_rules.entry(kind).or_default().push(Box::new(rule));
    }

    /// Register a rule for every token of the given kind, anywhere in the
    /// document. Node rules override token rules where they conflict.
    pub fn token(&mut self, kind: SyntaxKind, rule: impl Fn(&Selection) + 'static) {
        self.token_rules.entry(kind).or_default().push(Box::new(rule));
    }

    /// Register a rule that runs for every node in the document. It runs at
    /// [`Tier::Wildcard`]: node rules override it, but it overrides token
    /// rules. Keep the body to a single scan of the node's children — it
    /// runs everywhere.
    ///
    /// CONTRACT for the adjacent-node spacing rule: only attach `Space`
    /// between two child *nodes* with no significant token between them.
    /// Because Wildcard sits above Token, a wildcard `Space` next to a
    /// punctuation token would beat the token rules' `Antispace` and
    /// re-space every `:`/`;`/`,` in the document.
    // Not used by the shipped ruleset yet; exercised by the engine tests.
    #[allow(dead_code)]
    pub fn any_node(&mut self, rule: impl Fn(&Selection) + 'static) {
        self.wildcard_rules.push(Box::new(rule));
    }
}

/// Phase 1: run all rules over the document, collecting atoms in `sink`.
pub fn annotate(
    document: &SyntaxNode,
    slots: &[TokenSlot],
    rules: &FormatRules,
    sink: &AtomSink,
    source: &str,
) {
    for slot in slots {
        // Eof is the synthetic zero-length terminator; a rule keyed on it
        // would measure the whole document and anchor atoms past the last
        // real gap.
        if slot.token.kind() == SyntaxKind::Eof {
            continue;
        }
        let Some(token_rules) = rules.token_rules.get(&slot.token.kind()) else { continue };
        let selection = Selection {
            items: vec![NodeOrToken::Token(slot.token.clone())],
            context: slot.token.parent(),
            tier: Tier::Token,
            sink,
            source,
        };
        for rule in token_rules {
            rule(&selection);
        }
    }

    for node in document.descendants() {
        let node_rules = rules.node_rules.get(&node.kind());
        if node_rules.is_none() && rules.wildcard_rules.is_empty() {
            continue;
        }
        let mut selection = Selection {
            items: vec![NodeOrToken::Node(node.clone())],
            context: node.clone(),
            tier: Tier::Wildcard,
            sink,
            source,
        };
        for rule in &rules.wildcard_rules {
            rule(&selection);
        }
        // Reusing the selection at the higher tier is safe: atoms record the
        // tier when they are attached, and derived selections copy it
        // eagerly, so nothing a wildcard rule produced can observe this
        // change.
        selection.tier = Tier::Node;
        for rule in node_rules.into_iter().flatten() {
            rule(&selection);
        }
    }
}

/// The comparable "how much whitespace" outcome of one resolved atom.
/// Variant order defines strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Strength {
    Nothing,
    Space,
    Newline,
}

/// Phase 2: collapse the collected atoms into concrete instructions.
///
/// Every gap is resolved: a rule's atoms decide the whitespace where they
/// fire, and elsewhere the default applies (a single space between tokens,
/// nothing at the document edges). The exceptions are gaps inside a [`leaf`]
/// range, which pass through verbatim. Gaps containing comments are split at
/// the comments and resolved per sub-gap (see [`resolve_gap`]).
///
/// [`leaf`]: Selection::leaf
pub fn resolve(slots: &[TokenSlot], annotations: &Annotations, source: &str) -> FormatPlan {
    let boundary_atoms = &annotations.boundary;
    let leaf_ranges = normalize_leaf_ranges(&annotations.markers);
    // Which leaf range (if any) each slot's token sits inside.
    let leaf_of_slot: Vec<Option<usize>> = slots
        .iter()
        .map(|slot| containing_leaf(slot.token.text_range().start(), &leaf_ranges))
        .collect();
    let delete_ranges = delete_ranges(&annotations.markers);

    let mut instructions = Vec::with_capacity(slots.len() * 2);
    let mut indentation_level: i32 = 0;
    // The most recent token that was actually emitted. A deleted token does
    // not become one, so its predecessor's appended atoms carry across it to
    // the next surviving gap — the deleted token's own trivia and atoms
    // collapse away.
    let mut last_surviving: Option<usize> = None;

    #[cfg(debug_assertions)]
    let mut idempotency_guard = IdempotencyGuard::default();

    for (slot_index, slot) in slots.iter().enumerate() {
        let leaf = leaf_of_slot[slot_index];
        // A delete inside a leaf is ignored: the leaf keeps its interior
        // verbatim (precedence goes to the leaf).
        let deleted = leaf.is_none()
            && delete_ranges.iter().any(|range| range.contains(slot.token.text_range().start()));
        if deleted {
            // The token emits nothing; its gap's whitespace collapses (but
            // any comment in it is kept — comments are never deleted).
            if slot.has_comment() {
                instructions.push(Instruction::KeepGap { slot: slot_index });
            } else {
                instructions.push(Instruction::ReplaceGap {
                    slot: slot_index,
                    whitespace: Whitespace::None,
                });
            }
            instructions.push(Instruction::DeleteToken { slot: slot_index });
            continue;
        }

        // The gap before this token is one physical location: the last
        // surviving token's appended atoms and this token's prepended atoms
        // meet here. They are kept apart because they route to different
        // sub-gaps when the gap contains comments.
        let no_atoms: &[AtomInstance] = &[];
        let append_atoms = last_surviving
            .and_then(|index| boundary_atoms.after.get(&slots[index].token.text_range().start()))
            .map_or(no_atoms, Vec::as_slice);
        let prepend_atoms = boundary_atoms
            .before
            .get(&slot.token.text_range().start())
            .map_or(no_atoms, Vec::as_slice);

        // A gap wholly inside a leaf (both flanking tokens in the same leaf
        // range) is kept verbatim whatever the rules said. The boundary gaps
        // just before and after the leaf resolve normally, so a rule can
        // still space the leaf against its surroundings. Indentation still
        // accrues so a balanced brace pair inside a leaf keeps the counter
        // exact.
        let leaf_internal =
            slot_index > 0 && leaf.is_some() && leaf == leaf_of_slot[slot_index - 1];

        // Literals inject fixed text without deciding whitespace: an
        // append-literal hugs the left token (before the gap's whitespace), a
        // prepend-literal hugs this token (after it). Inside a leaf they are
        // suppressed like every other boundary effect.
        if !leaf_internal {
            for text in literal_texts(append_atoms) {
                instructions.push(Instruction::EmitLiteral { text: text.to_string() });
            }
        }

        #[cfg(debug_assertions)]
        let gap_instructions_start = instructions.len();
        if leaf_internal {
            // A leaf emits its interior verbatim. Indentation still accrues so
            // a balanced brace pair inside it keeps the running level exact.
            indentation_level += net_indentation(append_atoms) + net_indentation(prepend_atoms);
            instructions.push(Instruction::KeepGap { slot: slot_index });
        } else {
            resolve_gap(
                slot,
                slot_index,
                append_atoms,
                prepend_atoms,
                DocumentEdges {
                    start: last_surviving.is_none(),
                    end: slot.token.kind() == SyntaxKind::Eof,
                },
                source,
                &mut indentation_level,
                &mut instructions,
            );
        }

        // Leaf-internal gaps are exempt from the guard: their atoms were
        // suppressed above, so nothing was emitted.
        #[cfg(debug_assertions)]
        if !leaf_internal {
            idempotency_guard.record_gap(
                append_atoms.iter().chain(prepend_atoms),
                &instructions[gap_instructions_start..],
                slot.token.text_range().start(),
                source,
            );
        }

        if !leaf_internal {
            for text in literal_texts(prepend_atoms) {
                instructions.push(Instruction::EmitLiteral { text: text.to_string() });
            }
        }
        instructions.push(Instruction::EmitToken { slot: slot_index });
        last_surviving = Some(slot_index);
    }

    #[cfg(debug_assertions)]
    idempotency_guard.check(source);

    FormatPlan { instructions }
}

/// Debug-only idempotency guard: a `Hardline` that produces a newline
/// strictly inside a softline span which resolved single-line would flip
/// that span to multiline on the next run, breaking
/// `format(format(x)) == format(x)`.
#[cfg(debug_assertions)]
#[derive(Default)]
struct IdempotencyGuard {
    single_line_softline_spans: Vec<TextRange>,
    hardline_newline_positions: Vec<TextSize>,
}

#[cfg(debug_assertions)]
impl IdempotencyGuard {
    /// Record one resolved gap: the softline spans its atoms measured as
    /// single-line, and whether a `Hardline` produced a newline here.
    fn record_gap<'atoms>(
        &mut self,
        atoms: impl Iterator<Item = &'atoms AtomInstance> + Clone,
        gap_instructions: &[Instruction],
        position: TextSize,
        source: &str,
    ) {
        for instance in atoms.clone() {
            if let Atom::SpacedSoftline(span) | Atom::EmptySoftline(span) = instance.atom {
                if !source[span].contains('\n') {
                    self.single_line_softline_spans.push(span);
                }
            }
        }
        let produced_newline = gap_instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::ReplaceGap { whitespace: Whitespace::Newline { .. }, .. }
                    | Instruction::ReplaceSubGap { whitespace: Whitespace::Newline { .. }, .. }
            )
        });
        let has_hardline = atoms.into_iter().any(|instance| instance.atom == Atom::Hardline);
        if has_hardline && produced_newline {
            self.hardline_newline_positions.push(position);
        }
    }

    fn check(&self, source: &str) {
        for &position in &self.hardline_newline_positions {
            if let Some(span) = self
                .single_line_softline_spans
                .iter()
                .find(|span| span.start() < position && position < span.end())
            {
                panic!(
                    "formatter idempotency violation: a Hardline produced a newline at offset {} \
                     strictly inside the softline-measured span {:?} ({:?}), which resolved \
                     single-line — the span would become multiline on the next run. Use Hardline \
                     only where no softline measures it (e.g. at Document top level).",
                    u32::from(position),
                    span,
                    &source[*span],
                );
            }
        }
    }
}

/// The text of every [`Atom::Literal`] among `atoms`, in attachment order.
fn literal_texts(atoms: &[AtomInstance]) -> impl Iterator<Item = &str> {
    atoms.iter().filter_map(|instance| match &instance.atom {
        Atom::Literal(text) => Some(text.as_str()),
        _ => None,
    })
}

/// The ranges marked for deletion (single tokens in practice).
fn delete_ranges(markers: &[(TextRange, Marker)]) -> Vec<TextRange> {
    markers
        .iter()
        .filter(|(_, marker)| *marker == Marker::Delete)
        .map(|(range, _)| *range)
        .collect()
}

/// Reduce the collected leaf markers to sorted, disjoint ranges. Leaf ranges
/// are significant spans, which nest or are disjoint but never partially
/// overlap; sorting by start ascending then end descending and keeping a
/// range only when it extends past every kept range so far therefore yields
/// the outermost spans, deduplicated. Kept ends increase strictly, so the
/// last kept range is the running maximum — comparing against it suffices.
fn normalize_leaf_ranges(markers: &[(TextRange, Marker)]) -> Vec<TextRange> {
    let mut ranges: Vec<TextRange> = markers
        .iter()
        .filter(|(_, marker)| *marker == Marker::Leaf)
        .map(|(range, _)| *range)
        .collect();
    ranges.sort_by_key(|range| (range.start(), std::cmp::Reverse(range.end())));
    let mut disjoint: Vec<TextRange> = Vec::new();
    for range in ranges {
        if disjoint.last().is_none_or(|last| range.end() > last.end()) {
            disjoint.push(range);
        }
    }
    disjoint
}

/// The index of the sorted, disjoint leaf range that contains `offset`, if any.
fn containing_leaf(offset: TextSize, leaf_ranges: &[TextRange]) -> Option<usize> {
    leaf_ranges.iter().position(|range| range.contains(offset))
}

fn net_indentation(atoms: &[AtomInstance]) -> i32 {
    atoms
        .iter()
        .map(|instance| match instance.atom {
            Atom::IndentStart => 1,
            Atom::IndentEnd => -1,
            _ => 0,
        })
        .sum()
}

fn whitespace_has_blank_line(whitespace: &str) -> bool {
    whitespace.matches('\n').count() >= 2
}

/// The atoms routed to one sub-gap, plus their bookkeeping side effects.
#[derive(Default)]
struct SubGapAtoms {
    /// Pre-resolved whitespace decisions. The merge is the lexicographic
    /// (tier, strength) maximum: the highest tier that contributed anything
    /// wins, and the strongest atom within it decides.
    decisions: Vec<(Tier, Strength)>,
    antispace_tier: Option<Tier>,
    allow_blank_lines: bool,
    indentation_delta: i32,
}

/// Resolve an atom to its strength and file it into the sub-gap it belongs
/// to: whitespace decisions that resolved to a newline go to
/// `newline_target`, everything else stays at `space_target` (for the left
/// token's appends, newlines transfer past hanging comments while spaces
/// stay put — R2 in API_DESIGN.md). Indentation and blank-line bookkeeping
/// travels with the newline side.
fn route_atom(
    instance: &AtomInstance,
    space_target: usize,
    newline_target: usize,
    input_softline_whitespace: &str,
    source: &str,
    sub_gaps: &mut [SubGapAtoms],
) {
    let strength = match instance.atom {
        Atom::Space => Strength::Space,
        Atom::Hardline => Strength::Newline,
        Atom::SpacedSoftline(measure_span) => {
            if source[measure_span].contains('\n') {
                Strength::Newline
            } else {
                Strength::Space
            }
        }
        Atom::EmptySoftline(measure_span) => {
            if source[measure_span].contains('\n') {
                Strength::Newline
            } else {
                Strength::Nothing
            }
        }
        Atom::InputSoftline => {
            if input_softline_whitespace.contains('\n') {
                Strength::Newline
            } else {
                // Advisory: without an input newline the atom says nothing
                // at all. Pushing a tier-bearing `Nothing` instead would
                // veto weaker-tier decisions (e.g. a wildcard `Space`) and
                // glue the tokens together.
                return;
            }
        }
        Atom::Antispace => {
            let antispace_tier = &mut sub_gaps[space_target].antispace_tier;
            *antispace_tier = (*antispace_tier).max(Some(instance.tier));
            return;
        }
        Atom::IndentStart => {
            sub_gaps[newline_target].indentation_delta += 1;
            return;
        }
        Atom::IndentEnd => {
            sub_gaps[newline_target].indentation_delta -= 1;
            return;
        }
        Atom::AllowBlankLines => {
            sub_gaps[newline_target].allow_blank_lines = true;
            return;
        }
        // Literals are emitted separately (they are not whitespace); they
        // never reach the whitespace decision here.
        Atom::Literal(_) => return,
    };
    let target = if strength == Strength::Newline { newline_target } else { space_target };
    sub_gaps[target].decisions.push((instance.tier, strength));
}

/// Resolve one gap into whitespace instructions.
///
/// The gap's trivia is split at its comments into sub-gaps S0..Sn; the left
/// token's appends route to S0 (space-level) or past the hanging comments
/// (newlines), the right token's prepends route to Sn, and each sub-gap
/// resolves independently. A comment-free gap is the degenerate single
/// sub-gap case and produces a single `ReplaceGap`. A sub-gap that no rule
/// touched resolves to `default`.
///
/// Comments never move lines: a boundary that had a newline in the input
/// keeps it, whatever the rules say — rules only influence the indentation.
/// A comment that starts at column 0 in the input additionally keeps
/// indentation level 0 (compiler syntax tests rely on the columns of such
/// comments).
/// Whether a slot's gap touches a document edge: the gap before the first
/// significant token (`start`), the gap before the terminating Eof (`end`).
struct DocumentEdges {
    start: bool,
    end: bool,
}

fn resolve_gap(
    slot: &TokenSlot,
    slot_index: usize,
    append_atoms: &[AtomInstance],
    prepend_atoms: &[AtomInstance],
    edges: DocumentEdges,
    source: &str,
    indentation_level: &mut i32,
    instructions: &mut Vec<Instruction>,
) {
    let structure = slot.split_gap();
    let comment_count = structure.comments.len();
    let expected_level_after =
        *indentation_level + net_indentation(append_atoms) + net_indentation(prepend_atoms);

    // Hanging comments: the maximal prefix with no newline before them.
    let trailing_count = (0..comment_count)
        .take_while(|&index| !slot.whitespace_text(structure.sub_gaps[index]).contains('\n'))
        .count();

    let mut sub_gaps: Vec<SubGapAtoms> = Vec::new();
    sub_gaps.resize_with(comment_count + 1, SubGapAtoms::default);
    for instance in append_atoms {
        route_atom(
            instance,
            0,
            trailing_count,
            slot.whitespace_text(structure.sub_gaps[trailing_count]),
            source,
            &mut sub_gaps,
        );
    }
    for instance in prepend_atoms {
        route_atom(
            instance,
            comment_count,
            comment_count,
            slot.whitespace_text(structure.sub_gaps[comment_count]),
            source,
            &mut sub_gaps,
        );
    }

    for (index, sub_gap) in sub_gaps.iter().enumerate() {
        let whitespace_index = structure.sub_gaps[index];
        let whitespace = slot.whitespace_text(whitespace_index);
        *indentation_level += sub_gap.indentation_delta;

        let decision: Whitespace = if comment_count > 0 && whitespace.contains('\n') {
            // A comment-adjacent boundary with an input newline keeps it
            // (and its blank line, capped at one). Rules only set the level.
            let before_column_zero_comment = index < comment_count && whitespace.ends_with('\n');
            Whitespace::Newline {
                blank_line: whitespace_has_blank_line(whitespace),
                indentation_level: if before_column_zero_comment {
                    0
                } else {
                    (*indentation_level).max(0) as u32
                },
            }
        } else {
            let strength = if sub_gap.decisions.is_empty() && sub_gap.antispace_tier.is_none() {
                // No rule decided this boundary: a single space between two
                // adjacent items, nothing at a document edge where there is
                // no adjacency to space. In a gap with comments only the
                // edge-touching sub-gap is the edge; the others — between
                // the significant token and a hanging comment, or between
                // two comments — have real adjacency and keep the space.
                let at_document_edge =
                    (edges.start && index == 0) || (edges.end && index == comment_count);
                if at_document_edge { Strength::Nothing } else { Strength::Space }
            } else {
                let merged = sub_gap.decisions.iter().copied().max();
                match merged {
                    // Only Antispace atoms contributed: nothing between the
                    // tokens.
                    None => Strength::Nothing,
                    // Antispace cancels a Space decision of its own or a lower
                    // tier — never a newline.
                    Some((tier, Strength::Space)) if sub_gap.antispace_tier >= Some(tier) => {
                        Strength::Nothing
                    }
                    Some((_, strength)) => strength,
                }
            };
            match strength {
                Strength::Nothing => Whitespace::None,
                Strength::Space => Whitespace::Space,
                Strength::Newline => Whitespace::Newline {
                    blank_line: sub_gap.allow_blank_lines && whitespace_has_blank_line(whitespace),
                    indentation_level: (*indentation_level).max(0) as u32,
                },
            }
        };

        if comment_count == 0 {
            instructions.push(Instruction::ReplaceGap { slot: slot_index, whitespace: decision });
        } else {
            instructions.push(Instruction::ReplaceSubGap {
                slot: slot_index,
                trivia_index: whitespace_index,
                whitespace: decision,
            });
        }

        // Emit the comment that follows this sub-gap. A comment placed on
        // its own (possibly re-indented) line shifts its continuation lines
        // along; hanging or inline comments stay as written.
        if index < comment_count {
            let trivia_index = structure.comments[index];
            let column_shift = match decision {
                Whitespace::Newline { indentation_level: new_level, .. } => {
                    let comment_start =
                        usize::from(slot.gap_before[trivia_index].text_range().start());
                    new_level as i32 * INDENT.len() as i32 - column_at(source, comment_start) as i32
                }
                _ => 0,
            };
            instructions.push(Instruction::EmitComment {
                slot: slot_index,
                trivia_index,
                column_shift,
            });
        }
    }

    debug_assert_eq!(
        *indentation_level, expected_level_after,
        "sub-gap routing must apply every indentation delta exactly once"
    );
}

/// The column (in bytes) at which `offset` sits on its line. Bytes equal
/// columns for the formatter's use: only the immediately preceding ASCII
/// whitespace token can sit between the last newline and a comment (a tab
/// counts as one column, matching the renderer's continuation-line shift).
fn column_at(source: &str, offset: usize) -> usize {
    offset - source[..offset].rfind('\n').map_or(0, |newline_offset| newline_offset + 1)
}

/// The comment that turns off formatting for the construct that follows it.
const IGNORE_DIRECTIVE: &str = "// slint-fmt:ignore";

/// Honor `// slint-fmt:ignore` comments: each one leafs the construct that
/// starts at the next significant token, so it is emitted verbatim. This is
/// engine-core (not a rule) because it produces the same [`Marker::Leaf`]
/// the leaf rules do — just triggered by a comment rather than a node kind.
fn apply_ignore_directives(slots: &[TokenSlot], sink: &AtomSink) {
    for slot in slots {
        // A directive comment governs the significant token that follows it,
        // which is this slot's own token. Eof ends no construct.
        if slot.token.kind() == SyntaxKind::Eof {
            continue;
        }
        // Trailing whitespace is part of a line-comment token but invisible
        // to the writer, so ignore it when matching; everything else must be
        // exact.
        let has_directive = slot.gap_before.iter().any(|trivia| {
            trivia.kind() == SyntaxKind::Comment && trivia.text().trim_end() == IGNORE_DIRECTIVE
        });
        if has_directive {
            if let Some(range) = ignored_span(&slot.token) {
                sink.mark(range, Marker::Leaf);
            }
        }
    }
}

/// The significant span of the largest construct that begins at `token`,
/// capped below the Document so a top-level directive ignores one item rather
/// than the whole file. `None` when no node starts at `token` (e.g. it is a
/// closing brace), in which case the directive has nothing to ignore.
fn ignored_span(token: &SyntaxToken) -> Option<TextRange> {
    let offset = token.text_range().start();
    let mut target = None;
    for ancestor in token.parent_ancestors() {
        // Stop before the Document, and at the first ancestor that begins
        // earlier than the token — every larger ancestor begins there too.
        if ancestor.kind() == SyntaxKind::Document
            || first_significant_token(&ancestor).map(|first| first.text_range().start())
                != Some(offset)
        {
            break;
        }
        target = Some(ancestor);
    }
    target.and_then(|node| significant_span(&node))
}

/// Format `document` with the given rules, writing the result through
/// `writer`.
pub fn format_document_with_rules(
    document: &SyntaxNode,
    rules: &FormatRules,
    writer: &mut impl TokenWriter,
) -> std::io::Result<()> {
    // Token ranges are absolute; measuring them against the text only works
    // for the parse root.
    debug_assert_eq!(
        u32::from(document.text_range().start()),
        0,
        "format_document_with_rules requires the parse root"
    );
    let source = document.text().to_string();
    let linearization = linearize(document);
    let sink = AtomSink::default();
    annotate(document, &linearization.slots, rules, &sink, &source);
    apply_ignore_directives(&linearization.slots, &sink);
    let plan = resolve(&linearization.slots, &sink.finish(), &source);
    crate::fmt::render::render(&plan, &linearization, writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_compiler::diagnostics::BuildDiagnostics;

    fn parse(source: &str) -> SyntaxNode {
        i_slint_compiler::parser::parse(
            String::from(source),
            None,
            &mut BuildDiagnostics::default(),
        )
    }

    fn find_node(document: &SyntaxNode, kind: SyntaxKind) -> SyntaxNode {
        document.descendants().find(|node| node.kind() == kind).unwrap()
    }

    #[test]
    fn linearize_captures_gaps_and_trailing_trivia() {
        let document = parse("\n// leading\ncomponent A { }\n");
        let linearization = linearize(&document);

        let token_texts: Vec<&str> =
            linearization.slots.iter().map(|slot| slot.token.text()).collect();
        assert_eq!(token_texts, ["component", "A", "{", "}", ""], "last token is Eof");

        // Leading file trivia (whitespace, comment, whitespace) is slot 0's gap.
        let leading: Vec<&str> =
            linearization.slots[0].gap_before.iter().map(|token| token.text()).collect();
        assert_eq!(leading, ["\n", "// leading", "\n"]);
        assert!(linearization.slots[0].has_comment());

        // The trailing newline belongs to the Eof slot; nothing is left over.
        let eof_slot = linearization.slots.last().unwrap();
        assert_eq!(eof_slot.token.kind(), SyntaxKind::Eof);
        assert_eq!(eof_slot.gap_before.len(), 1);
        assert_eq!(eof_slot.gap_before[0].text(), "\n");
        assert!(linearization.trailing_trivia.is_empty());
    }

    #[test]
    fn gap_facts_are_derived_from_the_tokens() {
        let document = parse("component A {\n\n    x: 1;\n}");
        let linearization = linearize(&document);

        let x_slot =
            linearization.slots.iter().find(|slot| slot.token.text() == "x").expect("slot for `x`");
        assert_eq!(x_slot.gap_newlines(), 2);
        assert!(x_slot.has_blank_line());
        assert!(!x_slot.has_comment());
        assert_eq!(x_slot.single_whitespace_token().unwrap().text(), "\n\n    ");
    }

    #[test]
    fn split_gap_covers_all_trivia_shapes() {
        // Each case: source, token whose gap we inspect, expected sub-gap
        // whitespace texts (None = empty sub-gap), expected comment texts.
        let cases: &[(&str, &str, &[Option<&str>], &[&str])] = &[
            // Completely empty gap (document start): one empty sub-gap.
            ("component A { }", "component", &[None], &[]),
            // Comment-free gap: one sub-gap, no comments.
            ("component A { }", "{", &[Some(" ")], &[]),
            // \r\n line endings stay part of the whitespace tokens.
            (
                "component A { } // t\r\nexport component B { }",
                "export",
                &[Some(" "), Some("\r\n")],
                &["// t"],
            ),
            // ws comment ws
            (
                "component A { } // t\nexport component B { }",
                "export",
                &[Some(" "), Some("\n")],
                &["// t"],
            ),
            // Comment glued to both sides: empty sub-gaps.
            ("component A {/* c */}", "}", &[None, None], &["/* c */"]),
            // Two hanging comments with a gap between them.
            (
                "component A { } /* a */ /* b */\nexport component B { }",
                "export",
                &[Some(" "), Some(" "), Some("\n")],
                &["/* a */", "/* b */"],
            ),
            // Adjacent comments: empty middle sub-gap.
            ("component A {/* a *//* b */}", "}", &[None, None, None], &["/* a */", "/* b */"]),
        ];
        for (source, token_text, expected_sub_gaps, expected_comments) in cases {
            let document = parse(source);
            let linearization = linearize(&document);
            let slot =
                linearization.slots.iter().find(|slot| slot.token.text() == *token_text).unwrap();
            let structure = slot.split_gap();
            let sub_gap_texts: Vec<Option<&str>> = structure
                .sub_gaps
                .iter()
                .map(|whitespace| whitespace.map(|index| slot.gap_before[index].text()))
                .collect();
            let comment_texts: Vec<&str> =
                structure.comments.iter().map(|index| slot.gap_before[*index].text()).collect();
            assert_eq!(sub_gap_texts, *expected_sub_gaps, "sub-gaps for {source:?}");
            assert_eq!(comment_texts, *expected_comments, "comments for {source:?}");
            assert_eq!(structure.sub_gaps.len(), structure.comments.len() + 1);
        }
    }

    #[test]
    fn significant_span_excludes_trailing_trivia() {
        let source = "component A { x: 1 + 2; }";
        let document = parse(source);

        // The parser flushes trivia into whichever node is open when it
        // peeks ahead, so some nodes end with whitespace *inside* them.
        let node = document
            .descendants()
            .find(|node| {
                significant_span(node).is_some_and(|span| span.end() < node.text_range().end())
            })
            .expect("some node ends in trailing trivia");

        let span = significant_span(&node).unwrap();
        let trailing = &source[usize::from(span.end())..usize::from(node.text_range().end())];
        assert!(!trailing.is_empty());
        assert!(trailing.chars().all(char::is_whitespace));
    }

    /// Rebuild the input text from a linearization; must always equal the
    /// document text (every token exactly once).
    fn reconstruct(linearization: &Linearization) -> String {
        let mut text = String::new();
        for slot in &linearization.slots {
            for trivia in &slot.gap_before {
                text += trivia.text();
            }
            text += slot.token.text();
        }
        for trivia in &linearization.trailing_trivia {
            text += trivia.text();
        }
        text
    }

    #[test]
    fn linearize_visits_every_token() {
        let sources = [
            "",
            "   \n",
            "component A { }",
            // Regression: the empty else-Expression after `{}` has no trivia
            // separating it from `foo`; next_token must scan past it instead
            // of ending the walk.
            "component A { function f() { if (true) {}foo(); } }",
            // Error-truncated trees must not lose text either.
            "export ",
            "component A {",
            "import { Foo }",
            // Regression: an Element whose leading QualifiedName is empty
            // (`inherits` with no type after it). rowan's shallow first_token
            // reported the Element as token-less, so next_token skipped the
            // whole component body — panic in debug, silent text loss in
            // release.
            "component Bar inherits {\n    if true : {}\n}",
            // Regression: a wildcard match case with no case before it yields
            // a match node whose first child is an empty Expression.
            "export component Baz {\n    match foo {\n        *: Rectangle { }\n    }\n}",
        ];
        for source in sources {
            let document = parse(source);
            let linearization = linearize(&document);
            assert_eq!(
                reconstruct(&linearization),
                document.text().to_string(),
                "lost tokens for source {source:?}"
            );
        }
    }

    #[test]
    fn annotate_attaches_atoms_at_token_boundaries() {
        let source = "component A { states [ s when c: { x: 1; } ] }";
        let document = parse(source);
        let linearization = linearize(&document);

        let mut rules = FormatRules::default();
        rules.token(SyntaxKind::Colon, |colon| {
            colon.prepend(Atom::Antispace).append(Atom::Space);
        });
        rules.node(SyntaxKind::States, |states| {
            states.keyword("states").append(Atom::Space);
            states.token(SyntaxKind::LBracket).append(states.spaced_softline());
            states.node(SyntaxKind::State).prepend(Atom::AllowBlankLines);
            states.token(SyntaxKind::RBracket).prepend(Atom::IndentEnd);
        });

        let sink = AtomSink::default();
        annotate(&document, &linearization.slots, &rules, &sink, source);
        let boundary_atoms = sink.finish().boundary;

        let token_start = |text: &str| {
            let offset = source.find(text).unwrap();
            i_slint_compiler::parser::TextSize::new(offset as u32)
        };
        let atoms_of = |instances: &[AtomInstance]| {
            instances.iter().map(|instance| instance.atom.clone()).collect::<Vec<_>>()
        };

        // `keyword("states")` matched the identifier, not e.g. the state `s`.
        assert_eq!(atoms_of(&boundary_atoms.after[&token_start("states")]), [Atom::Space]);
        // The softline constructed by `spaced_softline()` carries the
        // rule's node span as its measure span.
        let states_span = significant_span(&find_node(&document, SyntaxKind::States)).unwrap();
        assert_eq!(
            atoms_of(&boundary_atoms.after[&token_start("[")]),
            [Atom::SpacedSoftline(states_span)]
        );
        // The State's boundary is its first significant token, `s`.
        assert_eq!(
            atoms_of(&boundary_atoms.before[&token_start("s when")]),
            [Atom::AllowBlankLines]
        );
        assert_eq!(atoms_of(&boundary_atoms.before[&token_start("]")]), [Atom::IndentEnd]);

        // The global Colon rule fired on both colons, with the Token tier.
        for colon in [token_start(": {"), token_start(": 1")] {
            assert_eq!(atoms_of(&boundary_atoms.before[&colon]), [Atom::Antispace]);
            assert_eq!(atoms_of(&boundary_atoms.after[&colon]), [Atom::Space]);
            assert_eq!(boundary_atoms.before[&colon][0].tier, Tier::Token);
        }

        // Node rules carry the Node tier.
        assert_eq!(boundary_atoms.after[&token_start("[")][0].tier, Tier::Node);
    }

    fn colon_and_semicolon_rules() -> FormatRules {
        let mut rules = FormatRules::default();
        rules.token(SyntaxKind::Colon, |colon| {
            colon.prepend(Atom::Antispace).append(Atom::Space);
        });
        rules.token(SyntaxKind::Semicolon, |semicolon| {
            semicolon.prepend(Atom::Antispace);
        });
        rules
    }

    fn resolve_with_rules(source: &str, rules: &FormatRules) -> (Linearization, FormatPlan) {
        let document = parse(source);
        let linearization = linearize(&document);
        let sink = AtomSink::default();
        annotate(&document, &linearization.slots, rules, &sink, source);
        let plan = resolve(&linearization.slots, &sink.finish(), source);
        (linearization, plan)
    }

    fn slot_of(linearization: &Linearization, text: &str) -> usize {
        linearization.slots.iter().position(|slot| slot.token.text() == text).unwrap()
    }

    #[test]
    fn resolve_produces_gap_instructions() {
        let (linearization, plan) =
            resolve_with_rules("component A { x   :1; }", &colon_and_semicolon_rules());

        // Each slot yields one gap instruction and one token instruction.
        let colon_slot = slot_of(&linearization, ":");
        // `x   :` — the three spaces are deleted (Antispace)...
        assert_eq!(
            plan.instructions[2 * colon_slot],
            Instruction::ReplaceGap { slot: colon_slot, whitespace: Whitespace::None }
        );
        // ...and `:1` gains a space.
        assert_eq!(
            plan.instructions[2 * (colon_slot + 1)],
            Instruction::ReplaceGap { slot: colon_slot + 1, whitespace: Whitespace::Space }
        );
        // A gap with no rule atoms takes the default: nothing before the first
        // token...
        let component_slot = slot_of(&linearization, "component");
        assert_eq!(
            plan.instructions[2 * component_slot],
            Instruction::ReplaceGap { slot: component_slot, whitespace: Whitespace::None }
        );
        assert_eq!(
            plan.instructions[2 * component_slot + 1],
            Instruction::EmitToken { slot: component_slot }
        );
        // ...and a single space between two tokens (here `component` and `A`).
        let name_slot = slot_of(&linearization, "A");
        assert_eq!(
            plan.instructions[2 * name_slot],
            Instruction::ReplaceGap { slot: name_slot, whitespace: Whitespace::Space }
        );
    }

    #[test]
    fn higher_tier_rules_override_lower_tiers() {
        // The token-tier Colon rule wants `: ` around every colon; a
        // node-tier rule on the state disagrees on both sides. The node tier
        // must win even where its atoms resolve to *less* whitespace.
        let source = "component A { states [ s: { x: 1; } ] }";
        let mut rules = colon_and_semicolon_rules();
        rules.node(SyntaxKind::State, |state| {
            // Resolves to Nothing (the state is single-line): must beat the
            // token tier's Space after `:` even though Nothing is *weaker*.
            state.token(SyntaxKind::Colon).append(state.empty_softline());
        });
        let (linearization, plan) = resolve_with_rules(source, &rules);

        let state_colon_slot = slot_of(&linearization, ":");
        assert_eq!(
            plan.instructions[2 * (state_colon_slot + 1)],
            Instruction::ReplaceGap { slot: state_colon_slot + 1, whitespace: Whitespace::None },
            "node-tier EmptySoftline must override the token-tier Space"
        );
    }

    #[test]
    fn indentation_is_tracked_across_default_gaps() {
        // Only the states brackets produce newlines here; the Element rule
        // contributes indentation bookkeeping so those newlines land at the
        // right depth even though the element's own gaps only take the default
        // spacing.
        let source = "component A { states [\n s: { x: 1; }\n] }";
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::Element, |element| {
            element.token(SyntaxKind::LBrace).append(Atom::IndentStart);
            element.token(SyntaxKind::RBrace).prepend(Atom::IndentEnd);
        });
        rules.node(SyntaxKind::States, |states| {
            states
                .token(SyntaxKind::LBracket)
                .append(Atom::IndentStart)
                .append(states.spaced_softline());
            states
                .token(SyntaxKind::RBracket)
                .prepend(Atom::IndentEnd)
                .prepend(states.spaced_softline());
        });
        let (linearization, plan) = resolve_with_rules(source, &rules);

        // The gap after `[` breaks at level 2: inside the element and the
        // states brackets.
        let state_name_slot = slot_of(&linearization, "s");
        assert_eq!(
            plan.instructions[2 * state_name_slot],
            Instruction::ReplaceGap {
                slot: state_name_slot,
                whitespace: Whitespace::Newline { blank_line: false, indentation_level: 2 }
            }
        );
        // The `]` goes back to level 1.
        let bracket_slot = slot_of(&linearization, "]");
        assert_eq!(
            plan.instructions[2 * bracket_slot],
            Instruction::ReplaceGap {
                slot: bracket_slot,
                whitespace: Whitespace::Newline { blank_line: false, indentation_level: 1 }
            }
        );
    }

    /// The gap instructions emitted for `slot` (everything between the
    /// previous slot's EmitToken and this slot's EmitToken).
    fn gap_instructions(plan: &FormatPlan, slot: usize) -> Vec<Instruction> {
        let position_of = |wanted: Instruction| {
            plan.instructions.iter().position(|instruction| *instruction == wanted).unwrap()
        };
        let end = position_of(Instruction::EmitToken { slot });
        let start =
            if slot == 0 { 0 } else { position_of(Instruction::EmitToken { slot: slot - 1 }) + 1 };
        plan.instructions[start..end].to_vec()
    }

    #[test]
    fn comment_gaps_resolve_per_sub_gap() {
        let source = "component A { x /* a */ :/* b */1; }";
        let (linearization, plan) = resolve_with_rules(source, &colon_and_semicolon_rules());

        let colon_slot = slot_of(&linearization, ":");
        // Before the colon: the sub-gap before the hanging comment has no
        // atoms, so it takes the default single space; the colon's Antispace
        // deletes the space after the comment.
        assert_eq!(
            gap_instructions(&plan, colon_slot),
            [
                Instruction::ReplaceSubGap {
                    slot: colon_slot,
                    trivia_index: Some(0),
                    whitespace: Whitespace::Space
                },
                Instruction::EmitComment { slot: colon_slot, trivia_index: 1, column_shift: 0 },
                Instruction::ReplaceSubGap {
                    slot: colon_slot,
                    trivia_index: Some(2),
                    whitespace: Whitespace::None
                },
            ]
        );
        // After the colon: the appended Space lands in the empty sub-gap
        // before the comment (inserted); the sub-gap after it has no atoms
        // and takes the default space.
        let one_slot = colon_slot + 1;
        assert_eq!(
            gap_instructions(&plan, one_slot),
            [
                Instruction::ReplaceSubGap {
                    slot: one_slot,
                    trivia_index: None,
                    whitespace: Whitespace::Space
                },
                Instruction::EmitComment { slot: one_slot, trivia_index: 0, column_shift: 0 },
                Instruction::ReplaceSubGap {
                    slot: one_slot,
                    trivia_index: None,
                    whitespace: Whitespace::Space
                },
            ]
        );
    }

    #[test]
    fn comments_never_move_off_their_line() {
        // The colon's appended Space meets an own-line comment: the input
        // newline wins on both sides of the comment, and the column-0
        // comment keeps level 0.
        let source = "component A { x:\n// c\n1; }";
        let (linearization, plan) = resolve_with_rules(source, &colon_and_semicolon_rules());

        let one_slot = slot_of(&linearization, "1");
        let newline_level_0 = Whitespace::Newline { blank_line: false, indentation_level: 0 };
        assert_eq!(
            gap_instructions(&plan, one_slot),
            [
                Instruction::ReplaceSubGap {
                    slot: one_slot,
                    trivia_index: Some(0),
                    whitespace: newline_level_0
                },
                Instruction::EmitComment { slot: one_slot, trivia_index: 1, column_shift: 0 },
                Instruction::ReplaceSubGap {
                    slot: one_slot,
                    trivia_index: Some(2),
                    whitespace: newline_level_0
                },
            ]
        );
    }

    #[test]
    fn newline_appends_transfer_past_hanging_comments() {
        // A newline-strength append on `{` must land *after* the hanging
        // comment (R2) — a space-strength atom would have stayed before it.
        // The target sub-gap has no input newline, so the newline can only
        // come from the routed atom.
        let source = "component A { states [ s: { /* note */ c: 1; } ] }";
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::State, |state| {
            state.token(SyntaxKind::LBrace).append(Atom::Hardline);
        });
        let (linearization, plan) = resolve_with_rules(source, &rules);

        let content_slot = slot_of(&linearization, "c");
        assert_eq!(
            gap_instructions(&plan, content_slot),
            [
                Instruction::ReplaceSubGap {
                    slot: content_slot,
                    trivia_index: Some(0),
                    whitespace: Whitespace::Space
                },
                Instruction::EmitComment { slot: content_slot, trivia_index: 1, column_shift: 0 },
                Instruction::ReplaceSubGap {
                    slot: content_slot,
                    trivia_index: Some(2),
                    whitespace: Whitespace::Newline { blank_line: false, indentation_level: 0 }
                },
            ]
        );
    }

    #[test]
    fn own_line_comments_reindent_unless_at_column_zero() {
        // Same shape twice: an indented comment re-indents to the current
        // level (with its column shift recorded), a column-0 comment stays
        // at level 0.
        let mut rules = colon_and_semicolon_rules();
        rules.node(SyntaxKind::Element, |element| {
            element.token(SyntaxKind::LBrace).append(Atom::IndentStart);
            element.token(SyntaxKind::RBrace).prepend(Atom::IndentEnd);
        });

        let indented = "component A {\n    x:\n  // c\n    1;\n}";
        let (linearization, plan) = resolve_with_rules(indented, &rules);
        let one_slot = slot_of(&linearization, "1");
        assert_eq!(
            gap_instructions(&plan, one_slot)[..2],
            [
                Instruction::ReplaceSubGap {
                    slot: one_slot,
                    trivia_index: Some(0),
                    whitespace: Whitespace::Newline { blank_line: false, indentation_level: 1 }
                },
                // From column 2 to level 1 (column 4): shift +2.
                Instruction::EmitComment { slot: one_slot, trivia_index: 1, column_shift: 2 },
            ]
        );

        let column_zero = "component A {\n    x:\n// c\n    1;\n}";
        let (linearization, plan) = resolve_with_rules(column_zero, &rules);
        let one_slot = slot_of(&linearization, "1");
        assert_eq!(
            gap_instructions(&plan, one_slot)[..2],
            [
                Instruction::ReplaceSubGap {
                    slot: one_slot,
                    trivia_index: Some(0),
                    whitespace: Whitespace::Newline { blank_line: false, indentation_level: 0 }
                },
                Instruction::EmitComment { slot: one_slot, trivia_index: 1, column_shift: 0 },
            ]
        );
    }

    fn format_with(source: &str, rules: &FormatRules) -> String {
        let document = parse(source);
        let mut output = Vec::new();
        format_document_with_rules(
            &document,
            rules,
            &mut crate::fmt::writer::FileWriter { file: &mut output },
        )
        .unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn format_with_rules_end_to_end() {
        // The colon is re-spaced (including a space *inserted* where the
        // input had no whitespace token); the other boundaries were already a
        // single space, which the default reproduces.
        assert_eq!(
            format_with("component A { x   :1; }", &colon_and_semicolon_rules()),
            "component A { x: 1; }"
        );
    }

    #[test]
    fn document_edge_default_spares_comment_adjacency() {
        // The nothing-at-the-document-edge default is scoped to the
        // edge-touching sub-gap. A hanging comment before Eof and a leading
        // comment before the first token sit at real adjacencies inside the
        // edge gaps: they keep their single space.
        let rules = FormatRules::default();
        assert_eq!(format_with("component A { } // tail", &rules), "component A { } // tail");
        assert_eq!(
            format_with("/* header */ component A { }", &rules),
            "/* header */ component A { }"
        );
    }

    #[test]
    fn token_matching_selects_a_kind_set() {
        // One rule spaces every arithmetic operator of a BinaryExpression;
        // `token_matching` replaces four separate `token(kind)` calls.
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::BinaryExpression, |binary| {
            binary
                .token_matching(|kind| {
                    matches!(
                        kind,
                        SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::Star | SyntaxKind::Div
                    )
                })
                .prepend(Atom::Space)
                .append(Atom::Space);
        });
        // No colon/semicolon rule here, so `x :` and `;` take the default
        // single space; the operator spacing is the rule under test.
        assert_eq!(
            format_with("component A { x: 1+2 *3- 4; }", &rules),
            "component A { x : 1 + 2 * 3 - 4 ; }"
        );
    }

    #[test]
    fn children_iter_and_at_express_pairwise_rules() {
        // Statement boundaries in a code block: the direct children are
        // `{ Expression ; Expression ; }`, and the boundary of interest is
        // "a `;` directly followed by the next statement node" — inherently
        // positional, so the rule iterates the children in plain Rust and
        // re-enters the selection API with `at` on the right-hand item.
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::CodeBlock, |block| {
            let children: Vec<_> = block.children().iter().cloned().collect();
            for pair in children.windows(2) {
                if pair[0].kind() == SyntaxKind::Semicolon && pair[1].as_node().is_some() {
                    block.at(pair[1].clone()).prepend(block.spaced_softline());
                }
            }
        });
        // Single-line block: the softline resolves to a space. Everything
        // else takes the default single space (no other rule fires).
        assert_eq!(
            format_with("component A { function f() { a = 1;b = 2; } }", &rules),
            "component A { function f ( ) { a = 1 ; b = 2 ; } }"
        );
    }

    /// The one wildcard rule the design calls for: a space between two
    /// adjacent child *nodes* — pairs with a significant token between them
    /// are none of the wildcard's business (see [`FormatRules::any_node`]).
    fn adjacent_node_space_rules() -> FormatRules {
        let mut rules = FormatRules::default();
        rules.any_node(|node| {
            let children: Vec<_> = node.children().iter().cloned().collect();
            for pair in children.windows(2) {
                if pair[0].as_node().is_some() && pair[1].as_node().is_some() {
                    node.at(pair[1].clone()).prepend(Atom::Space);
                }
            }
        });
        rules
    }

    #[test]
    fn wildcard_rules_run_on_every_node() {
        // The wildcard rule spaces the two adjacent sub-element nodes; the
        // remaining boundaries take the default single space.
        assert_eq!(
            format_with("component A { Text{}Image{} }", &adjacent_node_space_rules()),
            "component A { Text { } Image { } }"
        );
    }

    #[test]
    fn node_tier_overrides_wildcard_tier() {
        let mut rules = adjacent_node_space_rules();
        rules.node(SyntaxKind::Element, |element| {
            // Resolves to Nothing on this single-line element; it must beat
            // the wildcard Space even though Nothing is *weaker*.
            element.node(SyntaxKind::SubElement).prepend(element.empty_softline());
        });
        // The prepend fires before *every* SubElement, so the boundary before
        // Text and before Image is Nothing (hence `{Text` and `}Image`, the
        // Node tier beating the wildcard Space). The braces inside each
        // sub-element take the default space.
        assert_eq!(
            format_with("component A { Text{}Image{} }", &rules),
            "component A {Text { }Image { } }"
        );
    }

    #[test]
    fn wildcard_tier_overrides_token_tier() {
        // The contract on `any_node` warns that a wildcard Space beats a
        // token Antispace at the same boundary — the reason wildcard rules
        // must never touch punctuation. Pin that ordering down: the wildcard
        // Space between the two sub-elements meets a token-tier Antispace
        // prepended before every identifier (which includes `Image`, the
        // second sub-element's first token). Assert only that boundary; the
        // Antispace's effect on the other identifiers is beside the point.
        let mut rules = adjacent_node_space_rules();
        rules.token(SyntaxKind::Identifier, |identifier| {
            identifier.prepend(Atom::Antispace);
        });
        let (linearization, plan) = resolve_with_rules("component A { Text{}Image{} }", &rules);

        let image_slot = slot_of(&linearization, "Image");
        assert_eq!(
            plan.instructions[2 * image_slot],
            Instruction::ReplaceGap { slot: image_slot, whitespace: Whitespace::Space },
            "the wildcard Space must beat the token-tier Antispace"
        );
    }

    #[test]
    fn input_softline_without_newline_is_advisory() {
        let mut rules = adjacent_node_space_rules();
        rules.node(SyntaxKind::Element, |element| {
            element.node(SyntaxKind::SubElement).prepend(Atom::InputSoftline);
        });
        // No input newline: the Node-tier InputSoftline stays silent, so the
        // Wildcard-tier Space wins (a tier-bearing Nothing would veto it).
        assert_eq!(
            format_with("component A { Text{}Image{} }", &rules),
            "component A { Text { } Image { } }"
        );
        // With an input newline it resolves to a newline and beats the Space.
        assert_eq!(
            format_with("component A { Text{}\nImage{} }", &rules),
            "component A { Text { }\nImage { } }"
        );
    }

    /// Element indentation + a multiline-aware States block, plus the global
    /// colon rule — the setup the leaf test formats around.
    fn states_indent_rules() -> FormatRules {
        let mut rules = colon_and_semicolon_rules();
        rules.node(SyntaxKind::Element, |element| {
            element.token(SyntaxKind::LBrace).append(Atom::IndentStart);
            element.token(SyntaxKind::RBrace).prepend(Atom::IndentEnd);
        });
        rules.node(SyntaxKind::States, |states| {
            states
                .token(SyntaxKind::LBracket)
                .append(Atom::IndentStart)
                .append(states.spaced_softline());
            states.node(SyntaxKind::State).prepend(states.spaced_softline());
            states
                .token(SyntaxKind::RBracket)
                .prepend(Atom::IndentEnd)
                .prepend(states.spaced_softline());
        });
        rules
    }

    #[test]
    fn leaf_keeps_interior_verbatim_while_surroundings_format() {
        // Leaf the whole State: the colons inside (`s1 :`, `x :1`) stay
        // exactly as written even though the global colon rule fires on
        // them, while the binding *outside* the leaf (`b :2`) is respaced and
        // the states block still normalizes around it.
        let mut rules = states_indent_rules();
        rules.node(SyntaxKind::State, |state| {
            state.leaf();
        });
        let source = "component A {
    b :2;
    states [
        s1 :{ x :1; }
    ]
}";
        // The element's own gaps take the default single space (no rule
        // breaks its children onto lines), while the states block normalizes
        // and the leafed State stays verbatim.
        assert_eq!(
            format_with(source, &rules),
            "component A { b: 2; states [
        s1 :{ x :1; }
    ] }"
        );

        // The `:1` gap (after the interior colon) is emitted verbatim — a
        // KeepGap, not the space-inserting ReplaceGap the colon rule
        // produces at the `b :2` colon outside the leaf.
        let (linearization, plan) = resolve_with_rules(source, &rules);
        let interior_value = slot_of(&linearization, "1");
        assert!(matches!(
            gap_instructions(&plan, interior_value).as_slice(),
            [Instruction::KeepGap { .. }]
        ));
        let exterior_value = slot_of(&linearization, "2");
        assert!(matches!(
            gap_instructions(&plan, exterior_value).as_slice(),
            [Instruction::ReplaceGap { whitespace: Whitespace::Space, .. }]
        ));
    }

    // A minimal function-call ruleset that manages the argument list's
    // trailing comma. Deliberately just this one rule so the demo shows the
    // comma behavior without indentation from the surrounding element.
    fn trailing_comma_rules() -> FormatRules {
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::FunctionCallExpression, |call| {
            let multiline = call.is_multiline();
            let children: Vec<NodeOrToken> = call.children().iter().cloned().collect();
            let Some(open) = children.iter().position(|child| child.kind() == SyntaxKind::LParent)
            else {
                return;
            };
            let Some(close) =
                children.iter().rposition(|child| child.kind() == SyntaxKind::RParent)
            else {
                return;
            };
            if close <= open + 1 {
                return; // `()` — no arguments.
            }
            call.at(children[open].clone()).append(Atom::IndentStart).append(call.empty_softline());
            call.at(children[close].clone())
                .prepend(Atom::IndentEnd)
                .prepend(call.empty_softline());

            let last = close - 1;
            let has_trailing_comma = children[last].kind() == SyntaxKind::Comma;
            for index in (open + 1)..close {
                if children[index].kind() != SyntaxKind::Comma {
                    continue;
                }
                call.at(children[index].clone()).prepend(Atom::Antispace);
                if index == last {
                    // The trailing comma is dropped when the call collapses
                    // onto one line, kept when it stays broken.
                    if !multiline {
                        call.at(children[index].clone()).delete();
                    }
                } else {
                    call.at(children[index].clone()).append(call.spaced_softline());
                }
            }
            if multiline && !has_trailing_comma {
                // A broken call gains the trailing comma it lacks.
                call.at(children[last].clone()).append(Atom::Literal(String::from(",")));
            }
        });
        rules
    }

    #[test]
    fn trailing_comma_managed_across_all_four_quadrants() {
        let rules = trailing_comma_rules();
        let check = |input: &str, expected: &str| {
            assert_eq!(format_with(input, &rules), expected);
            assert_eq!(format_with(expected, &rules), expected, "not idempotent");
        };
        // (Only the FunctionCallExpression rule fires; the boundaries around
        // it — `x :`, `test (`, `) ;` — take the default single space.)
        // Single-line, no trailing comma: arg list untouched.
        check("component A { x: test(a, b); }", "component A { x : test (a, b) ; }");
        // Single-line, trailing comma present: deleted.
        check("component A { x: test(a, b,); }", "component A { x : test (a, b) ; }");
        // Broken across lines, no trailing comma: one is inserted (the
        // `test(\n    a,\n    b,\n)` target from the design plan).
        check("component A { x: test(a,\nb); }", "component A { x : test (\n    a,\n    b,\n) ; }");
        // Broken across lines, trailing comma present: kept.
        check(
            "component A { x: test(a,\nb,); }",
            "component A { x : test (\n    a,\n    b,\n) ; }",
        );
    }

    #[test]
    fn literal_inside_a_leaf_is_suppressed() {
        // A rule leafs the call and also injects a literal after its comma (an
        // interior boundary). Leaf suppression must swallow the literal, the
        // same way it suppresses whitespace and deletes, leaving the call
        // verbatim.
        let mut leafed = FormatRules::default();
        leafed.node(SyntaxKind::FunctionCallExpression, |call| {
            call.leaf();
            call.token(SyntaxKind::Comma).append(Atom::Literal(String::from("!")));
        });
        // The call is leafed, so its interior (including the comma boundary
        // the rule targets) is verbatim; only the surrounding boundaries take
        // the default space.
        assert_eq!(
            format_with("component A { x: test(a, b); }", &leafed),
            "component A { x : test(a, b) ; }"
        );

        // Without the leaf the very same literal rule does fire — proving the
        // suppression above is real, not a rule that never triggered.
        let mut plain = FormatRules::default();
        plain.token(SyntaxKind::Comma, |comma| {
            comma.append(Atom::Literal(String::from("!")));
        });
        assert_eq!(
            format_with("component A { x: test(a, b); }", &plain),
            "component A { x : test ( a ,! b ) ; }"
        );
        // And an ignore directive, which leafs via the engine core, suppresses
        // it too (the whole binding stays verbatim).
        assert_eq!(
            format_with("component A {\n    // slint-fmt:ignore\n    x: test(a, b);\n}", &plain),
            "component A {\n// slint-fmt:ignore\nx: test(a, b); }"
        );
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "idempotency violation")]
    fn hardline_inside_a_single_line_softline_span_is_caught() {
        // The call is single-line, so `empty_softline()` measures its span as
        // single-line — yet a Hardline forces a newline strictly inside that
        // span. On the next run the span would be multiline, so the debug
        // guard must reject this ruleset.
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::FunctionCallExpression, |call| {
            call.token(SyntaxKind::LParent).append(call.empty_softline());
            call.token(SyntaxKind::Comma).append(Atom::Hardline);
        });
        format_with("component A { x: test(a, b); }", &rules);
    }

    #[test]
    fn delete_inside_a_leaf_is_ignored() {
        // The call is leafed (kept verbatim) and its commas are also marked
        // for deletion. The leaf wins, so the commas — including the trailing
        // one that would otherwise be dropped single-line — survive.
        let mut rules = FormatRules::default();
        rules.node(SyntaxKind::FunctionCallExpression, |call| {
            call.leaf();
            let children: Vec<NodeOrToken> = call.children().iter().cloned().collect();
            for child in &children {
                if child.kind() == SyntaxKind::Comma {
                    call.at(child.clone()).delete();
                }
            }
        });
        assert_eq!(
            format_with("component A { x: test(a, b,); }", &rules),
            "component A { x : test(a, b,) ; }"
        );
    }

    #[test]
    fn ignore_directive_targets_the_following_construct() {
        // The directive before the binding ignores the whole binding.
        let document = parse("component A { // slint-fmt:ignore\n x   :1; }");
        let binding = find_node(&document, SyntaxKind::Binding);
        let target = first_significant_token(&binding).unwrap();
        assert_eq!(ignored_span(&target), significant_span(&binding));
    }

    #[test]
    fn ignore_directive_caps_at_a_document_child() {
        // A top-level directive ignores only the next construct, never the
        // whole file.
        let source = "// slint-fmt:ignore\ncomponent A { }\ncomponent B { }";
        let document = parse(source);
        let target = first_significant_token(&document).unwrap();
        let span = ignored_span(&target).unwrap();
        assert_eq!(&source[span], "component A { }");
    }

    #[test]
    fn ignore_directive_before_a_closing_brace_is_a_no_op() {
        // No construct starts at `}`, so the directive has nothing to ignore.
        let document = parse("component A { x: 1; // slint-fmt:ignore\n}");
        let linearization = linearize(&document);
        let rbrace =
            &linearization.slots.iter().find(|slot| slot.token.text() == "}").unwrap().token;
        assert_eq!(ignored_span(rbrace), None);
    }

    #[test]
    fn ignore_directive_before_eof_marks_nothing() {
        // A trailing directive at the end of the file governs only Eof, which
        // ends no construct.
        let document = parse("component A { }\n// slint-fmt:ignore\n");
        let linearization = linearize(&document);
        let sink = AtomSink::default();
        apply_ignore_directives(&linearization.slots, &sink);
        assert!(sink.finish().markers.is_empty());
    }

    #[test]
    fn ignore_directive_marks_one_leaf() {
        let document = parse("component A {\n// slint-fmt:ignore\nx   :1;\n}");
        let linearization = linearize(&document);
        let sink = AtomSink::default();
        apply_ignore_directives(&linearization.slots, &sink);
        let markers = sink.finish().markers;
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].1, Marker::Leaf);
    }

    #[test]
    fn ignore_directive_tolerates_trailing_whitespace() {
        // A trailing space is part of the comment token but invisible; the
        // directive must still fire.
        let document = parse("component A {\n// slint-fmt:ignore  \nx   :1;\n}");
        let linearization = linearize(&document);
        let sink = AtomSink::default();
        apply_ignore_directives(&linearization.slots, &sink);
        assert_eq!(sink.finish().markers.len(), 1);
    }

    #[test]
    fn children_skips_trivia_and_token_less_nodes() {
        // `if (true) {}` without an `else` gets a fake, token-less
        // else-Expression as a direct child of the ConditionalExpression;
        // `children()` must skip it (and the comment trivia), leaving only
        // the `if` keyword, the condition, and the body.
        let document = parse("component A { function f() { if /* c */ (true) {} } }");
        let conditional = find_node(&document, SyntaxKind::ConditionalExpression);
        assert_eq!(conditional.children().count(), 3, "condition, body, fake else");
        let sink = AtomSink::default();
        let selection = Selection {
            items: vec![NodeOrToken::Node(conditional)],
            context: document.clone(),
            tier: Tier::Node,
            sink: &sink,
            source: "",
        };
        let kinds: Vec<SyntaxKind> = selection.children().iter().map(NodeOrToken::kind).collect();
        assert_eq!(kinds, [SyntaxKind::Identifier, SyntaxKind::Expression, SyntaxKind::Expression]);
    }

    #[test]
    fn significant_tokens_skip_empty_nodes() {
        // `if (true) {}` produces empty Expression/CodeBlock nodes for the
        // missing else branch; the helpers must skip them, not panic.
        let document = parse("component A { function f() { if (true) {} } }");
        let code_block = find_node(&document, SyntaxKind::CodeBlock);
        assert!(first_significant_token(&code_block).is_some());
        assert!(last_significant_token(&code_block).is_some());

        let empty = document
            .descendants()
            .find(|node| node.first_child_or_token().is_none())
            .expect("the parse contains an empty node");
        assert!(first_significant_token(&empty).is_none());
        assert!(last_significant_token(&empty).is_none());
        assert!(significant_span(&empty).is_none());

        // A rule firing on a token-less node still constructs valid
        // softlines: the empty measure span resolves as single-line.
        let sink = AtomSink::default();
        let selection = Selection {
            items: vec![NodeOrToken::Node(empty.clone())],
            context: empty,
            tier: Tier::Node,
            sink: &sink,
            source: "",
        };
        assert_eq!(
            selection.spaced_softline(),
            Atom::SpacedSoftline(TextRange::empty(TextSize::new(0)))
        );
        assert!(!selection.is_multiline());
    }
}
