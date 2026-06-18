<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Plan: Markdown content for the Rust API (rustdoc → Markdown)

## Goal

Produce **Markdown** for the public Rust API of the `slint` crate so it can later
be consumed by:

- an `llms.txt` / `llms-full.txt` aggregation for the docs site, and
- the documentation MCP server.

The source of truth is **rustdoc's JSON output** (`--output-format json`), not the
rendered HTML. We convert that JSON into a stable, link-resolved Markdown tree.

This mirrors the existing C++ pipeline, which converts **Doxygen XML → Markdown**
for Starlight (`docs/cpp/scripts/generate-api.ts`,
`docs/cpp/scripts/lib/doxygen.ts`). The Rust analogue is **rustdoc JSON → Markdown**.

## The core problem: the public API is re-exports from other crates

`api/rs/slint/lib.rs` is mostly a façade. Its public surface is assembled from
`pub use` re-exports of *other workspace crates*:

| Re-export in `slint` | Originating crate |
| --- | --- |
| `pub use i_slint_core::api::*`, `model::*`, `timers::*`, … | `i-slint-core` |
| `pub use i_slint_backend_selector::api::*` | `i-slint-backend-selector` |
| `pub use slint_macros::slint` | `slint-macros` |
| `platform::software_renderer` (`i_slint_renderer_software::*`) | `i-slint-renderer-software` |
| `platform::femtovg_renderer` | `i-slint-renderer-femtovg` |
| `platform::skia_renderer` | `i-slint-renderer-skia` |
| `winit_030::*` | `i-slint-backend-winit` |
| `fontique_010::fontique` | `i-slint-common` |
| `platform::*` | `i-slint-core::platform` |

rustdoc JSON only emits full `Item` entries — including the `docs` string — for
items **local to the crate being documented**. Re-exported foreign items are *not*
inlined into `slint.json`. **Conclusion:** generating `slint.json` is necessary
but not sufficient — we must also generate rustdoc JSON for every originating
workspace crate and **stitch the crates together**, resolving each re-export
target by fully-qualified path into the crate that defines it.

## Verified behavior (empirical, not assumed)

This was validated with a minimal two-crate example (`facade` re-exporting from
`dep_crate`) using `cargo +nightly rustdoc -- -Z unstable-options
--output-format json` (nightly 1.98.0, **`FORMAT_VERSION = 57`**):

1. **Re-exported foreign items are absent from the façade index.** `facade.json`'s
   `index` contained only the local `LocalThing` (plus blanket-impl noise like
   `From`/`Into`). The re-exported `Widget`, its `width` field, the `Drawable`
   trait, and the glob'd `Helper` were **not** in the index at all.
2. **Each re-export is a `use` item** in the index carrying: `name`, `is_glob`
   (bool), a `source` string (e.g. `"dep_crate::Drawable"`), and a target `id`.
   That target `id` is **not** in the façade's `index`, but **is** in `paths`.
3. **`paths[id]` gives `{ crate_id, kind, path }`** — e.g.
   `{crate_id: 20, kind: "struct", path: ["dep_crate","Widget"]}` — and
   **`external_crates[crate_id]`** gives the originating crate `name`
   (and its `.rmeta` path).
4. **Resolution works against the originating crate's own JSON.** Generating
   `dep_crate.json` and indexing its `paths` where `crate_id == 0` (local) yields
   a `path → id` map; `["dep_crate","Widget"]` resolves to the fully-populated
   `Item` with `docs: "Documentation for Widget…"` and a `width` field whose own
   `docs` is present.
5. **Glob re-exports point at the module, not its members.** `pub use
   dep_crate::sub::*` is one `use` item with `is_glob: true` targeting the
   `dep_crate::sub` *module*; members (`Helper`) are enumerated from the module's
   `items` list **in the originating crate's JSON**.
