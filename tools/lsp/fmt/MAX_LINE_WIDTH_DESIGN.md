# Max Line Width — Design Notes

Status: **exploration, not yet planned in detail**

Goal: let the formatter decide "one line or many?" from how wide the output
would be — like prettier and rustfmt — instead of only from whether the
input was already multiline. That removes the tedious step of hand-breaking
a construct just so the formatter keeps it broken. `API_DESIGN.md` describes
the base architecture this builds on.

The implementation follows this paper:

> **A Pretty Expressive Printer** — Sorawee Porncharoenwase, Justin Pombrio,
> Emina Torlak. OOPSLA 2023. <https://doi.org/10.1145/3622837>
> (full paper with appendices:
> <https://sorawee.github.io/pretty-expressive-oopsla23-artifact/full-paper.pdf>)

Its printer finds the provably cheapest layout among all the ways a document
could be broken across lines, in time linear in the document size. It is
formally verified in Lean and powers the Racket code formatter.

## The paper in one paragraph

The printer takes a *document* (text fragments plus explicit "this layout or
that one" choice points), a *cost factory* (the definition of "pretty"), and
a *computation width limit* W. Expanding every choice would give
exponentially many layouts, so it never does that: it walks the document
bottom-up and keeps, per sub-document, only a small set of candidate
summaries ("measures") that could still win, pruning everything provably
worse. Past W it stops optimizing — output is still produced, just without
the optimality guarantee — and that cutoff is what bounds the running time.

## Terminology: paper ↔ our formatter

The paper's input is not a syntax tree. It is what our **phase 2 would
produce if it kept alternatives open instead of deciding them** — a
`FormatPlan` where some instructions are still "either/or".

| Paper (Π_e)     | Our formatter                                                                |
|-----------------|------------------------------------------------------------------------------|
| `text "foo"`    | `EmitToken` / `EmitLiteral` — a run of characters without a newline           |
| `nl`            | a `Newline` whitespace decision (newline + current indentation)               |
| `a <> b`        | plain concatenation — adjacent slots in the plan                              |
| `nest n d`      | `IndentStart` … `IndentEnd` around `d` (indent level +n for the `nl`s inside) |
| `align d`       | **no equivalent** — sets indentation to the *current column* (e.g. lining up arguments under the open paren). We only have indent levels. |
| `flatten d`     | the single-line rendering of a group: every `nl` inside becomes one space — what a softline group emits when it resolves to `Space` |
| `a <|> b`       | **the new concept**: a choice point. Both alternatives are kept; the *printer* picks one by cost. |
| choiceless document | a fully resolved `FormatPlan`: every gap has a concrete decision         |
| resolve phase   | our phase 2                                                                   |
| render phase    | our phase 3 (mechanical, no decisions left)                                   |

Two mappings matter most:

- **Our softline group is the paper's `group`.** `group(d)` is sugar for
  `flatten(d) <|> d`: all break points in the group collapse or break
  together. A measure span containing several softlines is exactly one
  group — one binary choice controlling many gaps. Today the input decides
  that choice; here, cost does.
- **Our trailing-comma trick is the paper's headline argument for arbitrary
  choice.** `group` alternatives can only differ in whitespace, but a broken
  argument list has a trailing comma the collapsed one lacks (literally
  footnote 1 of the paper). Our `Delete` marker + `Literal(",")` pair
  encodes exactly such a content-differing choice — today decided up front
  by the rule, here by the search.

## The cost factory

The printer hard-codes no notion of "pretty"; it takes a cost type with four
operations: `text(column, length)` — the cost of placing characters starting
at a column (where "past column 80 hurts" lives), `nl` — the cost of a
newline, `+`, and a total order `≤`. A layout's cost is the sum over its
lines. The operations must obey a few contracts (associative `+` that
respects `≤`, text cost monotone in the column, splitting a string costs the
same as placing it whole) — these are what make it sound to discard a losing
partial layout mid-search: it can never come back.

The paper's default factory — (sum of *squared* overflow past the page
width, line count), compared lexicographically — degrades gracefully:
squaring makes one line 20 characters over worse than two lines 10 over, and
when overflow is unavoidable you get the least-bad layout instead of an
error or a greedy mess.

## Decided: author intent as a cost component

Today's softlines preserve the author's intent. We keep that default by
making *deviation from the input* part of the cost — a lexicographic triple:

1. **overflow** — sum of squared characters past the max line width,
2. **deviation** — number of groups decided against the author's layout,
3. **height** — number of newlines.

While everything fits, deviation decides and the author's layout wins.
Height ranks *below* deviation on purpose — otherwise the printer would
collapse every construct the author deliberately spread out, since fewer
lines is always cheaper. Once a line exceeds the max width, overflow
dominates and the printer overrides the author with the cheapest breaks.

The penalty is attached at document-build time, where the input is still in
hand: the choice branch that flips the author's decision costs one
deviation. It is assessed **once per group decision**, against the group's
input multilineness — not per gap. Per-gap counting would change semantics:
a list with 1 of 4 gaps broken would flatten (1 flip beats 3), whereas today
any input newline in the span breaks the whole group.

**Decided: `InputSoftline` stays outside the search.** It keeps its literal
meaning — a newline iff the input had one — and resolves at document-build
time into a *fixed* decision, exactly as today. The search never adds a
break there. (It never *removes* input newlines anywhere: joining two lines
cannot reduce overflow, and height ranks below deviation — so the only
question was whether overflow may add breaks here, and the answer is no.)
This keeps the atom's role crisp — "the author owns this break", which its
error-recovery uses need — and keeps these gaps out of the choice-point
count. A rule that *wants* a width-breakable, input-preferred boundary needs
no new atom: a measured softline whose span is just that gap produces
exactly that through the ordinary group machinery.

Idempotency becomes provable for the core case — untainted resolutions,
given the flatten rule below (a single-line body never contains a newline).
On a second run the chosen layout *is* the input, so its deviation is 0 at
every explicit choice point. A competitor either has worse overflow (which
does not depend on the input — conditional literals render the same text per
variant in both runs), or equal overflow and deviation ≥ 1, or agrees on
every explicit choice point — and then it already lost in run 1 on height or
on the deterministic tie-break. That tie-break must live in our own
`dedup`/`merge` (keep the earlier measure on exact ties); the paper's left
bias covers only the all-tainted case. Outside the proof: **tainted output**
(idempotent separately, via the taint bias toward the input-matching
variant, since taintedness itself reproduces — widths do not depend on the
input), and **documents whose comment geometry changes between runs** — the
remaining comments open question.

## Decided: dynamic rules via conditional atoms

Some rules decide *content* from multilineness — the trailing comma:
`Literal(",")` after the last item when the list breaks, `delete()` the
comma token when it collapses. Today that is a plain Rust `if` over input
multilineness at annotation time. With the decision moved into the resolver,
a rule can no longer branch — it must hand the engine **both worlds** and
let the search pick.

### Conditional atoms (the primitive)

Atoms and the `delete()` marker gain a condition, keyed by a group's span
the same way softlines already carry their measure span by value:

```rust
// sugar — group span = the rule's context node, the common case:
last_item.append(states.literal_if_multiline(","));
comma.delete_if_single_line(states);

// explicit-span form, same convention as Atom::SpacedSoftline(range):
last_target.append(Atom::Literal(",").if_multiline(target_list_range));
```

At document construction, everything sharing a span — softlines, conditional
atoms and deletes, the deviation penalty — is wired into **one** choice, so
it all flips together by construction.

Reading note: `if_multiline` refers to the **chosen variant of the group**,
not to the input. The words match `Selection::is_multiline()` (and the
Topiary predicates this replaces), but the referent moved from "what the
author wrote" to "what the resolver picked".

There is deliberately no new "group" concept in the rule API: the group
identity is the measure span the selection already carries as context. The
change is engine-internal — today equal spans agree trivially; under search,
atoms sharing a span must feed the *same* choice point. A condition is
stored as `(span, variant)` with `Variant` an enum, so future n-ary layout
styles (e.g. fill-wrapped arrays) extend the engine without touching the
atom vocabulary.

One restriction keeps the architecture sound: **conditions attach to
`Literal` and `delete()` only** — never to whitespace or indent atoms.
Whitespace already has its conditional mechanism (the softline *is* how a
group controls whitespace), and unconditional indent atoms are what make
every potential newline's indent computable before the search — the
resolver's memo key depends on it. Debug assertions guard the boundary: a
conditionally deleted token must not carry `IndentStart`/`IndentEnd`, and a
`Literal`'s text must not contain a newline.

Two engine obligations:

- **Choice spans must nest or be disjoint** — a choice tree cannot
  represent partial overlap. Node-derived spans satisfy this automatically;
  hand-built spans are covered by a debug assertion.
- **Any fixed newline inside a group's span removes its single-line
  variant** at build time: a line comment (flattening would swallow the
  rest of the line into it), an own-line comment (R1 forces its newline), a
  `Hardline`, a fired `InputSoftline` (when it survives the flat variant's
  tier merge), or a multi-line block comment or `Leaf` range. This is a
  constraint (one branch deleted), not a condition, and is invisible to
  rules. These are the *only* fixed-newline sources: the engine resolves
  every gap, and a gap no rule touched defaults to a single space — input
  newlines there are not preserved.

### `separated_by` (the sugar)

The trailing-separator idiom is pervasive, so it gets a one-line helper:

```rust
// FunctionCallExpression rule:
call.node(SyntaxKind::Expression).separated_by(SyntaxKind::Comma);
```

It expands to the primitives: an **explicit group span** from the first to
the last item (grammar nodes may fuse a list and a body into one node —
`Function` holds both its parameters and its code block — so the rule's
context node is often the wrong span), the group's separator softlines,
`Literal(",").if_multiline(group)` on the last item, and
`delete().if_single_line(group)` on an input trailing separator token.
Correlation correctness and the idempotency argument are inherited from the
primitives, proven once.

The helper assumes nothing about the global punctuation rules, and this
design assumes nothing about *which* lists use the helper. How a construct
breaks — as one atomic group, per input newline, or not at all — is the
ruleset's business; the algorithm only ever sees groups and fixed decisions.

`separated_by` always manages the trailing separator — no `with_trailing()`
builder, no no-trailing twin. Justified by a survey of all parser loops:

- **Trailing comma accepted (12 lists):** arrays, object literals,
  function-call arguments, callback-connection parameters,
  callback-declaration parameters, function argument declarations, object
  types (structs reuse them), enum values, import lists, `uses` lists,
  export lists, gradient stops.
- **Trailing comma rejected (2 lists):** the `animate x, y` target list and
  `@tr(...)` arguments — the parser demands another item after the comma.

The two exceptions are exactly the lists that would never call the helper
(short names that realistically never break; a call-shaped list expressible
with plain atoms). If a list ever wants group-coherent spacing without
trailing insertion, a second function is added then — the same deferral
policy the main design applies to `first()`/`last()`.

### Rejected alternative: per-variant rule closures

```rust
states.variants(|flat, broken| {
    flat.token(SyntaxKind::Comma).delete();
    broken.node(SyntaxKind::State).append(Literal(","));
});
```

Arbitrary Rust `if`s work inside each closure, and branches may differ in
anything. Rejected: every branch difference the grammar actually needs is
insert-literal, delete-token or spacing — all expressible with conditional
atoms — while closures cost layered annotation sinks, layer multiplication
under nested groups, and nothing structurally prevents the two closures from
drifting into inconsistent pairs. It remains the escape hatch if a rule ever
needs more.

## The algorithm, briefly

A **measure** summarizes a candidate layout without rendering it: the length
of its last line (the column where the suffix will start) and its
accumulated cost. Neither dominates the other — paying more can buy a
shorter last line that a long suffix needs — so per (sub-document, start
column) the resolver keeps the **Pareto frontier** of measures and prunes
the rest immediately. Last-line lengths are distinct integers ≤ W, so a
frontier has at most W+1 entries.

**Taintedness** bounds the work: the moment a resolution's column exceeds
the computation width limit W (the paper defaults to 100 for page width 80),
the measure set collapses to a single lazily-computed "tainted" candidate
with no optimality claim. Merging prefers untainted sets, so tainted
branches survive only when *every* alternative blows past W — the printer
then still produces output rather than failing. Worst-case time is
O(n · W⁴) with n the document size; in practice fast (10k-line JSON in
~7 ms, 5–6k-line Racket files in ~0.4 s). The guarantee: the output is
valid, and optimal among all layouts that stay within W.

## Implementation sketch (pseudo-code)

Scope: no `align`, no fill-wrapping — binary groups only. `page_width`
(where overflow starts to hurt) and `computation_width` (W, where the search
gives up optimizing) are configurable parameters.

Phases 1 and 3 are untouched, and the output is the same `FormatPlan` as
today, so the `TokenWriter` sinks never know the difference. Phase 2
changes — and per-gap resolution moves inside document construction, run
once per (gap, variant):

```text
annotate (unchanged)
  ──▶ linearize (unchanged)
  ──▶ NEW: build the choice document
        — per-gap resolution (tier merge, Antispace, comment sub-gaps,
          defaults) moves INTO this step, run once per (gap, variant)
  ──▶ NEW: resolve = search for the cheapest variant assignment
  ──▶ emit FormatPlan (as today, but reading group decisions from the search)
  ──▶ render (unchanged)
```

### Why per-gap resolution moves inside document construction

The engine resolves every gap once, in one linear pass (untouched gaps get
the default: a single space, nothing at a document edge). Under the search
that pass would need the group decisions — which do not exist yet. Two
dependencies force the per-variant split:

1. **Conditional deletes change which atoms meet in a gap.** A deleted
   token's own atoms are discarded, and the following gap sources its
   append-side atoms from the last *emitted* token. With
   `delete_if_single_line`, which token that is depends on the variant.
2. **Comment routing (R2) routes by resolved strength.** In the multiline
   variant a group's newline transfers past a trailing comment (`{ // note`
   keeps the comment hanging); in the single-line variant the space stays
   before the comment. Even the *order* of whitespace and comment differs
   per variant — and comment position affects line width, so none of this
   can wait until plan emission.

