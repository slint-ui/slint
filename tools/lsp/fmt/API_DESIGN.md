# Query-Based Formatter — API Design

Status: **implemented** — the engine and ruleset live in this directory,
and the old imperative formatter they replace is deleted.

This document describes the architecture and API for rewriting the Slint
formatter (the old `tools/lsp/fmt/fmt.rs`) as a rule/atom engine in the style of
[Topiary](https://topiary.tweag.io/), but operating on Slint's own rowan-based
syntax tree (`internal/compiler/parser.rs`) instead of a tree-sitter grammar.

Background: the Topiary experiment is documented in issue #11799 and PR #11825.
Its query file (`slint.scm`) is the reference ruleset this design must be able
to express. The conclusions from that experiment were: the query/atom *model*
is a very effective way to write formatting rules, but we do not want to depend
on an external formatting crate, and we do not want to format based on the
tree-sitter grammar — the rowan grammar in `internal/compiler/parser.rs` is the
source of truth for Slint syntax.

## Design principles

1. **Lean into Rust and rowan.** Tree-sitter queries need a predicate DSL
   (`#single_line_only!`, `#scope_id!`, …) because queries have no control
   flow. Our rules are Rust closures, so anything conditional or unusual is
   plain Rust over plain rowan nodes. The API surface stays minimal; rowan's
   own navigation (`children()`, `first_child()`, `parent()`, …) is *not*
   mirrored — rules escape into rowan and re-enter the annotation world.
2. **Declarative rules for the 90% case.** The common shape — "space after
   this token, indent inside these brackets, one item per line" — must read
   as tersely as the equivalent `.scm` query.
3. **Typed distinctions.** Boundary-scoped concepts (spacing between tokens)
   and item-scoped concepts (render this subtree verbatim, delete this token)
   are different types, so nonsensical combinations are unrepresentable.

## Architecture: three-phase pipeline

```text
source ──parse (existing parser.rs)──▶ rowan tree
       ──Phase 1: annotate──▶ atoms attached to token boundaries (+ item markers)
       ──Phase 2: resolve──▶ FormatPlan: concrete formatting instructions
       ──Phase 3: render──▶ TokenWriter (existing trait, unchanged)
```

### Phase 1 — annotate

One walk of the tree. For each node, run the rules registered for its
`SyntaxKind` (plus the wildcard and global-token tiers, see below). Rules
attach `Atom`s *before* or *after* children. Internally every atom lands on a
*significant-token boundary*: "before node X" means "before X's first
significant (non-trivia) token". Atoms are stored in `prepend`/`append` maps
keyed by the token's text offset (`text_range().start()` — unique per token in
one immutable tree; the only zero-length token is the trailing `Eof`, which
passes through the renderer harmlessly).
Markers are stored for whole items (rowan nodes or tokens), not boundaries.
The markers are "Leaf" (the node should be emitted as-is without any
formatting changes) and "Delete" (the item should not be emitted at all).

### Linearization (engine core, between phases 1 and 2)

One linear walk over the token stream — the only place the trivia walk
happens — produces the resolver's input:

```rust
struct TokenSlot {
    /// The ordered trivia (Whitespace/Comment tokens) preceding `token`,
    /// exactly as in the input. Within a gap, trivia has the shape
    /// `ws? (comment ws?)*`; the ordering between whitespace and comments is
    /// significant and preserved. Facts such as the newline count or
    /// blank-line presence are derived on demand (`newlines()`,
    /// `has_blank_line()`), never stored redundantly.
    gap_before: Vec<SyntaxToken>,
    /// A significant token.
    token: SyntaxToken,
}
```

The document is a `Vec<TokenSlot>`. Leading file trivia is slot 0's gap; the
tree's explicit `Eof` token means trailing trivia needs no special case (it is
the Eof slot's `gap_before`). Because a gap holds the original tokens, `Leaf`
ranges can be reconstructed byte-exactly.

### Phase 2 — resolve