6. **Intra-doc links live in a per-item `links` map** (`text → Id`), e.g.
   `Drawable.docs = "A trait [\`Widget\`] interacts with."` with
   `links = {"\`Widget\`": <id>}`. The `Id` is in the **originating crate's**
   id-space, so link rewriting must be done in that crate's context.

## Stitching algorithm (the heart of the tool)

```
load each <crate>.json as rustdoc_types::Crate; assert FORMAT_VERSION matches.
for each crate file, build:
    index_by_id:   Id            -> Item          (local index)
    path_to_id:    Vec<String>   -> Id            (from `paths` where crate_id == 0)
    name_of_crate: from the file we are parsing

resolve(use_item, in_crate):
    target_id = use_item.id
    if target_id in in_crate.index:                 # local re-export
        item = in_crate.index[target_id]
    else:                                            # cross-crate re-export
        summary   = in_crate.paths[target_id]        # { crate_id, kind, path }
        crate_nm  = in_crate.external_crates[summary.crate_id].name
        defining  = crates[crate_nm]                 # the JSON we generated for it
        item      = defining.index[ defining.path_to_id[summary.path] ]
    if item is itself a `use` (transitive re-export):   # e.g. slint -> selector -> core
        return resolve(item, defining)               # follow the chain to the definer
    return (defining_crate, item)

expand_glob(use_item, in_crate):                     # is_glob == true
    (defining, module) = resolve(use_item, in_crate) # module item
    return [ defining.index[child] for child in module.inner.module.items ]
```

Notes:

- **Match by path, never by `Id`** — `Id`s are per-file and not comparable across
  JSON files. The `path_to_id` map keyed on the fully-qualified path is the bridge.
- **Transitive chains** must be followed: `slint` re-exports
  `i_slint_backend_selector::api::*`, and selector may itself re-export from
  `i-slint-core`. Keep resolving while the landed item is another `use`.
- **External (non-workspace) targets** (e.g. `winit`, `wgpu`, `raw_window_handle`)
  have no JSON we generate; render them as a link to `docs.rs` using the same
  `--extern-html-root-url` mapping the HTML docs job already configures.

## Detailed steps

### Phase 1 — Generate rustdoc JSON

- rustdoc JSON is **nightly-only**, gated behind `-Z unstable-options
  --output-format json`. CI already uses nightly in the `rust-cpp-docs` job of
  `.github/workflows/build_docs.yaml`, next to
  `cargo doc -p slint -p slint-build -p slint-interpreter` — this is where the new
  step belongs.
- Command shape (per crate):
  `cargo +nightly rustdoc -p <crate> --all-features -- -Z unstable-options --output-format json`
  emitting `target/doc/<crate_underscored>.json`. (Note: `cargo rustdoc` has no
  `--no-deps`; the JSON only contains the documented crate regardless.)
- Match the HTML docs flags so content lines up: `--cfg docsrs` and the same
  feature set (`--all-features` for `slint`).
- Crates to document (façade + every originating crate): `slint`, `i-slint-core`,
  `i-slint-backend-selector`, `i-slint-renderer-software`, `i-slint-renderer-femtovg`,
  `i-slint-renderer-skia`, `i-slint-backend-winit`, `i-slint-common`, `slint-macros`
  (plus `slint-build` and `slint-interpreter`, already in the HTML docs job).
- **Target caveat:** `winit_030` and `android` are documented in CI under the
  `aarch64-linux-android` target (the second `cargo doc` in `build_docs.yaml`).
  The generator needs per-crate target/feature overrides and a second android pass
  for `i-slint-backend-winit` / android-activity.
- **Pin the schema:** rustdoc JSON is unstable (currently `FORMAT_VERSION = 57`).
  The parser must assert `rustdoc_types::FORMAT_VERSION` equals the producer's and
  fail loudly on mismatch; pin the nightly date and the `rustdoc-types` version
  together.

### Phase 2 — Multi-crate index

Build the `index_by_id`, `path_to_id`, and `external_crates` maps per crate as in
the algorithm above. Key the global picture by `(crate_name, path)`.