So the builder calls `resolve_gap(gap, variant)`: today's merge machinery —
tier-first/strength-second, Antispace cancellation, sub-gap splitting, R1–R3
anchoring, the default — with the controlling group's variant substituted
for the softline measurement. It returns the gap's document sequence
(whitespace and comment `Text` docs, in routed order) instead of pushing
instructions. `InputSoftline` is not part of the search: `resolve_gap`
resolves it from the input in every variant — an input newline yields a
fixed `Newline` doc, otherwise the atom abstains and the remaining atoms or
the default decide.

### Which group controls a gap

Atoms with *different* spans can meet in one gap (a token-tier rule's
softline measures the token's parent node; a node-tier rule's softline may
carry a hand-built span). The controlling group is decided the way today's
merge would decide it:

- The winning **tier** at a gap does not depend on any variant, so it is
  decided up front. If the winning tier contains a softline, the gap is
  group-controlled ("controlled" = its outcome varies with that group's
  variant); among several softlines in the winning tier, the **innermost
  span** controls.
- An outer group's softline landing at a gap strictly inside an inner
  group's span is **captured** by the inner group: it breaks when the inner
  group goes multiline and flattens when it goes single-line.
- Two softlines whose spans neither nest nor are disjoint cannot be
  represented in a choice tree — debug-assert (no current rule produces
  it).