Phase 2 collapses the annotations into concrete formatting instructions.
Inputs: the `Vec<TokenSlot>`, the prepend/append atom maps, and the marker
ranges. Output: a `FormatPlan` (below) — pure data, no strings built, no
`TokenWriter` involved, so resolution is unit-testable by asserting on
instruction sequences, and a debug dump of the plan is trivial.

Although the rule API speaks in prepend/append, physically there is only one
location: the **gap** between two adjacent significant tokens. Append-atoms of
the left token and prepend-atoms of the right token meet in that gap and
resolve together.

Softline atoms carry a *measure span* by value (usually the significant-token
span of the node whose rule created them, as constructed by
`Selection::spaced_softline()` / `empty_softline()`; a rule can construct
`Atom::SpacedSoftline(range)` with any other span).

Per gap, in one linear pass over the slots:

1. Gaps inside a `Leaf` range are skipped — their trivia passes through
   verbatim.
2. `Delete`d tokens emit nothing (they still pass the writer once, as empty
   content) and their gap's whitespace collapses. The two surrounding gaps
   *merge* without any physical restructuring: the deleted token's own atoms
   are discarded, and the next surviving gap sources its append-side atoms
   from the last *emitted* token rather than the immediate predecessor. So the
   trailing-comma pattern splits across two boundaries — `delete()` the comma
   token (single-line), or append a `Literal(",")` to the last argument
   (multiline); the injected literal must live on a surviving token, never on
   the deleted one, since the deleted token's atoms are dropped. The
   surrounding spacing still comes out right. (A comment inside a deleted
   token's gap is kept verbatim rather than collapsed.)
3. The gap is split at comments into sub-gaps, and atoms are anchored to
   sub-gaps (see "Comments split gaps" below). A comment-free gap is the
   degenerate single-sub-gap case.
4. Softlines resolve to plain strengths (`None < Space < Newline`):
   `SpacedSoftline`/`EmptySoftline` via their measure span's *input*
   multilineness, `InputSoftline` via the sub-gap's input newlines.
5. The whitespace channel merges lexicographically by **(tier, strength)**:
   the highest rule tier that contributed any whitespace atom wins the
   decision; within that tier the strongest atom wins; duplicates collapse.
   `Antispace` cancels `Space`-strength results of its own or lower tiers —
   it never cancels a newline. (Tier-first means a node rule's `Space` can
   deliberately override a global rule's resolved newline.) A boundary no
   atom decided takes the **default**: a single space between two tokens, and
   nothing at the document edges (before the first token and before the
   terminating Eof, where there is no adjacency to space). In an edge gap
   with comments, only the edge-touching sub-gap is the edge; the sub-gaps
   next to the comments have real adjacency and default to a space. The formatter
   therefore reformats every gap — there is no keep-the-input fallback — so
   running it over a file shows exactly what the ruleset does and does not yet
   cover.
6. Blank-line upgrade: if the decision is `Newline`, an `AllowBlankLines`
   atom is present, and the input sub-gap had a blank line, upgrade to one
   blank line (this caps preserved blank lines at one).
7. `IndentStart`/`IndentEnd` never conflict — they sum. A running counter
   over the pass yields the *indent level*, recorded with every newline
   decision (a level, not spaces: width/style is phase-3 config, and
   block-comment re-indentation needs the level too).
8. Debug-assert the idempotency constraint: no `Hardline` inside a softline
   measure span that resolved single-line.

#### Comments split gaps into sub-gaps

Ordering between whitespace and comments matters:

```slint
property <int> x; // comment for x   ← must stay on x's line
property <int> y;

property <int> x;
// comment for y                     ← must stay on its own line
property <int> y;
```

A gap containing comments is split into sub-gaps S0..Sn around them, and
atoms never merge across a comment. (This mirrors Topiary, where leaf atoms
in the linear atom stream are inherent boundaries.) Anchoring rules:

- **R1**: a comment-adjacent boundary whose input whitespace contains a
  newline always resolves to a newline, with an input blank line preserved
  (capped at one) — no rule atom needed. A comment-adjacent boundary with no
  newline and no routed atoms takes the default (a single space), like any
  other boundary.
- **R2**: atoms route by their *resolved* strength. Newline-strength append
  atoms of the left token transfer past the *trailing* comments (those with
  no input newline before them); everything else (`Space`, `Antispace`, and
  atoms that resolved to nothing) stays at the left boundary. So `{ // note`
  keeps the comment hanging and the `{`'s softline-newline lands after it,
  while `foo( /* c */ x)` keeps the `(`'s Antispace before the comment.
  Appends' `InputSoftline` resolves against the whitespace just after the
  trailing comments; prepends' against the last sub-gap. Indentation and
  `AllowBlankLines` bookkeeping travels with the newline side.
- **R3**: prepend atoms of the right token anchor at the *last* sub-gap. A
  useful consequence: a comment before `}` sits before the `}`'s `IndentEnd`
  anchor, so own-line comments there automatically render at the *inner*
  indent level — the classic mis-indent bug is structurally impossible.

Decided comment behaviors (with their trade-offs):

- **Comments never move lines**: rules re-indent comments but never pull one
  onto another line or push one off it — a boundary that had a newline in
  the input keeps it, whatever the rules say (this is R1's unconditional
  newline; it also means a rule's `Space` is dropped at such a boundary).
- Horizontal whitespace before a hanging comment takes the default single
  space, like any other undecided boundary (a rule can still widen it).
- Own-line comments are **re-indented to the current indent level** (matches
  Topiary; diverges from the current formatter, corpus diffs accepted) —
  **except comments starting at column 0**, which keep indentation level 0:
  the compiler's syntax tests use column-0 comments like `//   ^error{…}`
  whose internal spacing points at columns on the line above. At document
  start, an empty leading sub-gap counts as column 0.
- A re-indented multiline block comment shifts each continuation line's
  leading whitespace by the same column delta (clamped at zero), preserving
  the comment's internal alignment.
- `foo( /* c */ x)` rendering as `(/* c */ x` (Antispace in the first
  sub-gap) is accepted for now — matches the Topiary reference; revisit
  after the corpus run.
- `{ // note` keeps the comment hanging (matches the current formatter;
  Topiary would move it to the next line).
- `AllowBlankLines` is sub-gap-local: it protects the boundary it routes to,
  and comment-adjacent boundaries protect themselves.

#### Output: `FormatPlan`

Instructions reference the linearization (which travels alongside the plan)
by index instead of carrying tokens: `slot` indexes the token slots,
`trivia_index` a slot's `gap_before`. A gap containing comments expands into
a sub-gap instruction sequence in trivia order: S0, C1, S1, …, Cn, Sn.

```rust
struct FormatPlan { instructions: Vec<Instruction> }

enum Whitespace { None, Space, Newline { blank_line: bool, indentation_level: u32 } }

enum Instruction {
    /// Emit the whole gap's trivia unchanged. Produced for gaps inside a
    /// `Leaf` range (interior emitted verbatim) and for the comment-bearing
    /// gap before a deleted token (the comment is preserved).
    KeepGap { slot: usize },
    /// Replace the (comment-free) gap's trivia with the given whitespace.
    ReplaceGap { slot: usize, whitespace: Whitespace },
    /// Replace one whitespace trivia token — or insert whitespace where the
    /// sub-gap is empty.
    ReplaceSubGap { slot: usize, trivia_index: Option<usize>, whitespace: Whitespace },
    /// Emit a comment; a re-indented multiline block comment shifts its
    /// continuation lines by `column_shift`.
    EmitComment { slot: usize, trivia_index: usize, column_shift: i32 },
    /// Emit fixed text from an `Atom::Literal` (not backed by any input token).
    EmitLiteral { text: String },
    /// Emit the slot's significant token unchanged.
    EmitToken { slot: usize },
    /// Emit the slot's significant token as nothing (a `Delete`d token). The
    /// token still passes the writer once, so the write protocol holds.
    DeleteToken { slot: usize },
}
```

A `Leaf` range needs no new instruction — its interior gaps become the
existing slot-granular `KeepGap`/`EmitToken` verbatim sequence.

### Phase 3 — render

Realize the `FormatPlan` through the existing `TokenWriter` trait
(`tools/lsp/fmt/writer.rs`). Deliberately almost nothing happens here:
`Whitespace` decisions become strings (the indent unit lives here), and the
writer protocol is honored — every original token, trivia included, passes
through the writer exactly once:

- `EmitToken`: `no_change`
- `KeepGap`: `no_change` per trivia token of the gap (leaf interiors only)
- `ReplaceGap`, `ReplaceSubGap`: `with_new_content` on the whitespace token
  (possibly `""`), or `insert_content` where the input had none
- `EmitComment`: `no_change`, or `with_new_content` when continuation lines
  shift
- `EmitLiteral`: `insert_content`; `DeleteToken`: `with_new_content("")`

Keeping `TokenWriter` preserves both existing sinks unchanged: the CLI tool
(`fmt/tool.rs`, including `.rs`/`.md` embedded-source handling and the
append-unparsed-tail fallback on syntax errors) and the LSP
(`language/formatting.rs`, which renders to a `String` and derives `TextEdit`s
via `dissimilar::diff`). The engine must keep working on error-truncated trees
without panicking.

## Atoms (boundary-scoped)

```rust
pub enum Atom {
    /// One space.
    Space,
    /// Suppress any space-level atom at this boundary.
    Antispace,
    /// Always a newline. See idempotency constraint below.
    Hardline,
    /// Newline if the given input span was multiline, else a space.
    SpacedSoftline(TextRange),
    /// Newline if the given input span was multiline, else nothing.
    EmptySoftline(TextRange),
    /// Newline iff the input had a newline at this boundary.
    InputSoftline,
    /// Push / pop one indent level (applies to newlines emitted in between).
    IndentStart,
    IndentEnd,
    /// Preserve up to one input blank line before this boundary.
    AllowBlankLines,
    /// Emit fixed text at this boundary — an append-literal right after the
    /// left token, a prepend-literal right before the right token. It makes no
    /// whitespace decision (the gap's whitespace resolves independently) but
    /// engages the gap. Together with the `Delete` marker this manages a
    /// list's trailing comma: append `Literal(",")` to the last item when the
    /// list breaks across lines, `delete()` the comma when it collapses onto
    /// one line. An owned `String` (not `&'static str`) so conditionally
    /// built text can be injected.
    Literal(String),
}
```

The measured softlines carry their *measure span* by value, so the span is
enforced exactly on the atoms that need one. `Selection::spaced_softline()` /
`empty_softline()` construct them measured on the rule's node — the common
case — while a rule can construct `Atom::SpacedSoftline(range)` with any
other span directly. That explicit-span form replaces Topiary's `#scope_id!`
measuring scopes and is required in practice: `animate x, y { … }` has no body
node in the rowan grammar — the braces and commas are bare tokens inside
`PropertyAnimation` — so the rule computes the target-list span itself and
measures the comma softlines against it.

## Markers (item-scoped)

Some concepts apply to a whole item — a node's entire range, or a single
token — rather than to a boundary. Internally the annotation sink has **two
stores**: the boundary atom maps, and a set of `(range, Marker)` entries
consumed by the renderer:

```rust
/// Internal — not part of the public rule API.
enum Marker {
    /// Render every token in this range verbatim; suppress all boundary
    /// processing inside. Only meaningful on nodes.
    Leaf,
    /// Do not emit the item at all. In practice used on single tokens
    /// (a node marked `Delete` drops its whole significant range).
    Delete,
    // future candidates: preserve-inner-blank-lines, …
}
```

The marker enum is deliberately internal: it expresses the extensible concept
in the engine, while the public API exposes each marker as a dedicated method
on `Selection`:

```rust
selection.leaf();     // AtRustAttr; engine core uses it for `// slint-fmt:ignore`
selection.delete();   // canonicalization, e.g. Topiary's delete+delimiter trick
```

## Rule registration — three tiers

```rust
let mut rules = FormatRules::new();

// Tier 1 (lowest priority): global token rules, keyed by token SyntaxKind.
// Replaces the .scm's bare `":" @prepend_antispace @append_space` etc.
rules.token(SyntaxKind::Colon,     |t| { t.prepend(Antispace).append(Space); });
rules.token(SyntaxKind::Comma,     |t| { t.prepend(Antispace).append(t.spaced_softline()); });
rules.token(SyntaxKind::Semicolon, |t| { t.prepend(Antispace); });
rules.token(SyntaxKind::LParent,   |t| { t.append(Antispace); });

// Tier 2: wildcard node rule — the universal "adjacent child nodes get a
// separating space" fallback (`(_ (_) @append_space . (_))` in the .scm).
// CONTRACT: a wildcard rule must only attach whitespace between two child
// *nodes* with no significant token between them. The Wildcard tier sits
// ABOVE Token, so a wildcard `Space` next to a punctuation token would beat
// the global punctuation `Antispace` and re-space every `:`/`;`/`,`.
rules.any_node(|node| { /* … */ });

// Tier 3 (highest priority): per-kind node rules. The workhorse.
rules.node(SyntaxKind::States, |states: &Selection| { /* … */ });
```

Rules run per node *instance* during the tree walk — there is no global
pattern-matching step as in tree-sitter. Note that keywords such as `states`,
`when`, `animate`, `in`, `out`, `global`, `inherits` are plain `Identifier`
tokens in the rowan grammar; they are matched by text with `keyword(...)`,
which is only available on node rules (a global text-matched rule would
collide with ordinary identifiers).

## `Selection`

The single type rules interact with. A rule lambda receives a `Selection`
containing just the matched node; navigation methods derive sub-selections.

```rust
pub struct Selection<'r> {
    items: Vec<NodeOrToken>,   // significant items only — trivia never visible
    context: SyntaxNode,       // default softline measure span
    sink: &'r AtomSink,        // interior-mutable annotation store
}

impl Selection<'_> {
    // Navigation — thin, trivia-filtered wrappers over rowan, applying
    // across all selected items:
    pub fn node(&self, kind: SyntaxKind) -> Selection<'_>;
    pub fn token(&self, kind: SyntaxKind) -> Selection<'_>;
    pub fn token_matching(&self, f: impl Fn(SyntaxKind) -> bool) -> Selection<'_>;
    pub fn keyword(&self, text: &str) -> Selection<'_>;   // Identifier with this text
    pub fn children(&self) -> Selection<'_>;              // all significant children

    // Escape hatches — into rowan and back:
    pub fn iter(&self) -> impl Iterator<Item = &NodeOrToken>;
    pub fn at(&self, item: impl Into<NodeOrToken>) -> Selection<'_>;

    // Query:
    pub fn is_multiline(&self) -> bool;   // significant-token span, in the input

    // Annotation — atoms at boundaries, markers on the items themselves:
    pub fn prepend(&self, atom: Atom) -> &Self;
    pub fn append(&self, atom: Atom) -> &Self;
    pub fn leaf(&self) -> &Self;
    pub fn delete(&self) -> &Self;
}
```

Why this type exists at all (rather than raw rowan):

1. **Trivia filtering as an invariant** — rules must never see `Whitespace` /
   `Comment` tokens; baking that into the type beats a convention.
2. **Set semantics** — `node(State)` selects *all* `State` children and
   `.prepend(...)` attaches to each; no per-rule `for` loops.
3. **The annotation channel** — atoms need the shared sink and a measure
   context; bare rowan nodes have nowhere to carry either.

Deliberate non-features:

- **No `when(Cond)` predicate combinator.** Conditions are plain Rust:
  `if !element.is_multiline() { … }`. This makes annotation-time control flow
  explicit and makes it obvious *which node* the condition reads. (This is the
  port of `#single_line_only!` / `#multi_line_only!`; evaluating at annotation
  time is sound because multilineness is measured on the *input*, the same
  measurement phase-2 softline resolution uses.)
