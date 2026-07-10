// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The query-based formatter engine: linearization of the syntax tree into
//! token slots, rule dispatch, and resolution of atoms into a `FormatPlan`.
//!
//! See `API_DESIGN.md` in this directory for the overall design.

use crate::fmt::atoms::{
    Atom, AtomInstance, AtomSink, BoundaryAtoms, FormatPlan, Instruction, Tier, Whitespace,
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
    pub fn gap_newlines(&self) -> usize {
        self.gap_before
            .iter()
            .filter(|token| token.kind() == SyntaxKind::Whitespace)
            .map(|token| token.text().matches('\n').count())
            .sum()
    }

    /// Whether the gap contains at least one blank line (a whitespace token
    /// with two or more newlines).
    pub fn has_blank_line(&self) -> bool {
        self.gap_before.iter().any(|token| {
            token.kind() == SyntaxKind::Whitespace && token.text().matches('\n').count() >= 2
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
            // not by each caller's kind filter.
            .filter(|token| !is_trivia(token.kind()))
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
        debug_assert!(!is_trivia(kind), "rules must not select trivia");
        self.derived(
            self.child_tokens()
                .filter(|token| token.kind() == kind)
                .map(NodeOrToken::Token)
                .collect(),
        )
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

    /// Attach an atom to the boundary before each selected item (before a
    /// node's first significant token). Token-less items (e.g. empty
    /// `Expression` nodes) have no boundary and are skipped.
    pub fn prepend(&self, atom: Atom) -> &Self {
        for item in &self.items {
            let anchor = match item {
                NodeOrToken::Token(token) => Some(token.clone()),
                NodeOrToken::Node(node) => first_significant_token(node),
            };
            let Some(anchor) = anchor else { continue };
            self.sink
                .attach_before(anchor.text_range().start(), AtomInstance { atom, tier: self.tier });
        }
        self
    }

    /// Attach an atom to the boundary after each selected item (after a
    /// node's last significant token). Token-less items are skipped.
    pub fn append(&self, atom: Atom) -> &Self {
        for item in &self.items {
            let anchor = match item {
                NodeOrToken::Token(token) => Some(token.clone()),
                NodeOrToken::Node(node) => last_significant_token(node),
            };
            let Some(anchor) = anchor else { continue };
            self.sink
                .attach_after(anchor.text_range().start(), AtomInstance { atom, tier: self.tier });
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
        let Some(node_rules) = rules.node_rules.get(&node.kind()) else { continue };
        let selection = Selection {
            items: vec![NodeOrToken::Node(node.clone())],
            context: node.clone(),
            tier: Tier::Node,
            sink,
            source,
        };
        for rule in node_rules {
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
/// Gaps that no rule attached spacing to are kept verbatim — the formatter
/// is a no-op except where rules fire. Gaps containing comments are also
/// kept verbatim for now (the comment-aware sub-gap handling from
/// API_DESIGN.md is not implemented yet).
pub fn resolve(slots: &[TokenSlot], boundary_atoms: &BoundaryAtoms, source: &str) -> FormatPlan {
    let mut instructions = Vec::with_capacity(slots.len() * 2);
    let mut indentation_level: i32 = 0;

    for (slot_index, slot) in slots.iter().enumerate() {
        // The gap before this token is one physical location: the previous
        // token's appended atoms and this token's prepended atoms meet here.
        let mut gap_atoms: Vec<AtomInstance> = Vec::new();
        if let Some(previous_slot) = slot_index.checked_sub(1).map(|index| &slots[index]) {
            let previous_start = previous_slot.token.text_range().start();
            gap_atoms
                .extend(boundary_atoms.after.get(&previous_start).into_iter().flatten().copied());
        }
        let start = slot.token.text_range().start();
        gap_atoms.extend(boundary_atoms.before.get(&start).into_iter().flatten().copied());

        // Indentation is bookkeeping, not a whitespace decision: sum it even
        // for gaps that are kept verbatim, so that rules firing inside
        // otherwise unformatted code still see correct levels.
        for instance in &gap_atoms {
            match instance.atom {
                Atom::IndentStart => indentation_level += 1,
                Atom::IndentEnd => indentation_level -= 1,
                _ => {}
            }
        }

        // Keep in sync with the `continue` arms in `resolve_gap_whitespace`:
        // these are the atoms that make no whitespace decision on their own.
        // (Antispace counts as a decision — an Antispace-only gap collapses
        // to nothing; an AllowBlankLines-only gap must stay verbatim.)
        let has_spacing_atom = gap_atoms.iter().any(|instance| {
            !matches!(instance.atom, Atom::IndentStart | Atom::IndentEnd | Atom::AllowBlankLines)
        });
        if slot.has_comment() || !has_spacing_atom {
            instructions.push(Instruction::KeepGap { slot: slot_index });
        } else {
            let whitespace = resolve_gap_whitespace(&gap_atoms, slot, source, indentation_level);
            instructions.push(Instruction::ReplaceGap { slot: slot_index, whitespace });
        }
        instructions.push(Instruction::EmitToken { slot: slot_index });
    }

    FormatPlan { instructions }
}

/// Resolve one gap's atoms into a whitespace decision.
fn resolve_gap_whitespace(
    gap_atoms: &[AtomInstance],
    slot: &TokenSlot,
    source: &str,
    indentation_level: i32,
) -> Whitespace {
    let measures_multiline = |measure_span: TextRange| source[measure_span].contains('\n');

    // Each atom resolves to a strength on its own (softlines against their
    // measure span, which may differ between instances). The decision is the
    // lexicographic (tier, strength) maximum: the highest tier that
    // contributed anything wins, and the strongest atom within it decides.
    let mut decision: Option<(Tier, Strength)> = None;
    let mut antispace_tier: Option<Tier> = None;
    for instance in gap_atoms {
        let strength = match instance.atom {
            Atom::Space => Strength::Space,
            Atom::Hardline => Strength::Newline,
            Atom::SpacedSoftline(measure_span) => {
                if measures_multiline(measure_span) { Strength::Newline } else { Strength::Space }
            }
            Atom::EmptySoftline(measure_span) => {
                if measures_multiline(measure_span) {
                    Strength::Newline
                } else {
                    Strength::Nothing
                }
            }
            Atom::InputSoftline => {
                if slot.gap_newlines() > 0 { Strength::Newline } else { Strength::Nothing }
            }
            Atom::Antispace => {
                antispace_tier = antispace_tier.max(Some(instance.tier));
                continue;
            }
            Atom::IndentStart | Atom::IndentEnd | Atom::AllowBlankLines => continue,
        };
        decision = decision.max(Some((instance.tier, strength)));
    }

    let strength = match decision {
        // Only Antispace atoms contributed: nothing between the tokens.
        None => Strength::Nothing,
        // Antispace cancels a Space decision of its own or a lower tier —
        // never a newline.
        Some((tier, Strength::Space)) if antispace_tier >= Some(tier) => Strength::Nothing,
        Some((_, strength)) => strength,
    };

    match strength {
        Strength::Nothing => Whitespace::None,
        Strength::Space => Whitespace::Space,
        Strength::Newline => Whitespace::Newline {
            blank_line: slot.has_blank_line()
                && gap_atoms.iter().any(|instance| instance.atom == Atom::AllowBlankLines),
            indentation_level: indentation_level.max(0) as u32,
        },
    }
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

        let x_slot = linearization
            .slots
            .iter()
            .find(|slot| slot.token.text() == "x")
            .expect("slot for `x`");
        assert_eq!(x_slot.gap_newlines(), 2);
        assert!(x_slot.has_blank_line());
        assert!(!x_slot.has_comment());
        assert_eq!(x_slot.single_whitespace_token().unwrap().text(), "\n\n    ");
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
        let boundary_atoms = sink.finish();

        let token_start = |text: &str| {
            let offset = source.find(text).unwrap();
            i_slint_compiler::parser::TextSize::new(offset as u32)
        };
        let atoms_of = |instances: &[AtomInstance]| {
            instances.iter().map(|instance| instance.atom).collect::<Vec<_>>()
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
        // Gaps without rule atoms are kept verbatim.
        let component_slot = slot_of(&linearization, "component");
        assert_eq!(
            plan.instructions[2 * component_slot],
            Instruction::KeepGap { slot: component_slot }
        );
        assert_eq!(
            plan.instructions[2 * component_slot + 1],
            Instruction::EmitToken { slot: component_slot }
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
    fn indentation_is_tracked_across_kept_gaps() {
        // Only the states brackets produce newlines here; the Element rule
        // contributes indentation bookkeeping so those newlines land at the
        // right depth even though the element's own gaps are kept verbatim.
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

    #[test]
    fn format_with_rules_end_to_end() {
        let document = parse("component A { x   :1; }");
        let mut output = Vec::new();
        format_document_with_rules(
            &document,
            &colon_and_semicolon_rules(),
            &mut crate::fmt::writer::FileWriter { file: &mut output },
        )
        .unwrap();
        // The colon is re-spaced (including a space *inserted* where the
        // input had no whitespace token); everything else is untouched.
        assert_eq!(String::from_utf8(output).unwrap(), "component A { x: 1; }");
    }

    #[test]
    fn significant_tokens_skip_empty_nodes() {
        // `if (true) {}` produces empty Expression/CodeBlock nodes for the
        // missing else branch; the helpers must skip them, not panic.
        let document =
            parse("component A { function f() { if (true) {} } }");
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