- A group can end up controlling no gap (everything captured by inner
  groups); its choice is degenerate and the builder drops it.

### Data types

```rust
/// Identity of one correlated choice. One per distinct span used by
/// measured softlines / conditional atoms (InputSoftline resolves to a
/// fixed decision instead). Spans must nest or be disjoint.
struct GroupId(u32);
enum Variant { SingleLine, Multiline }

/// The lexicographic cost triple. Compared field by field, in this order.
struct Cost {
    overflow: u64,   // sum of squared characters past page_width
    deviation: u32,  // number of explicit choices decided against the input
    height: u32,     // number of Newline docs (a blank-line upgrade still counts 1)
}

// Saturating: columns left of page_width cost nothing (and u32 subtraction
// must not underflow).
fn squared_excess(x: u32) -> u64 { (x.saturating_sub(page_width) as u64).pow(2) }

impl Cost {
    fn add(a: Cost, b: Cost) -> Cost { /* component-wise sum */ }
    fn less_than(a: Cost, b: Cost) -> bool { /* lexicographic */ }

    /// Cost of placing `length` characters starting at `column`.
    /// IMPORTANT: defined as a difference so that placing a line in pieces
    /// costs the same as placing it at once (the cost-factory "splitting"
    /// contract — a plain per-piece square would violate it and with it
    /// the optimality proof):
    fn text(column: u32, length: u32) -> Cost {
        Cost { overflow: squared_excess(column + length) - squared_excess(column), ..zero }
    }
    /// A newline must charge the indentation it emits: otherwise breaking
    /// at an indent beyond page_width looks free, and the search would
    /// prefer many overflowing short lines over the least-bad compromise.
    /// (The paper's practical implementation makes nl a function of the
    /// indent for exactly this reason.)
    fn newline(indent_width: u32) -> Cost {
        Cost { overflow: squared_excess(indent_width), height: 1, ..zero }
    }
    fn deviation() -> Cost { Cost { deviation: 1, ..zero } }
}

// Why the deviation constant is sound (the paper's verified optimality
// theorem does NOT cover per-site costs — its practical `cost` construct
// is left unformalized): each penalty is a constant added uniformly to
// every measure of one branch, BEFORE that branch is first merged (and
// therefore pruned) against its sibling. Adding a constant preserves
// domination within the branch (lexicographic order over the triple is
// translation-invariant), so no pruned measure could have beaten a
// survivor; across branches the penalty is already in place when pruning
// first compares them.

/// The choice document. Built once per format run; nodes are referenced by
/// id so the resolver can memoize on them.
enum Doc {
    /// A SINGLE-LINE run of characters: a token, one line of a comment, a
    /// space, or a conditional literal's text — never contains a newline.
    /// The builder expands multi-line verbatim items (block comments,
    /// multi-line Leaf ranges) into
    ///   Text(first line) · Newline(fixed) · … · Text(last line)
    /// so width and overflow are counted per line and the column after
    /// them is the LAST line's length — one Text with the total width
    /// would compute garbage columns and taint spuriously.
    Text { source: SlotOrLiteral, width: u32 },
    /// A line break — fixed (R1 comment newlines, Hardline, verbatim
    /// newlines inside Leaf ranges and multi-line block comments) or
    /// emitted by a group's multiline body. The indent is baked in at
    /// build time: indent atoms are never conditional, so a running
    /// counter fixes every potential newline's indent before the search.
    /// (This is why the sketch needs no `nest` node and no indent in the
    /// memo key — a real simplification over the paper.)
    Newline { indent_width: u32 },
    Concat(Vec<DocId>),
    /// One group. Built only when flattening is permitted — otherwise the
    /// builder emits the multiline body directly (see build_group).
    Choice {
        group: GroupId,
        single_line: DocId,
        multiline: DocId,
        /// Which variant deviates from the input layout (pays Cost::deviation()).
        penalized: Variant,
    },
}

/// One surviving candidate: everything we must know about a partial layout
/// without rendering it.
struct Measure {
    last_line_width: u32,
    cost: Cost,
    /// The decisions taken so far, as an O(1)-append persistent list of
    /// (GroupId, Variant) entries — flattened into a map once, at top
    /// level. (A persistent-map union per combine() would multiply the
    /// complexity by the group count.)
    decisions: DecisionList,
}

/// A Pareto frontier — sorted by cost strictly ascending, which on a valid
/// frontier means last_line_width strictly DESCENDING: cheap layouts end in
/// long last lines; paying more buys a shorter last line for the suffix to
/// start from — or a single lazily computed fallback once we blew past W.
enum MeasureSet {
    Set(Vec<Measure>),          // at most W+1 entries (distinct widths in 0..=W)
    Tainted(Lazy<Measure>),     // greedy, no optimality claim, MUST stay unevaluated
}
```

