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

Why this matters for JSON: rustdoc JSON only guarantees **full `Item` entries
(including the `docs` string) for items local to the crate being documented.**
When `slint` re-exports an item that is *defined in another crate*, that item
frequently appears only as:

- a `Use` item in `slint`'s `index` whose target `Id` is **not present** in
  `slint`'s `index`, and
- an entry in `paths` (`ItemSummary { crate_id, path, kind }`) plus an
  `external_crates` entry giving the originating crate's name.

So the doc text and member details for those re-exported types **do not live in
`slint.json`** — they live in the JSON of the originating crate. Glob re-exports
(`pub use …::*`) make this worse: rustdoc cannot even enumerate the members from
`slint.json` alone.

**Conclusion:** generating `slint.json` is necessary but not sufficient. We must
also generate rustdoc JSON for every originating workspace crate and **stitch the
crates together**, resolving each re-export target by fully-qualified path into
the crate that actually defines it.

## High-level approach

1. Generate rustdoc JSON for `slint` **and** each originating workspace crate.
2. Load all JSON files into a multi-crate index keyed by `(crate, Id)`, plus a
   path-based lookup so cross-crate references can be resolved by name.
3. Walk the **public API as `slint` exposes it** (its module tree, including the
   synthetic modules like `platform`, `software_renderer`, `winit_030`,
   `fontique_010`), following `Use` items into the defining crate when the target
   is external.
4. Render each item to Markdown from the rustdoc `Item` (the `docs` field is
   already CommonMark), resolving intra-doc links to our own slugs.
5. Write a Markdown tree plus a manifest, ready for `llms.txt`/MCP aggregation.

## Detailed steps

### Phase 1 — Generate rustdoc JSON

- rustdoc JSON is **nightly-only** and gated behind `-Z unstable-options
  --output-format json`. CI already uses the nightly toolchain in the
  `rust-cpp-docs` job of `.github/workflows/build_docs.yaml`, right next to the
  existing `cargo doc -p slint -p slint-build -p slint-interpreter` step — this is
  where the new step belongs.
- Command shape (per crate):
  `cargo +nightly rustdoc -p <crate> --all-features -- -Z unstable-options --output-format json --document-private-items=false`
  emitting `target/doc/<crate_underscored>.json`.
- Match the flags the HTML docs already use so content lines up: `--cfg docsrs`
  and the feature set used by the docs job (`--all-features` for `slint`).
- Crates to document (the façade + every originating crate from the table):
  `slint`, `i-slint-core`, `i-slint-backend-selector`, `i-slint-renderer-software`,
  `i-slint-renderer-femtovg`, `i-slint-renderer-skia`, `i-slint-backend-winit`,
  `i-slint-common`, `slint-macros`. Optionally also `slint-build` and
  `slint-interpreter`, which the HTML docs job already publishes.
- **Target caveat:** the `winit_030` and `android` modules are documented in CI
  under the `aarch64-linux-android` target (see the second `cargo doc` invocation
  in `build_docs.yaml`). The generator must allow per-crate target/feature
  overrides and run a second android-target pass for `i-slint-backend-winit`
  (and android-activity) the same way.
- Pin the nightly + the JSON `FORMAT_VERSION`: the schema is unstable, so the
  parser must assert the version it was built against and fail loudly on a
  mismatch.

### Phase 2 — Multi-crate index

- Parse each file as a `rustdoc_types::Crate` (use the `rustdoc-types` crate,
  whose `FORMAT_VERSION` constant pins the schema).
- rustdoc `Id`s are **per-file, not globally unique**, so key everything by
  `(crate_name, Id)`.
- Build a **path index**: from each crate's `paths`, map fully-qualified path
  (`Vec<String>`) → `(crate_name, Id)`. Resolving a cross-crate re-export means:
  read the target's `ItemSummary` from `slint`'s `paths`, get the originating
  crate name via `external_crates[crate_id]`, then look the path up in that
  crate's path index to find the real, fully-populated `Item`.

### Phase 3 — Walk the public API of `slint`