### Phase 3 — Walk the public API of `slint`

- Start at `slint.json`'s `root` module and recurse over module `items`.
- **Local item:** render directly.
- **`use` (re-export):** `resolve()` / `expand_glob()` into the defining crate,
  but keep the **slug as `slint` exposes it** (e.g. `slint::Model`, not
  `i_slint_core::model::Model`), preserving `slint`'s synthetic module tree
  (`platform`, `software_renderer`, `winit_030`, `fontique_010`, …).
- Respect `#[doc(hidden)]` and `no_inline`, and skip the deprecated-and-hidden
  compatibility shims (`ComponentFactory`, `StandardListViewItem`, `TableColumn`)
  the same way the HTML docs hide them (these are flagged via the item's
  `attrs`/`deprecation` fields).

### Phase 4 — Render Markdown

- Per item: kind + signature, the `docs` body (already CommonMark — pass through),
  generics/bounds, fields, variants, associated items, notable trait impls.
- **Link rewriting:** resolve each item's `links` map **in the crate that authored
  the doc string** (point 6 above), then rewrite to our generated slug; leave
  external `docs.rs` links intact.
- Stable slugs/anchors for deep-linking; reuse the slug conventions from
  `docs/cpp/scripts/lib/slug.ts` for cross-language consistency.

### Phase 5 — Output & manifest

- One Markdown page per module (see decisions) into a generated dir
  (e.g. `target/rustdoc-md/`), matching the C++ converter's `{ pages, sidebar }`
  shape, plus a **JSON manifest** (slug, title, source crate, item kind) to drive
  later `llms.txt` aggregation and MCP indexing.

### Phase 6 — Tooling, integration & tests

- **Tool placement:** a small Rust binary using the **`rustdoc-types`** crate (the
  canonical, version-pinned typed schema), as a new subcommand of
  `docs/slint-doc-generator` or a focused `tools/rustdoc-md` crate. Rust is
  preferred over a TS converter here precisely because `rustdoc-types` pins the
  schema; the C++ side used TS only because Doxygen emits XML.
- **CI:** add a step to the `rust-cpp-docs` job after the existing
  `cargo doc -p slint …` step, running the generator and folding the Markdown tree
  into the `docs-rust-cpp` artifact.
- **Tests:** unit-test the converter against small checked-in fixture JSON (the
  analogue of `docs/cpp/tests/convert.test.ts`), covering exactly the three cases
  proven above: a local item, a single-item cross-crate re-export, and a glob
  cross-crate re-export — plus a transitive (two-hop) re-export.

## Decisions (firmed up)

1. **Granularity:** one `.md` per module, plus a concatenated `llms-full.md`.
   (Per-item is overkill for files; deep-links are handled by stable anchors.)
2. **Output location:** standalone `target/rustdoc-md/` artifact first; wiring into
   the Astro site / MCP is the explicitly "later" step.
3. **Scope:** include `slint`, `slint-build`, and `slint-interpreter` — all already
   in the HTML docs job, and the stitch machinery is identical.
4. **Link policy:** rewrite intra-doc links to local slugs (richer for the MCP);
   fall back to `docs.rs` for non-workspace targets.

## Open question to confirm with maintainers

- **Feature/target matrix:** is `--all-features` host + the android pass (matching
  today's HTML docs) sufficient, or do wgpu/skia variants need separate passes to
  surface items hidden behind those `cfg`s?

## Risks

- rustdoc JSON schema is unstable — mitigated by pinning nightly + `rustdoc-types`
  and asserting `FORMAT_VERSION` (verified at 57).
- Transitive/glob cross-crate resolution is the main complexity — de-risked by the
  path-keyed multi-crate index and the `resolve()` chain above.
- Feature/`cfg` skew between the façade and originating crates can hide or expose
  items inconsistently; document each crate with a feature set consistent with how
  `slint` re-exports it.