### Building the document

Input: the token slots and atom stores from phase 1, and the set of groups
(every distinct span carried by a measured softline or conditional atom).
The groups' spans form a forest by nesting; tokens and gaps hang off the
group that controls them.

```rust
fn build_document() -> DocId {
    // Walk the span forest from the root. Each group builds at most two
    // docs — its multiline body and its flattened body — each built once
    // and shared, so the document stays linear in the input size.
    build_group(root_span)
}

fn build_group(group) -> DocId {
    let multiline = build_body(group, Variant::Multiline);
    match build_flat(group) {
        // Flattening forbidden: not a real choice. No Choice node exists,
        // so this GroupId is never recorded in any decision list — see the
        // lookup rule in emit_plan.
        None => multiline,
        Some(single_line) => alloc(Doc::Choice {
            group: group.id,
            single_line,
            multiline,
            penalized: if group.input_was_multiline { Variant::SingleLine }
                       else                         { Variant::Multiline },
        }),
    }
}

fn build_body(group, variant) -> DocId {
    let mut parts = Vec::new();
    for item in group.items_in_source_order() {
        match item {
            Token(slot)          => parts.push(text_doc(slot)),
            // Conditional literals / deletes contribute only to their variant:
            ConditionalLiteral(text, for_variant) if for_variant == variant
                                 => parts.push(literal_doc(text)),
            DeletedToken(slot, for_variant)
                                 => if for_variant != variant { parts.push(text_doc(slot)) },
            // EVERY gap goes through per-variant gap resolution. For a gap
            // controlled by this group, `variant` decides its softline;
            // fixed gaps ignore it. The result is a small doc sequence:
            // Text(" ") / Newline / nothing, interleaved with comment
            // Text docs in routed order.
            Gap(gap)             => parts.extend(resolve_gap(gap, variant)),
            // Inner groups stay open choices in the multiline body:
            InnerGroup(g)        => parts.push(build_group(g)),
        }
    }
    alloc(Doc::Concat(parts))
}

/// The single-line body: like build_body(SingleLine), but inner groups are
/// forced single-line recursively. Returns None if the built body contains
/// ANY Newline doc, of whatever origin (see the fixed-newline list in the
/// dynamic-rules section). Checking the BUILT body for Newline docs is
/// mechanical — there is no case list to keep in sync.
fn build_flat(group) -> Option<DocId>
```

