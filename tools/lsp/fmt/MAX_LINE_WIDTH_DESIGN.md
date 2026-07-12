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

Idempotency, which today rests on an informal argument, becomes provable. On
a second run the chosen layout *is* the input, so its deviation is 0. Any
competitor either has worse overflow (loses at tier 1 — a layout's overflow
does not depend on the input), or agrees with the chosen layout on every
penalized group, in which case it already competed on equal terms in the
first run and lost on height. So the first run's winner wins again. The one
requirement is deterministic tie-breaking, which the paper's left-biased
merge already provides.

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

Two engine obligations come with this:

- **Spans used as choice identities must nest or be disjoint** — a choice
  tree cannot represent partial overlap. Node-derived spans satisfy this
  automatically; hand-constructed spans (the `PropertyAnimation` target
  list) are covered by a debug assertion.
- **A line comment inside a group's span removes the single-line variant**
  at document-build time — flattening would swallow the rest of the line
  into the comment. This is a constraint (one branch deleted), not a
  condition, and is invisible to rules.

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

The pipeline gains two steps inside phase 2; everything else is unchanged:

```text
annotate (unchanged)
  ──▶ linearize (unchanged)
  ──▶ per-gap tier merge & comment sub-gaps (unchanged)
  ──▶ NEW: build the choice document
  ──▶ NEW: resolve = search for the cheapest variant assignment
  ──▶ emit FormatPlan (as today, but reading group decisions from the search)
  ──▶ render (unchanged)
```

### Data types

```rust
/// Identity of one correlated choice. One per distinct span used by
/// softlines / conditional atoms. Spans must nest or be disjoint.
struct GroupId(u32);
enum Variant { SingleLine, Multiline }

/// The lexicographic cost triple. Compared field by field, in this order.
struct Cost {
    overflow: u64,   // sum of squared characters past page_width
    deviation: u32,  // number of groups decided against the input
    height: u32,     // number of newlines
}

impl Cost {
    fn add(a: Cost, b: Cost) -> Cost { /* component-wise sum */ }
    fn less_than(a: Cost, b: Cost) -> bool { /* lexicographic */ }

    /// Cost of placing `length` characters starting at `column`.
    /// IMPORTANT: defined as a difference so that placing a line in pieces
    /// costs the same as placing it at once (the cost-factory "splitting"
    /// contract — a plain per-piece square would violate it):
    fn text(column: u32, length: u32) -> Cost {
        fn squared_excess(x: u32) -> u64 { max(x - page_width, 0)^2 }
        Cost { overflow: squared_excess(column + length) - squared_excess(column), ..zero }
    }
    fn newline() -> Cost { Cost { height: 1, ..zero } }
    fn deviation() -> Cost { Cost { deviation: 1, ..zero } }
}

/// The choice document. A tree built once per format run; nodes are
/// referenced by id so the resolver can memoize on them.
enum Doc {
    /// A significant token or a comment (anything with a width),
    /// referencing the linearization. Conditional literals appear as
    /// `Text` too, included only in the variant they belong to.
    Text { slot: SlotRef, width: u32 },
    /// A decided line break. The indent is baked in at build time: indent
    /// atoms are unconditional, so a running counter over the boundaries
    /// fixes every potential newline's indent level before the search.
    /// (This is why the sketch needs no `nest` node and no indent in the
    /// memo key — a real simplification over the paper.)
    Newline { indent_width: u32 },
    Space,
    Concat(Vec<DocId>),
    /// One group. `single_line` is None when flattening is forbidden
    /// (a line comment or a Hardline inside the span).
    Choice {
        group: GroupId,
        single_line: Option<DocId>,
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
    /// The decisions taken so far: GroupId -> Variant. A persistent
    /// (structurally shared) map — measures fork off each other constantly.
    decisions: SharedMap<GroupId, Variant>,
}

/// A Pareto frontier, sorted by last_line_width descending / cost strictly
/// ascending — or a single lazily computed fallback once we blew past W.
enum MeasureSet {
    Set(Vec<Measure>),          // at most W+1 entries, see dedup()
    Tainted(Lazy<Measure>),     // greedy, no optimality claim
}
```

### Building the document

Input: the token slots, the per-gap merged atom decisions (existing
machinery), and the set of groups (every distinct span carried by a softline
or conditional atom, plus one single-gap group per `InputSoftline`). The
groups' spans form a forest by nesting; tokens and fixed gaps hang off the
innermost group containing them.

```rust
fn build_document() -> DocId {
    // Walk the span forest from the root. Each group builds exactly two
    // docs — its multiline body and its flattened body — each built once
    // and shared, so the document stays linear in the input size.
    build_group(root_span)
}

fn build_group(group) -> DocId {
    let multiline = build_body(group, Variant::Multiline);
    match build_flat(group) {
        // Flattening forbidden: not a real choice, just use the broken form.
        None => multiline,
        Some(single_line) => alloc(Doc::Choice {
            group: group.id,
            single_line: Some(single_line),
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
            Token(slot)      => parts.push(text_doc(slot)),
            Comment(slot)    => parts.push(text_doc(slot)),
            // A conditional atom / delete contributes only to its variant:
            ConditionalLiteral(text, for_variant) if for_variant == variant
                             => parts.push(literal_doc(text)),
            DeletedToken(slot, for_variant)
                             => if for_variant != variant { parts.push(text_doc(slot)) },
            // Gaps controlled by THIS group follow the variant:
            GroupGap(gap)    => parts.push(match variant {
                Variant::Multiline  => alloc(Doc::Newline { indent_width: gap.indent }),
                Variant::SingleLine => flat_whitespace(gap), // Space or nothing
            }),
            // Fixed gaps keep their already-decided whitespace:
            FixedGap(gap)    => parts.push(fixed_whitespace_doc(gap)),
            // Inner groups stay open choices in the multiline body:
            InnerGroup(g)    => parts.push(build_group(g)),
        }
    }
    alloc(Doc::Concat(parts))
}

/// The single-line body: like build_body(SingleLine), but inner groups are
/// forced single-line recursively (a newline inside a flattened span is
/// impossible). Returns None if the span contains a line comment, a
/// Hardline, or an inner group that itself cannot flatten.
fn build_flat(group) -> Option<DocId>
```