- **No sibling-anchor combinators** (`followed_by` etc.) and **no
  `first()`/`last()`** until a rule demands them; genuinely positional logic
  (adjacent-child pairs, the trailing element of a list, the Document-level
  blank-line cascades) is written as an `iter()`/`children()` loop plus
  `at()`.
- Derived selections and `at()` inherit `context` from their parent selection,
  so softlines attached several hops deep still measure against the rule's
  node unless the rule constructs a softline with an explicit span.
- With `&self` methods and an interior-mutable sink, several sub-selections
  can be held at once without borrow-checker friction; the `RefCell` is
  invisible outside the engine.

## Engine-core responsibilities (not rules)

Comments are trivia and therefore invisible to rules by design. The engine
core handles:

- comments: the sub-gap splitting and R1–R3 anchoring described in phase 2,
  plus re-indentation of own-line comments and block-comment continuation
  lines
- `// slint-fmt:ignore`: applies the internal `Leaf` marker to the next item,
  defined as *the largest node whose first significant token is the next
  significant token after the comment*
- blank-line preservation mechanics (capping at one blank line where
  `AllowBlankLines` is present)

## Verified implementation constraints

These came out of a feasibility review against the actual parser and rowan
(no blockers found; these are the corrections that must be respected):

1. **Trailing trivia lives *inside* nodes.** `DefaultParser` flushes trivia on
   `nth()` peeks, so e.g. a `Binding` node textually contains its trailing
   comment and newline. Therefore:
   - Multilineness is measured over the **significant-token span** (first to
     last non-trivia token), never `node.text_range()` — the raw range would
     misclassify many nodes as multiline.
   - Trivia is located by **linear token walk** (`SyntaxToken::next_token`,
     which already works around a rowan bug with empty sibling nodes such as
     the empty `Expression`/`CodeBlock` in `if (true) {}`), never by tree
     containment.