- Start at `slint.json`'s `root` module and recurse.
- For each child:
  - **Local item** (`Id` in `slint`'s `index`): render directly.
  - **`Use` / re-export** whose target is local: follow within `slint`.
  - **`Use` / re-export** whose target is external (not in `index`): resolve via
    Phase 2 into the defining crate and render from *there*, but keep the
    **path/slug as `slint` exposes it** (e.g. `slint::Model`, not
    `i_slint_core::model::Model`). Glob re-exports (`*`) expand to every public
    item of the referenced module in the originating crate.
- Respect `#[doc(hidden)]` and `#[doc(inline)]`/`no_inline` semantics, and skip
  `deprecated`-but-hidden compatibility shims (e.g. the deprecated
  `ComponentFactory`, `StandardListViewItem`, `TableColumn` re-exports) the same
  way the HTML docs hide them.

### Phase 4 — Render Markdown

- For each item emit: kind + signature (struct/enum/trait/fn/typedef/const),
  the `docs` body (already CommonMark — pass through), generics/bounds, fields,
  variants, associated items, and notable trait impls.
- Resolve **intra-doc links**: rustdoc JSON records link targets in each item's
  `links` map (text → `Id`). Rewrite those to our generated slugs; leave external
  `docs.rs` links intact (the existing job already sets
  `--extern-html-root-url` for `rgb`, `winit`, `wgpu`, etc.).
- Stable slugs/anchors so the MCP and `llms.txt` can deep-link. Reuse the slug
  conventions from `docs/cpp/scripts/lib/slug.ts` for consistency across C++/Rust.

### Phase 5 — Output & manifest

- Write one Markdown page per module (or per item, TBD — see open questions) into
  a generated dir, e.g. `target/rustdoc-md/` or under the Astro content tree,
  matching the C++ converter's `{ pages, sidebar }` shape.
- Emit a **manifest** (JSON) listing pages, slugs, titles, and source crate, to
  drive later `llms.txt` aggregation and MCP indexing.

### Phase 6 — Tooling, integration & tests

- **Where the tool lives:** recommend a small Rust binary using `rustdoc-types`
  (the canonical typed schema), either as a new subcommand of
  `docs/slint-doc-generator` or a focused `tools/rustdoc-md` crate. Rust is
  preferred over a TS converter here because `rustdoc-types` gives a typed,
  version-pinned schema; the C++ side used TS only because Doxygen emits XML.
- **CI:** add a step to the `rust-cpp-docs` job in `build_docs.yaml` after the
  existing `cargo doc -p slint …` step, running the generator and uploading the
  Markdown tree as part of the `docs-rust-cpp` artifact.
- **Tests:** unit-test the converter against a small checked-in fixture JSON
  (the analogue of `docs/cpp/tests/convert.test.ts`), covering: a local item, a
  single-item cross-crate re-export, and a glob cross-crate re-export.

## Open questions / decisions to confirm

1. **Granularity:** one `.md` per module (fewer, larger files, friendlier to
   `llms-full.txt`) vs. one per item (better for MCP deep-links)? Lean: per
   module + a combined `llms-full` concatenation.
2. **Output location:** standalone `target/rustdoc-md/` artifact vs. inside the
   Astro content collection. Lean: standalone artifact first; wiring into the
   site/MCP is the "later" step.
3. **Feature/target matrix:** confirm we only need `--all-features` host +
   android pass (matching today's HTML docs), or whether wgpu/skia variants need
   separate passes.
4. **Scope:** `slint` only, or also `slint-build` and `slint-interpreter` (both
   already in the HTML docs job)? Lean: include all three since the JSON+stitch
   machinery is identical.
5. **Link policy:** rewrite intra-doc links to local slugs (richer) vs. strip to
   plain text (simpler, more robust for `llms.txt`).

## Risks

- rustdoc JSON schema is unstable across nightlies — pin nightly + `rustdoc-types`
  and assert `FORMAT_VERSION`.
- Cross-crate glob re-export resolution is the main complexity; the multi-crate
  path index in Phase 2 is what de-risks it.
- Feature/`cfg` skew between the façade crate and originating crates can hide or
  expose items inconsistently; document each crate with a feature set consistent
  with how `slint` re-exports it.
