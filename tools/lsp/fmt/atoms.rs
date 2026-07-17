// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Vocabulary types for the query-based formatter engine.
//!
//! The engine works in three phases (see `API_DESIGN.md` in this directory):
//! rules *annotate* token boundaries with [`Atom`]s collected in an
//! [`AtomSink`], the resolver collapses them into a [`FormatPlan`], and the
//! renderer realizes that plan through a `TokenWriter`.

use i_slint_compiler::parser::{TextRange, TextSize};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// A formatting annotation attached to the boundary before or after a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// One space.
    Space,
    /// Suppress any space-level atom at this boundary.
    Antispace,
    /// Always a newline.
    Hardline,
    /// Newline if the given input span was multiline, else a space.
    ///
    /// The span is usually the significant-token span of the rule's node —
    /// `Selection::spaced_softline` constructs that. An empty span resolves
    /// as single-line.
    SpacedSoftline(TextRange),
    /// Newline if the given input span was multiline, else nothing.
    /// Span semantics as for [`Atom::SpacedSoftline`].
    EmptySoftline(TextRange),
    /// Newline if the input had a newline at this boundary; otherwise the
    /// atom abstains entirely — unlike an [`Atom::EmptySoftline`] resolving
    /// to nothing, it makes no tier-bearing decision that could veto
    /// weaker-tier atoms (a lower-tier `Space` at the same boundary still
    /// wins).
    InputSoftline,
    /// Increase the indentation level for following newlines.
    IndentStart,
    /// Decrease the indentation level for following newlines.
    IndentEnd,
    /// Preserve one blank line from the input at this boundary.
    AllowBlankLines,
    /// Emit fixed text at this boundary — an append-literal right after the
    /// left token, a prepend-literal right before the right token. Unlike the
    /// spacing atoms it makes no whitespace decision (the gap's whitespace is
    /// resolved independently) but it does engage the gap. Used to inject a
    /// trailing comma into a list that broke across lines.
    // Not used by the shipped ruleset yet; exercised by the engine tests.
    #[allow(dead_code)]
    Literal(String),
}

/// An annotation that applies to a whole item (a node's range or a single
/// token) rather than to a boundary. See "Markers" in API_DESIGN.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// Emit every token in the item's range verbatim; suppress all boundary
    /// processing inside. Only meaningful on nodes.
    Leaf,
    /// Do not emit the item at all.
    Delete,
}

/// The rule tier an atom originates from. When whitespace decisions conflict
/// at the same boundary, a higher tier overrides lower tiers.
///
/// Variant order defines priority — later variants override earlier ones
/// (via the derived `Ord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Global rules keyed by token kind.
    Token,
    /// Wildcard rules that run for every node.
    Wildcard,
    /// Rules registered for a specific node kind.
    Node,
}

/// One atom as attached by a rule, with the tier it originates from.
#[derive(Debug, Clone)]
pub struct AtomInstance {
    pub atom: Atom,
    pub tier: Tier,
}

/// Collects the annotations produced by the rules during the annotate phase.
///
/// The interior mutability lets a rule hold several `Selection`s at once
/// while all of them attach atoms or markers; it stays confined to the
/// annotate phase, after which [`AtomSink::finish`] hands the plain
/// [`Annotations`] to the resolver.
#[derive(Default)]
pub struct AtomSink(RefCell<Annotations>);

/// Everything the rules produce during the annotate phase: boundary atoms and
/// item markers.
#[derive(Default)]
pub struct Annotations {
    pub boundary: BoundaryAtoms,
    /// Item markers in the order the rules produced them (unsorted, possibly
    /// overlapping across rule passes; the resolver normalizes them).
    pub markers: Vec<(TextRange, Marker)>,
}

/// The collected atoms, keyed by token offset: `before` holds atoms
/// prepended to the token starting at the key, `after` holds atoms appended
/// to it. Offsets are unique within one syntax tree; `BTreeMap` keeps debug
/// output deterministic.
#[derive(Default)]
pub struct BoundaryAtoms {
    pub before: BTreeMap<TextSize, Vec<AtomInstance>>,
    pub after: BTreeMap<TextSize, Vec<AtomInstance>>,
}