### The resolver (the search)

```rust
/// Memoized on (doc, column). Column is capped by W, so the table stays
/// small; sharing (each group's two bodies built once) keeps it linear in
/// the document.
fn resolve(doc: DocId, column: u32) -> MeasureSet {
    match doc {
        Doc::Text { width, .. } => {
            if column + width > computation_width {
                MeasureSet::Tainted(lazy_single_measure(doc, column))
            } else {
                set_of_one(Measure {
                    last_line_width: column + width,
                    cost: Cost::text(column, width),
                    decisions: empty,
                })
            }
        }
        Doc::Newline { indent_width } => {
            if indent_width > computation_width { /* Tainted, as above */ }
            set_of_one(Measure {
                last_line_width: indent_width,
                cost: Cost::newline(indent_width),
                decisions: empty,
            })
        }
        Doc::Concat(parts) => {
            // Fold left. For each surviving measure of the prefix, resolve
            // the next part starting at that measure's last-line width.
            let mut result = set_of_one(empty_measure(column));
            for part in parts {
                result = concat_sets(result, part);
            }
            result
        }
        Doc::Choice { group, single_line, multiline, penalized } => {
            // tag() and penalize_if() COPY the measure vector before
            // modifying it: the child body and its memo entry are shared
            // with other parents (notably every enclosing group's flat
            // body), which must not see this group's tag or penalty. The
            // post-tag result is memoized under the Choice's own id only.
            let flat = resolve(single_line, column)
                .tag(group, Variant::SingleLine)          // record the decision
                .penalize_if(penalized == Variant::SingleLine);
            let broken = resolve(multiline, column)
                .tag(group, Variant::Multiline)
                .penalize_if(penalized == Variant::Multiline);
            // Put the input-matching side LEFT: merge keeps the left side
            // when BOTH are tainted, so a hopeless overflow degrades to
            // "keep the author's layout" — today's behavior.
            merge(in_input_order(flat, broken))
        }
    }
}

fn concat_sets(left: MeasureSet, right_doc: DocId) -> MeasureSet {
    match left {
        Set(measures) => {
            // Resolve the suffix once per distinct prefix end-column, then
            // MERGE the per-prefix results. The Set-preferring merge means
            // one prefix whose suffix taints does NOT taint the others —
            // the result is Tainted only if every prefix's suffix tainted
            // (paper Fig. 15, ConcatRS). Tainting the whole concat would
            // let an irrelevant too-wide sibling flip an enclosing Choice.
            let per_prefix = measures.map(|m| {
                match resolve(right_doc, m.last_line_width) {
                    Set(suffixes) => Set(dedup(suffixes.map(|s| combine(m, s)))),
                    // combine under the SAME Lazy — nothing forced here:
                    Tainted(lazy) => Tainted(lazy_combine(m, lazy)),
                }
            });
            per_prefix.reduce(merge)
        }
        // Force the prefix, resolve the suffix greedily, combine — ALL of
        // it inside the returned Lazy, or the work bound past W is lost:
        Tainted(lazy_prefix) => Tainted(lazy_concat(lazy_prefix, right_doc)),
    }
}

fn combine(prefix: Measure, suffix: Measure) -> Measure {
    Measure {
        last_line_width: suffix.last_line_width,
        cost: Cost::add(prefix.cost, suffix.cost),
        decisions: prefix.decisions.append(suffix.decisions), // O(1), disjoint by construction
    }
}

/// Keep only the Pareto frontier. Sort by cost ascending (on equal cost:
/// width ascending; on equal cost AND width keep the earlier measure — the
/// deterministic tie-break idempotency relies on). Then keep a measure iff
/// its last_line_width is strictly smaller than every measure already kept.
/// (Getting this backwards — walking width-descending and filtering for
/// DECREASING cost — silently keeps only the cheapest measure and discards
/// the short-last-line candidates the frontier exists for. Unit-test this
/// against hand-built frontiers.)
fn dedup(measures: Vec<Measure>) -> MeasureSet

/// Merge two frontiers (mergesort by cost + the dedup width filter, left
/// preferred on exact ties). Prefer Set over Tainted; if both are Tainted,
/// keep the LEFT one (greedy fallback).
fn merge(a: MeasureSet, b: MeasureSet) -> MeasureSet
```