### The resolver (the search)

```rust
/// Memoized on (doc, column). Column is capped by W, so the table stays
/// small; sharing (each group's two bodies built once) keeps it linear in
/// the document.
fn resolve(doc: DocId, column: u32) -> MeasureSet {
    match doc {
        Doc::Text { width, .. } | Doc::Space => {
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
            set_of_one(Measure { last_line_width: indent_width, cost: Cost::newline(), .. })
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
            let flat = single_line.map(|d| {
                resolve(d, column)
                    .tag(group, Variant::SingleLine)          // record the decision
                    .penalize_if(penalized == Variant::SingleLine)
            });
            let broken = resolve(multiline, column)
                .tag(group, Variant::Multiline)
                .penalize_if(penalized == Variant::Multiline);
            // Put the input-matching side LEFT: merge is left-biased when
            // both are tainted, so a hopeless overflow degrades to "keep
            // the author's layout" — today's behavior.
            merge(in_input_order(flat, broken))
        }
    }
}

fn concat_sets(left: MeasureSet, right_doc: DocId) -> MeasureSet {
    match left {
        Set(measures) => {
            // Resolve the suffix once per distinct prefix end-column,
            // combine, then re-establish the Pareto frontier.
            let combined = measures.flat_map(|m| {
                match resolve(right_doc, m.last_line_width) {
                    Set(suffixes) => suffixes.map(|s| combine(m, s)),
                    Tainted(lazy) => /* whole result becomes Tainted(combine(m, lazy)) */,
                }
            });
            dedup(combined)
        }
        Tainted(lazy_prefix) => Tainted(/* force prefix, resolve suffix greedily, combine */),
    }
}

fn combine(prefix: Measure, suffix: Measure) -> Measure {
    Measure {
        last_line_width: suffix.last_line_width,
        cost: Cost::add(prefix.cost, suffix.cost),
        decisions: prefix.decisions.union(suffix.decisions), // disjoint by construction
    }
}

/// Keep only the Pareto frontier: sort by last_line_width descending,
/// keep entries whose cost strictly decreases. Distinct integer widths
/// in 0..=W bound the frontier at W+1 entries.
fn dedup(measures: Vec<Measure>) -> MeasureSet

/// Merge two frontiers (mergesort-style + dedup). Prefer Set over
/// Tainted; if both are Tainted, keep the LEFT one (greedy fallback).
fn merge(a: MeasureSet, b: MeasureSet) -> MeasureSet
```

### Top level and plan emission

```rust
fn resolve_widths(document_root: DocId) -> SharedMap<GroupId, Variant> {
    match resolve(document_root, /* column */ 0) {
        Set(measures)  => measures.min_by(cost).decisions,  // provably optimal within W
        Tainted(lazy)  => lazy.force().decisions,           // best effort, still valid
    }
}

fn emit_plan(decisions) -> FormatPlan {
    // Exactly today's phase-2 output step, except a group-controlled gap
    // asks `decisions[group]` instead of measuring input multilineness:
    //   Multiline  -> ReplaceGap Newline { indent level, blank-line upgrade
    //                 if AllowBlankLines and the input had one }
    //   SingleLine -> ReplaceGap Space / None
    // Conditional literals emit only in their variant; a conditionally
    // deleted token becomes DeleteToken only in its variant.
    // Fixed gaps, comment sub-gaps, Leaf ranges: unchanged.
}
```

### Notes for the implementer

- **Indent is static.** Indent atoms are unconditional, so every potential
  newline's indent level is known before the search. That removes the
  paper's `nest`/`align` constructs and the indent component of the memo
  key — our memo key is `(doc, column)` instead of the paper's
  `(doc, column, indent)`.
- **Blank lines and comments** stay engine-core mechanics at plan-emission
  time, outside the search. Comments participate only as `Text` (their
  width counts toward overflow) and as the flatten-forbidder.
- **Taint bias**: ordering the input-matching variant left means "all
  options overflow W" degrades to the author's layout, which is what the
  formatter does today.
- **Laziness matters.** `Tainted` must stay unevaluated unless it is the
  only option — that is what bounds work past the width limit (the paper's
  Section 6.3).
- **Cost::text must telescope.** The squared-excess-difference form is not
  optional; a naive per-piece square breaks the "splitting a string costs
  the same as placing it whole" contract and with it the optimality proof.
- **Debug assertions**: group spans nest or are disjoint; `Hardline` only
  at Document top level or inside never-flattened groups; decision maps
  union disjointly.

## Open questions (to resolve in the concrete plan)

- **Comments.** Line width obviously must include comments, so they need to
  enter the measured document as `text`, and the R1–R3 anchoring rules must
  compose with choice resolution. (One piece is decided: a line comment
  inside a group's span removes the single-line variant, see the dynamic
  rules section.)
- **`align`.** The paper supports column alignment; our design has only
  indent levels. Do we want it? (Nothing forces us to adopt it.)
- **Page width and W.** What page width do we format to, and what
  computation width limit W (the paper uses W = page width + 25%)?
- **Fill-wrapped lists.** Long literal arrays (model data) could pack as
  many items per line as fit, like the paper's `fillSep`. This is a
  document-construction pattern (a chain of per-gap choices), not a new
  condition kind, and today's formatter does not fill either — deferrable
  without regression.
