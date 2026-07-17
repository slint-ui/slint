# Max Line Width — Design Notes

Status: **exploration, not yet planned in detail**

This document is the starting point for adding max-line-width-aware layout to
the query-based formatter (see `API_DESIGN.md` for the overall architecture).
The goal: instead of deciding "one line or many lines?" purely from whether
the *input* was already multiline, the formatter should be able to decide it
from *how wide the output would be* — like prettier and rustfmt do. This
removes the tedious manual step of breaking a construct by hand just so the
formatter keeps it broken.

The implementation will be based on this paper:

> **A Pretty Expressive Printer** — Sorawee Porncharoenwase, Justin Pombrio,
> Emina Torlak. OOPSLA 2023. <https://doi.org/10.1145/3622837>
> (full paper with appendices:
> <https://sorawee.github.io/pretty-expressive-oopsla23-artifact/full-paper.pdf>)

The paper's printer (called Π_e, implemented as *PrettyExpressive*) finds the
**provably cheapest layout** among all the ways a document could be broken
across lines, in time linear in the document size. It is formally verified in
Lean and is the engine behind the Racket code formatter, so it is a solid
foundation rather than a research toy.

## The paper in one paragraph

You give the printer three inputs: a *document* (text fragments plus explicit
"either this layout or that one" choice points), a *cost factory* (your
definition of "pretty", e.g. "first minimize overflow past column 80, then
minimize the number of lines"), and a *computation width limit*. Expanding
every choice would give exponentially many layouts, so the printer never does
that. Instead it walks the document bottom-up and, for every sub-document,
keeps only a small set of candidate summaries ("measures") that could still
win, pruning everything provably worse. Past the width limit it stops
optimizing entirely (it still produces output, just without the optimality
guarantee) — that cutoff is what bounds the running time.

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

Two mappings deserve emphasis:

- **Our softline group is the paper's `group`.** `group(d)` is sugar for
  `flatten(d) <|> d`: "all these break points collapse together or break
  together". A measure span containing several `SpacedSoftline`s is exactly
  one group — one binary choice controlling many gaps at once. Today we
  answer that binary choice by looking at input multilineness; the paper
  answers it by cost.
- **Our trailing-comma trick is the paper's headline argument for arbitrary
  choice.** `group`/`flatten` alternatives can only differ in *whitespace*,
  but the broken and collapsed forms of an argument list differ in *content*:
  the trailing comma exists in one and not the other. That is literally
  footnote 1 of the paper. Our `Delete` marker + `Literal(",")` pair is a
  hand-rolled encoding of exactly such a content-differing choice — except
  today the rule picks the side up front, whereas the paper's printer picks
  it during resolution.

## The cost factory: "optimal" is pluggable

Instead of hard-coding "first layout that fits wins" (prettier) or "fewest
lines without overflow" (which *errors* when everything overflows), the
printer takes a user-supplied **cost factory**: a cost type plus four
operations —

- `text(column, length)` — cost of placing that many characters starting at
  that column (this is where "past column 80 hurts" lives),
- `nl` — cost of a newline (so fewer lines is cheaper),
- `+` — combine two costs,
- `≤` — compare two costs (must be a total order).

The cost of a whole layout is the sum over its lines. The paper's default
factory: cost = (sum of *squared* overflow past the page width, number of
newlines), compared lexicographically. Squaring means one line 20 characters
over is worse than two lines 10 over, and overflow degrades gracefully: when
it is unavoidable you get the *least bad* layout instead of an error or a
greedy mess.

The factory must obey a few contracts (total order, associative `+` that
respects `≤`, text cost monotone in the start column, splitting a string must
cost the same as placing it whole). These contracts are what make it sound to
discard a partial layout mid-search — a candidate that is already losing can
never come back.

## Decided: author intent as a cost component

Today's softlines preserve the author's intent: what the author wrote on one
line stays on one line, what they spread out stays spread out. We keep that
default in the width-aware world by making *deviation from the input* part of
the cost. Our cost type is a lexicographic triple, compared in order:

1. **overflow** — sum of squared characters past the max line width,
2. **deviation** — the number of groups whose decision flips the author's
   layout,
3. **height** — the number of newlines.

This ordering produces exactly the behavior we want:

- Everything fits → overflow is 0 for several candidates, so deviation
  decides: the author's layout wins. Height ranking *below* deviation is
  essential — otherwise the printer would collapse every construct the
  author deliberately spread out, because fewer lines is always cheaper.
- A line exceeds the max width → overflow dominates, and the printer
  overrides the author with the cheapest set of breaks; deviation only
  breaks ties among equally-overflowing candidates.

Mechanics: we build the document ourselves, and at document-build time the
input is still in hand (the same place today's phase 2 reads input
multilineness). So when constructing a choice, we know which branch deviates
from the input and attach a constant penalty to it. The paper's formal
language has no per-site cost construct, but its practical implementation
does (the Racket library's `cost` combinator, Appendix C), and in any case it
is a small engine extension that respects all the cost-factory contracts —
it is just one more `+`.

Granularity: the penalty is assessed **once per group decision**, against the
input multilineness of the group's span — not per gap. Per-gap counting would
change semantics: a list where the author broke 1 of 4 gaps would resolve
flat (1 flip beats 3 flips), whereas today any input newline in the measure
span breaks the whole group. `InputSoftline` stops being a special atom under
this design: it is simply a single-gap group carrying a deviation penalty.

Idempotency, which today rests on an informal argument, becomes provable —
for the core case: **untainted resolutions**, given the flatten rule from
the implementation sketch (a single-line body never contains a newline). On
a second run the chosen layout *is* the input, so its deviation is 0 at
every explicit choice point. A competitor either has worse overflow (loses
at tier 1 — a layout's overflow does not depend on the input; conditional
literals render the same text per variant in both runs), or has equal
overflow and deviation ≥ 1 (loses at tier 2), or agrees with the winner on
every explicit choice point — then it already competed on equal terms in
run 1 and lost on height or on the deterministic tie-break. Note the
tie-break must be specified in our own `dedup`/`merge` (keep the earlier
measure on exact ties); the paper's left bias covers only the all-tainted
case. Two cases sit outside this proof: **tainted output** has no
optimality property — there idempotency follows separately from the taint
bias (the input-matching variant sits left, and taintedness itself
reproduces because widths do not depend on the input); and **documents
whose comment geometry changes between runs** — R2 routing can move a
group's newline past a trailing comment, so the second run sees different
sub-gap shapes. That last case is the remaining comments open question.

## Decided: dynamic rules via conditional atoms

Some rules today decide *content*, not just whitespace, based on
multilineness — the trailing-comma pattern: append a `Literal(",")` to the
last list item when the list is multiline, `delete()` the comma token when it
collapses onto one line. Currently that is a plain Rust `if` over input
multilineness, evaluated at annotation time. In the width-aware world the
multiline decision is made *by the resolver*, long after annotation — so the
rule can no longer branch; it must hand the engine **both worlds** and let
the search pick.

### Conditional atoms (the primitive)

Atoms and the `delete()` marker gain a condition, keyed by a group's span the
same way softlines already carry their measure span by value:

```rust
// sugar — group span = the rule's context node, the common case:
last_item.append(states.literal_if_multiline(","));
comma.delete_if_single_line(states);

// explicit-span form, same convention as Atom::SpacedSoftline(range):
last_target.append(Atom::Literal(",").if_multiline(target_list_range));
```

During document construction, phase 2 gathers everything that shares a span —
softlines, conditional atoms, conditional deletes, and the deviation penalty —
into **one** `<|>` choice, so they all flip together by construction.

Important reading note: `if_multiline` refers to the **chosen variant of the
group**, not to the input. The words match the existing
`Selection::is_multiline()` vocabulary (and Topiary's `#multi_line_only!` /
`#single_line_only!` predicates, which this replaces), but the referent moved
from "what the author wrote" to "what the resolver picked".

There is deliberately no new "group" concept in the rule API. A `Selection`
is an ephemeral navigation handle; the group identity is the measure span it
already carries as context. What changes is engine-internal: today equal
spans agree trivially (each softline measures the same input independently);
under search, atoms sharing a span must be wired into the *same* choice
point. Internally a condition is stored as `(span, variant)` with variants
`Multiline`/`SingleLine` — an enum rather than a bool, so future n-ary
layout styles (e.g. fill-wrapped arrays) extend the engine without touching
the atom vocabulary.

One deliberate restriction keeps the rest of the architecture sound:
**conditions attach to `Literal` atoms and the `delete()` marker only** —
never to whitespace or indentation atoms. Whitespace already has its
conditional mechanism (the softline is *the* way a group controls
whitespace), and unconditional indent atoms are what make every potential
newline's indent computable before the search (see the implementation
sketch — the resolver's memo key depends on it). Two debug assertions
guard the boundary: a conditionally deleted token must not carry
`IndentStart`/`IndentEnd` atoms, and a `Literal`'s text must not contain a
newline.

Two engine obligations come with this:

- **Spans used as choice identities must nest or be disjoint** — a choice
  tree cannot represent partial overlap. Node-derived spans satisfy this
  automatically; hand-constructed spans (the `PropertyAnimation` target
  list) are covered by a debug assertion.
- **Any fixed newline inside a group's span removes the single-line
  variant** at document-build time: a line comment (flattening would
  swallow the rest of the line into it), an own-line comment (R1 forces
  its newline), a `Hardline`, a verbatim newline in a gap no rule engaged,
  or a multi-line block comment or `Leaf` range. This is a constraint (one
  branch deleted), not a condition, and is invisible to rules.

### `separated_by` (the sugar)

The trailing-separator idiom is pervasive, so it gets a one-line helper:

```rust
// FunctionCallExpression rule:
call.node(SyntaxKind::Expression).separated_by(SyntaxKind::Comma);
```

which expands to the primitives above: the separator boundaries join the
rule's group, the last item gets `Literal(",").if_multiline(group)`, and an
input trailing separator token gets `delete().if_single_line(group)`.
Because it expands to conditional atoms, correlation correctness and the
idempotency argument are inherited, proven once. Plain separator *spacing*
(`Antispace` before, softline after) is not this helper's job — the global
Tier-1 `Comma` token rule already does that.

`separated_by` always manages the trailing separator; there is no
`with_trailing()` builder and no no-trailing twin. That is justified by a
survey of the grammar (all parser loops checked):

- **Trailing comma accepted (12 lists):** arrays, object literals,
  function-call arguments, callback-connection parameters,
  callback-declaration parameters, function argument declarations, object
  types (structs reuse them), enum values, import lists, `uses` lists,
  export lists, gradient stops.
- **Trailing comma rejected (2 lists):** the `animate x, y` target list and
  `@tr(...)` arguments — in both, the parser demands another item after a
  comma.

The two exceptions are exactly the lists that would never call the helper: a
handful of short names that realistically never break (`animate`, already
the explicit-span escape-hatch case) and a call-shaped list expressible with
plain atoms if we ever want it to break. A helper *without* trailing
management would have no customers, since spacing is the global comma rule's
job. If a list ever wants group-coherent spacing without trailing insertion
(conceivable style call for gradient stops), a second function is added
then — the same deferral policy the main design applies to `first()`/`last()`.

### Rejected alternative: per-variant rule closures

A more general design would let a rule annotate each world separately:

```rust
states.variants(|flat, broken| {
    flat.token(SyntaxKind::Comma).delete();
    broken.node(SyntaxKind::State).append(Literal(","));
});
```

Inside each closure arbitrary Rust `if`s work again, and the branches may
differ in *anything*. Rejected for now: across the whole grammar, the branch
differences we actually need are insert-literal, delete-token, and spacing —
all expressible with conditional atoms — while the closure design costs
layered annotation sinks, layer multiplication under nested groups, and
nothing structurally prevents the two closures from drifting into
inconsistent pairs. It remains the known escape hatch if a rule ever needs
branches that differ beyond insert/delete/spacing.

## The algorithm, briefly

Expanding every `<|>` is exponential, so the printer resolves **bottom-up
with memoization**. The key idea is the **measure**: everything you need to
know about a candidate layout of a sub-document without rendering it —

1. the length of its last line (because that is the column where whatever
   comes *next* will start), and
2. its accumulated cost,

plus a pointer to the choiceless document that achieves it. Cost alone is not
enough: a slightly costlier layout with a shorter last line may win once the
suffix is appended. Neither component dominates the other, so per
sub-document the printer keeps the **Pareto frontier** — the candidates not
beaten on *both* axes by another candidate — and prunes the rest immediately.
The frontier is small: last-line lengths are distinct integers bounded by the
width limit, so it has at most W+1 entries.

Resolution is structural recursion: `text`/`nl` give a singleton set;
`a <> b` resolves `a`, then resolves `b` once per surviving end column of
`a`, adds costs, and merges; `a <|> b` resolves both sides and merges the two
frontiers; `nest`/`align` recurse with adjusted indentation. The memoization
key is **(sub-document identity, start column, indent level)**.

**Taintedness** is the load-bearing performance trick. The third input, the
computation width limit W (the paper defaults to 100 for a page width of 80 —
deliberately a little larger), bounds the search: the moment a resolution's
column or indentation exceeds W, the measure set collapses to a single
"tainted" lazily-evaluated candidate, picked greedily, with no optimality
claim. Merging prefers untainted sets, so tainted branches only survive when
*every* alternative blows past W — in which case the printer still produces
output rather than failing. Because columns and frontier sizes are both
bounded by W, the total time is **O(n · W⁴)** worst case (n = document size,
counting shared sub-documents once), and in practice fast: 10k-line JSON in
~7 ms, the largest files of the Racket codebase (~5–6k lines) in ~0.4 s. The
guarantee: the output is valid, and optimal among all layouts that stay
within W.

## How this fits our pipeline

Phases 1 and 3 are untouched; the change is confined to **phase 2**:

- Today, phase 2 is a single linear pass over the gaps: each softline reads
  its measure span's *input* multilineness and decides immediately.
- With this design, phase 2 instead *builds a document with open choices*
  (softline groups become `group`s, the trailing-comma pattern becomes a
  content-differing choice) and runs the paper's resolver over it. The output
  is the same `FormatPlan` as today — concrete decisions per gap — so phase 3
  and the `TokenWriter` protocol never know the difference.

Everything upstream also survives: rules, atoms, selections, markers stay the
formatter's vocabulary. It is specifically the *resolution* of softlines that
changes from "read the input" to "search for the cheapest output".

## Implementation sketch (pseudo-code)

Scope of this sketch: no `align`, no fill-wrapping — binary groups only.
`page_width` (where overflow starts to hurt) and `computation_width` (W, where
the search gives up optimizing) stay configurable parameters.

The pipeline gains two steps inside phase 2. Annotation, linearization and
rendering are untouched — but per-gap resolution is **not**: it moves inside
document construction and runs once per (gap, variant):

```text
annotate (unchanged)
  ──▶ linearize (unchanged)
  ──▶ NEW: build the choice document
        — per-gap resolution (tier merge, Antispace, comment sub-gaps,
          engagement) moves INTO this step, run once per (gap, variant)
  ──▶ NEW: resolve = search for the cheapest variant assignment
  ──▶ emit FormatPlan (as today, but reading group decisions from the search)
  ──▶ render (unchanged)
```

### Why per-gap resolution moves inside document construction

The base design resolves every gap once, in one linear pass. Under the
search, that pass would need the group decisions — which do not exist yet.
Three concrete dependencies force the per-variant split:

1. **Conditional deletes change which atoms meet in a gap.** A deleted
   token's own atoms are discarded, and the following gap sources its
   append-side atoms from the last *emitted* token. With
   `delete_if_single_line`, which token that is depends on the variant.
2. **Comment routing (R2) routes by resolved strength.** In the multiline
   variant a group's newline transfers past a trailing comment (`{ // note`
   keeps the comment hanging, the break lands after it); in the single-line
   variant the space stays before the comment. Even the *order* of
   whitespace and comment in the output differs per variant — and comment
   position affects line width, so none of this can wait until plan
   emission.
3. **Engagement is per variant.** A gap whose only atom is
   `Literal(",").if_multiline(…)` is engaged in that variant and verbatim
   in the other.

So the builder calls `resolve_gap(gap, variant)`: today's merge machinery —
tier-first/strength-second, Antispace cancellation, sub-gap splitting,
R1–R3 anchoring — with the controlling group's variant substituted for the
softline measurement. It returns the gap's document sequence (whitespace
and comment `Text` docs, in routed order). `InputSoftline` keeps its
abstention semantics for free: in the single-line variant it abstains and
the remaining atoms merge exactly as they do today.

### Which group controls a gap

Atoms with *different* spans can meet in one gap (the tier-1 comma softline
measures its context node; a tier-3 rule's softline may carry a hand-built
span). The controlling group is decided the way today's merge would decide
it, so the two designs agree wherever they overlap:

- The winning **tier** at a gap does not depend on any variant, so it is
  decided up front. If the winning tier contains a softline, the gap is
  group-controlled ("controlled" = its outcome varies with that group's
  variant); among several softlines in the winning tier, the one with the
  **innermost span** controls.
- An outer group's softline landing at a gap strictly inside an inner
  group's span is **captured** by the inner group: it breaks when the
  inner group goes multiline and flattens when it goes single-line.
- Two softlines whose spans neither nest nor are disjoint cannot be
  represented in a choice tree — debug-assert against this (no current
  rule produces it).
- A group can end up controlling no gap at all (everything captured by
  inner groups); its choice is degenerate and the builder drops it.

### Data types

```rust
/// Identity of one correlated choice. One per distinct span used by
/// softlines / conditional atoms. Spans must nest or be disjoint.
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
    /// contract — a plain per-piece square would violate it):
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

// Why the deviation constant is sound — the paper's verified optimality
// theorem does NOT cover per-site costs (its practical `cost` construct is
// explicitly left unformalized), so we owe our own short argument: each
// penalty is a constant added uniformly to every measure of one branch,
// BEFORE that branch is first merged (and therefore pruned) against its
// sibling. Adding a constant preserves domination between measures of the
// same branch (lexicographic order over the triple is translation-
// invariant), so no measure pruned inside a branch could have beaten a
// survivor once the penalty applies; and across branches the penalty is
// already in place when pruning first compares them.

/// The choice document. Built once per format run; nodes are referenced by
/// id so the resolver can memoize on them.
enum Doc {
    /// A SINGLE-LINE run of characters: a token, one line of a comment, a
    /// space, or a conditional literal's text — never contains a newline.
    /// Multi-line verbatim items (block comments, multi-line Leaf ranges)
    /// are expanded by the builder into
    ///   Text(first line) · Newline(fixed) · … · Text(last line)
    /// so their width and overflow are counted per line, and the column
    /// after them is the LAST line's length — one Text with the total
    /// width would compute garbage columns and taint spuriously.
    Text { source: SlotOrLiteral, width: u32 },
    /// A line break — either fixed (R1 comment newlines, Hardline, the
    /// verbatim newlines of unengaged gaps and Leaf ranges) or emitted by
    /// a group's multiline body. The indent is baked in at build time:
    /// indent atoms are never conditional (enforced in the dynamic-rules
    /// section), so a running counter over the boundaries fixes every
    /// potential newline's indent level before the search. (This is why
    /// the sketch needs no `nest` node and no indent in the memo key — a
    /// real simplification over the paper.)
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
(every distinct span carried by a softline or conditional atom, plus one
single-gap group per `InputSoftline`). The groups' spans form a forest by
nesting; tokens and gaps hang off the group that controls them (see "Which
group controls a gap" above).

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
            // EVERY gap goes through per-variant gap resolution (tier
            // merge, comments, sub-gaps, R1-R3 routing — see above). For a
            // gap controlled by this group, `variant` decides its softline;
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
/// ANY Newline doc, of whatever origin: a line comment, an own-line comment
/// (R1 forces its newline), a Hardline, a verbatim newline in an unengaged
/// gap, a multi-line block comment or Leaf range, or an inner group that
/// itself cannot flatten. Checking the BUILT body for Newline docs is
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
            // (paper Fig. 15, ConcatRS). Tainting the whole concat because
            // one too-wide prefix candidate existed would let an
            // irrelevant sibling flip an enclosing Choice.
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
/// width ascending; on equal cost AND width keep the earlier measure —
/// the deterministic tie-break idempotency relies on). Then keep a measure
/// iff its last_line_width is strictly smaller than every measure already
/// kept. Distinct integer widths in 0..=W bound the frontier at W+1
/// entries.
/// (Getting this backwards — walking width-descending and filtering for
/// DECREASING cost — silently keeps only the cheapest measure and discards
/// the short-last-line candidates the frontier exists for. Unit-test this
/// function against hand-built frontiers.)
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
    // (Flattening never reduces overflow, so such a candidate always
    // carries the ancestor's own penalty when it deviates — the undercount
    // cannot flip a decision by itself, but it IS a divergence from the
    // literal "number of groups that flip the author's layout" wording.)
}
```

### Notes for the implementer

- **Indent is static — but only because the dynamic-rules section enforces
  it.** Conditions exist on `Literal` and `delete()` only, never on
  whitespace or indent atoms, and a conditionally deleted token must not
  carry `IndentStart`/`IndentEnd` (debug assertion). Those restrictions
  are what license the missing `nest` construct and the `(doc, column)`
  memo key — the paper needs `(doc, column, indent)`.
- **Blank-line upgrades and own-line-comment re-indentation** stay
  engine-core mechanics at plan-emission time. Comments participate in the
  search as `Text` docs (width counts toward overflow) and `Newline` docs
  (which also forbid flattening); `height` counts Newline docs, so a
  blank-line upgrade is one unit like any other — an accepted, documented
  approximation.
- **Taint bias**: ordering the input-matching variant left means "all
  options overflow W" degrades to the author's layout, which is what the
  formatter does today.
- **Laziness matters.** `Tainted` must stay unevaluated unless it is the
  only option — including through `concat_sets`, where combining goes
  *inside* the returned Lazy. That is what bounds work past the width
  limit (the paper's Section 6.3).
- **Cost::text must telescope.** The squared-excess-difference form is not
  optional; a naive per-piece square breaks the "splitting a string costs
  the same as placing it whole" contract and with it the optimality proof.
  Likewise `Cost::newline` must charge its indent, or overflow at deep
  indentation becomes invisible to the search.
- **Debug assertions**: group spans nest or are disjoint; no conditional
  indent atoms; no newline inside a `Literal`'s text; a chosen SingleLine
  variant renders without newlines (the successor of the base design's
  "no Hardline inside a single-line softline span" assertion); decision
  lists concatenate disjointly.

## Open questions (to resolve in the concrete plan)

- **Comment geometry across runs.** How comments enter the search is now
  specified (they become `Text`/`Newline` docs, `resolve_gap` runs the
  R1–R3 anchoring once per variant, and any comment-forced newline forbids
  flattening). What remains open is *cross-run stability*: R2 routing can
  move a group's newline past a trailing comment, so the second run sees
  different sub-gap shapes than the first — the idempotency proof does not
  cover this. Either show the routed shape is a fixpoint (corpus check),
  or make comment-adjacent group boundaries resolve identically in both
  runs by construction. Decide during implementation.
- **`align`.** The paper supports column alignment; our design has only
  indent levels. Do we want it? (Nothing forces us to adopt it.)
- **Page width and W.** What page width do we format to, and what
  computation width limit W (the paper uses W = page width + 25%)?
- **Fill-wrapped lists.** Long literal arrays (model data) could pack as
  many items per line as fit, like the paper's `fillSep`. This is a
  document-construction pattern (a chain of per-gap choices), not a new
  condition kind, and today's formatter does not fill either — deferrable
  without regression.