impl AtomSink {
    /// Attach an atom to the boundary before the token starting at `anchor`.
    pub fn attach_before(&self, anchor: TextSize, instance: AtomInstance) {
        self.0.borrow_mut().boundary.before.entry(anchor).or_default().push(instance);
    }

    /// Attach an atom to the boundary after the token starting at `anchor`.
    pub fn attach_after(&self, anchor: TextSize, instance: AtomInstance) {
        self.0.borrow_mut().boundary.after.entry(anchor).or_default().push(instance);
    }

    /// Mark a whole item range (see [`Marker`]).
    pub fn mark(&self, range: TextRange, marker: Marker) {
        self.0.borrow_mut().markers.push((range, marker));
    }

    /// Consume the sink at the end of the annotate phase.
    pub fn finish(self) -> Annotations {
        self.0.into_inner()
    }
}

/// The resolved whitespace for one gap between two significant tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    None,
    Space,
    Newline {
        /// Emit one blank line (i.e. a second newline) before the indented
        /// line.
        blank_line: bool,
        /// Indentation level after the newline.
        indentation_level: u32,
    },
}

/// The output of the resolve phase: concrete formatting instructions.
///
/// This is pure data — no strings are built and no `TokenWriter` is involved,
/// so resolution can be unit-tested by asserting on instruction sequences.
#[derive(Debug, PartialEq, Eq)]
pub struct FormatPlan {
    pub instructions: Vec<Instruction>,
}

/// The indentation unit: [`Whitespace::Newline`]'s `indentation_level`
/// counts these.
pub const INDENT: &str = "    ";

/// One formatting instruction. `slot` is an index into the `Vec<TokenSlot>`
/// produced by linearization; the renderer receives the slots alongside the
/// plan. `trivia_index` indexes into that slot's `gap_before`.
///
/// A gap containing comments is emitted as a sequence of sub-gap
/// instructions in trivia order: sub-gap, comment, sub-gap, comment, …,
/// sub-gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    /// Emit the gap's input trivia unchanged. Produced for gaps inside a
    /// [`Marker::Leaf`] range (whose interior is emitted verbatim) and for the
    /// comment-bearing gap before a deleted token (the comment is preserved).
    KeepGap { slot: usize },
    /// Replace the (comment-free) gap's input trivia with the given
    /// whitespace.
    ReplaceGap { slot: usize, whitespace: Whitespace },
    /// Replace one whitespace trivia token — or insert whitespace where the
    /// sub-gap is empty (`trivia_index: None`).
    ReplaceSubGap { slot: usize, trivia_index: Option<usize>, whitespace: Whitespace },
    /// Emit a comment token. A re-indented multiline block comment shifts
    /// each continuation line's leading whitespace by `column_shift`
    /// (clamped at zero), preserving the comment's internal alignment.
    EmitComment { slot: usize, trivia_index: usize, column_shift: i32 },
    /// Emit fixed text produced by an [`Atom::Literal`] (not backed by any
    /// input token).
    EmitLiteral { text: String },
    /// Emit the slot's significant token unchanged.
    EmitToken { slot: usize },
    /// Emit the slot's significant token as nothing (a deleted token). The
    /// token still passes the writer once, so the write protocol holds.
    DeleteToken { slot: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_preserves_insertion_order_per_boundary() {
        let sink = AtomSink::default();
        let anchor = TextSize::new(7);
        sink.attach_before(anchor, AtomInstance { atom: Atom::Antispace, tier: Tier::Token });
        sink.attach_before(anchor, AtomInstance { atom: Atom::Hardline, tier: Tier::Node });

        let annotations = sink.finish();
        let atoms: Vec<_> = annotations.boundary.before[&anchor]
            .iter()
            .map(|instance| instance.atom.clone())
            .collect();
        assert_eq!(atoms, [Atom::Antispace, Atom::Hardline]);
        assert!(annotations.boundary.after.is_empty());
        assert!(annotations.markers.is_empty());
    }
}