### Top level and plan emission

```rust
fn resolve_widths(document_root: DocId) -> Decisions {
    match resolve(document_root, /* column */ 0) {
        // Frontier costs are strictly ascending, so the optimum is simply
        // the FIRST entry — deterministic by construction.
        Set(measures)  => measures.first().decisions.flatten_to_map(), // optimal within W
        Tainted(lazy)  => lazy.force().decisions.flatten_to_map(),     // best effort, still valid
    }
}

fn emit_plan(decisions) -> FormatPlan {
    // Exactly today's phase-2 output step, except a group-controlled gap
    // asks the decision map instead of measuring input multilineness:
    //   Multiline  -> ReplaceGap Newline { indent level, blank-line upgrade
    //                 if AllowBlankLines and the input had one }
    //   SingleLine -> ReplaceGap Space / None
    // Conditional literals emit only in their variant; a conditionally
    // deleted token becomes DeleteToken only in its variant.
    // Fixed gaps, comment sub-gaps, Leaf ranges: unchanged.
    //
    // A group can be MISSING from the map. The lookup rule:
    //   recorded                   -> as recorded
    //   absent + flattenable       -> SingleLine (it was inlined into an
    //                                 ancestor's chosen flat body — the only
    //                                 way a flattenable group goes undecided)
    //   absent + flatten-forbidden -> Multiline (its Choice never existed)
    //
    // Consequence: deviation is counted per EXPLICIT choice point; a group
    // flattened implicitly by an ancestor pays no penalty of its own.
    // (Flattening never reduces overflow, so the undercount cannot flip a
    // decision by itself — but it diverges from the literal "number of
    // groups that flip the author's layout" wording.)
}
```