2. **Trivia shape between two significant tokens** is
   `(Whitespace? (Comment Whitespace?)*)` — at most one whitespace token per
   gap (maximal-munch lexer); a line comment's trailing `\n` is in the
   following whitespace token.
3. **Boundary keys**: `text_range().start()` of the significant token. Do not
   use rowan tokens as map keys (works, but costlier; Slint's `SyntaxToken`
   wrapper lacks `Eq`/`Hash`).
4. **Grammar quirks to design rules around**:
   - `SyntaxKind::Element` covers component bodies, `global` blocks and
     interface-like blocks alike; rules distinguish them via `node_ref()`-style
     inspection of the parent (`Component`'s leading identifier, `SubElement`).
   - `PropertyAnimation` has no body node (see measured softlines above).
   - Every expression is wrapped: `Expression(ConcreteKind(...))`; empty
     `Expression`/`CodeBlock` nodes exist and selectors must skip token-less
     nodes.
   - Binary/unary operators are bare tokens of ~15 kinds inside
     `BinaryExpression` etc. — hence `token_matching`.
   - Plain `a.b` is a `QualifiedName` (dots are tokens inside it);
     `MemberAccess` only covers `(expr).x`, `"s".x`, `1.x` — the latter needs
     the space-before-dot special case so `1 .foo` doesn't lex as `1.foo`.
5. **Idempotency** (format(format(x)) == format(x)) holds because softlines
   resolve on input multilineness, which the first run's output reproduces.
   Two constraints keep it true:
   - Do **not** treat "contains a comment" as multiline (the newline after a
     line comment is already inside the measured span; a single-line
     `/* x */` must not force a break).
   - `Hardline` must not appear strictly inside a softline-measured context
     that resolved single-line (it would flip that context to multiline on the
     next run). In the ported ruleset `Hardline` only occurs at Document top
     level; the engine should debug-assert this constraint.
6. **Notes for the eventual full rule port** (no engine work):
   - The `Transition` keyword accepts `in_out` as an alias of `in-out`.
   - The reference slint.scm's multiline-only antispace atoms are dead:
     `Antispace` never cancels a newline, only a same-or-lower-tier `Space`.
   - `InputSoftline` is *advisory*: at a boundary with no input newline it
     abstains entirely rather than deciding "nothing", so a weaker-tier
     `Space` at the same boundary still wins (matches Topiary).

## Example: the `states` construct

Grammar (`parser.rs` / `parser/element.rs`):
`States` = `Identifier("states")` `[` `State*` `]`;
`State` = `DeclaredIdentifier` (`Identifier("when")` `Expression`)? `:` `{`
(`StatePropertyChange` | `Transition`)* `}`.

```rust
// Equivalent of (states_definition …) in the Topiary slint.scm:
rules.node(SyntaxKind::States, |states: &Selection| {
    states.keyword("states").append(Space);                // `states [`
    states.token(SyntaxKind::LBracket)
        .append(IndentStart)
        .append(states.spaced_softline());
    states.node(SyntaxKind::State)
        .prepend(AllowBlankLines)                          // user blank lines survive
        .prepend(states.spaced_softline());
    states.token(SyntaxKind::RBracket)
        .prepend(IndentEnd)
        .prepend(states.spaced_softline());
});

// Equivalent of (state_definition …):
rules.node(SyntaxKind::State, |state: &Selection| {
    state.keyword("when").prepend(Space).append(Space);    // `pressed when touch.pressed`
    // `:` spacing comes from the global Colon rule — nothing to do here.
    state.token(SyntaxKind::LBrace)
        .prepend(Space)
        .append(IndentStart)
        .append(state.spaced_softline());
    state.node(SyntaxKind::StatePropertyChange)
        .prepend(AllowBlankLines)
        .prepend(state.spaced_softline());
    state.node(SyntaxKind::Transition)                     // `in { … }` / `out { … }`
        .prepend(AllowBlankLines)
        .prepend(state.spaced_softline());
    state.token(SyntaxKind::RBrace)
        .prepend(IndentEnd)
        .prepend(state.spaced_softline());
});
```

All softlines above share the `States` / `State` context node, so a state that
fits on one line in the input stays on one line, while anything the user
spread out formats to the canonical multiline shape.

An escape-hatch example — the `MemberAccess` dot spacing:

```rust
rules.node(SyntaxKind::MemberAccess, |member: &Selection| {
    let base_is_int = member.node(SyntaxKind::Expression).iter()
        .next()
        .is_some_and(is_bare_int_literal);   // plain fn over rowan nodes

    let dot = member.token(SyntaxKind::Dot);
    dot.prepend(if base_is_int { Space } else { Antispace });
    dot.append(Antispace);
});
```

## Suggested module layout

```text
tools/lsp/fmt/
  writer.rs     — unchanged (TokenWriter trait + FileWriter)
  tool.rs       — unchanged (CLI entry, embedded-source handling)
  atoms.rs      — Atom, Marker, AtomSink, priorities, FormatPlan
  engine.rs     — tree walk, rule dispatch, linearization, comment core, resolution
  render.rs     — FormatPlan realization through TokenWriter
  rules.rs      — the Slint ruleset (FormatRules construction; reads like slint.scm)
  tests.rs      — end-to-end tests, ported from the old imperative formatter
```

## Verification plan / open questions

- **Corpus diff**: run old and new formatter over the repo's `.slint` corpus
  and diff outputs; plus an idempotency pass (format twice, require fixpoint).
  Open question: where old formatter and the Topiary reference disagree, which
  output is canonical?
- **Macro sugar** (`rules! { States => { … } }`) is deferred: build the plain
  builder API first, add sugar only if the ruleset feels noisy.