### Mapping onto the existing code (`engine.rs`)

The sketch reuses the implementation's names where the concepts line up.
The concrete deltas to the shipped engine:

- **`resolve()` is today a fused single pass**: it walks the slots, handles
  `Leaf`/`Delete` markers, pushes `Instruction`s directly, and threads a
  mutable `i32` indentation counter; `resolve_gap` likewise writes
  instructions straight into the output vector. The build → search → emit
  split refactors this into three passes: a builder that calls
  `resolve_gap(gap, variant)` and gets decisions / doc fragments back, the
  search, and an emitter that replays the winning decisions into exactly
  today's instruction sequence. Inside `route_atom`, only the three
  softline arms change (variant substitution instead of input
  measurement); the Antispace, indentation and blank-line bookkeeping stays
  as is.
- **Indent levels are transiently negative** — the counter is an `i32`,
  clamped only at emission (`.max(0)`). `Doc::Newline { indent_width }`
  must bake the *clamped* level, converted to columns (level × indent
  unit).
- **`Literal` and `delete()` are engine-complete but unused by the shipped
  ruleset** (`#[allow(dead_code)]`, exercised only by engine tests). So
  conditional atoms have no shipped-rule migration burden — and the
  existing four-quadrant trailing-comma test
  (`trailing_comma_managed_across_all_four_quadrants`), whose rules use the
  annotation-time `if is_multiline` pattern this design replaces, is a
  ready-made acceptance test: same expected outputs, decided by the search
  instead of the input measure.
- **The debug-only `IdempotencyGuard` is the home for the new assertions.**
  "A chosen SingleLine variant renders without newlines" generalizes its
  current Hardline-inside-single-line-softline-span check. The guard must
  become variant-aware: today it classifies spans by measuring the *input*,
  which stops being the decision procedure — it should read the search's
  decisions instead.
- **Eof is a real slot**, and the Document rule lands the file-final
  `Hardline` in the gap before it. The document builder must include that
  gap; the Eof token itself enters as a zero-width `Text`.

### Notes for the implementer

- **Blank-line upgrades and own-line-comment re-indentation** stay
  engine-core mechanics at plan-emission time. Comments participate in the
  search as `Text` docs (width counts toward overflow) and `Newline` docs
  (which also forbid flattening); `height` counts Newline docs, so a
  blank-line upgrade is one unit like any other — an accepted
  approximation.
- **Laziness matters.** `Tainted` must stay unevaluated unless it is the
  only option — including through `concat_sets`, where combining goes
  *inside* the returned Lazy. That is what bounds work past W.
- **Debug assertions**: group spans nest or are disjoint; no conditional
  indent atoms; no newline inside a `Literal`'s text; a chosen SingleLine
  variant renders without newlines; decision lists concatenate disjointly.

## Open questions (to resolve in the concrete plan)

- **Comment geometry across runs.** How comments enter the search is
  specified (they become `Text`/`Newline` docs, `resolve_gap` runs the
  R1–R3 anchoring once per variant, and any comment-forced newline forbids
  flattening). Open is *cross-run stability*: R2 routing can move a group's
  newline past a trailing comment, so the second run sees different sub-gap
  shapes than the first — the idempotency proof does not cover this. Either
  show the routed shape is a fixpoint (corpus check), or make
  comment-adjacent group boundaries resolve identically in both runs by
  construction. Decide during implementation.
- **`align`.** The paper supports column alignment; our design has only
  indent levels. Do we want it? (Nothing forces us to adopt it.)
- **Page width and W.** What page width do we format to, and what
  computation width limit W (the paper uses W = page width + 25%)?
- **Fill-wrapped lists.** Long literal arrays (model data) could pack as
  many items per line as fit, like the paper's `fillSep`. A
  document-construction pattern (a chain of per-gap choices), not a new
  condition kind — deferrable without regression.
