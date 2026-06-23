<!-- cSpell:ignore Doxyfile predev -->

# Slint C++ API documentation

Astro [Starlight](https://starlight.astro.build/) site for the Slint C++ API.
The API reference is generated from the C++ headers by running Doxygen (XML
output) and converting that XML to Markdown with a small in-repo tool — the
C++ counterpart of `starlight-typedoc`.

Content lives in `src/content/docs/`. The API reference under
`src/content/docs/api/` and the third-party license page under
`src/content/docs/generated/` are generated (both gitignored).

## How the API reference is generated

```
C++ headers ──doxygen (XML)──▶ target/cppdocs/xml ──converter──▶ src/content/docs/api/*.md ──astro──▶ dist/
```

- `Doxyfile` configures Doxygen to emit XML only (no HTML/LaTeX), via its
  `INPUT`/`EXCLUDE`/`PREDEFINED`/`WARN_AS_ERROR` settings.
- `scripts/generate-api.ts` runs Doxygen, then reads the Doxygen XML and writes
  one Markdown page per type/namespace into `src/content/docs/api/`. The pages
  are organized by namespace rather than by kind: a namespace is a directory and
  every type lives under its enclosing namespace, so classes and structs are not
  separated (in C++ the distinction is immaterial to users). For example
  `slint` → `api/slint`, `slint::Color` → `api/slint/color`,
  `slint::interpreter::Value` → `api/slint/interpreter/value`. It also writes
  `src/api-sidebar.generated.mjs`, the API-reference sidebar tree (organized by
  namespace, listing each namespace's types and linking to its free
  functions/enums), which `astro.config.mjs` imports. The root `slint` namespace
  is implicit, so its contents sit directly under "API Reference"; nested
  namespaces (`interpreter`, `platform`, …) are sub-groups. The conversion logic is in `scripts/lib/`:
  - `xml.ts` — a tiny dependency-free XML parser (Doxygen XML is machine
    generated, so a full XML library isn't needed).
  - `slug.ts` — namespace-rooted page slugs (`api/slint/color`) and member anchors.
  - `doxygen.ts` — the model + Markdown renderer (signatures, descriptions,
    parameter/return docs, notes → Starlight asides, code listings, enum value
    tables, and cross-references resolved to root-relative links) plus the
    namespace-organized sidebar tree.

The converter is dependency-free and unit-tested against a fixture:

```sh
pnpm -C docs/cpp test     # node --test, no extra deps
```

## Prerequisites

- [Node.js](https://nodejs.org/) (v22+) and [pnpm](https://pnpm.io/)
- [Doxygen](https://www.doxygen.nl/) for `gen:api`
- A Rust toolchain for `gen:api` (it runs `cargo xtask generate_cppdocs_headers`
  to produce the cbindgen headers) and for `thirdparty` (which runs
  `cargo xtask license` to produce the third-party license list)

`gen:api` generates the cbindgen headers itself and points Doxygen at them, so
no manual setup is needed. To use pre-generated headers (or skip the Rust
toolchain), set `SLINT_CPP_GENERATED_INCLUDE` to their directory and `gen:api`
will use it instead of invoking the xtask. Set `SLINT_CPP_DOCS_EXPERIMENTAL=1`
to include experimental APIs.

## Commands

From `docs/cpp` (or prefix with `pnpm -C docs/cpp`):

```sh
pnpm install     # install dependencies (usually done once from repo root)
pnpm dev         # gen:api (via predev) + start dev server
pnpm gen:api     # generate headers + run Doxygen + converter into src/content/docs/api/
pnpm thirdparty  # regenerate the third-party license page
pnpm build       # thirdparty + gen:api + type-check + production build
pnpm preview     # preview the production build
pnpm test        # run the converter unit tests
```

The static site is output under `dist/`.

## Status / follow-ups

Done: the Astro site + converter + build wiring, the ported prose pages
(`overview`, `types`, `getting-started`, `generated-code`, `cmake`,
`cmake-reference`, `live-preview`, `mcu/*`), and the CI swap — the
`rust-cpp-docs` job just runs `pnpm -C docs/cpp run build`, which generates the
cbindgen headers (`cargo xtask generate_cppdocs_headers`), the Doxygen XML, runs
the converter and Astro, and publishes `docs/cpp/dist`.

When porting the prose, these MyST constructs were translated to Starlight:
`:::{note}`/```` ```{caution} ```` → `:::note`/`:::caution` asides;
```` ```{eval-rst} ```` RST grid tables → GitHub Markdown tables; `{toctree}` →
sidebar entries in `astro.config.mjs`; and intra-doc `*.md` links → page slugs.

Cross-references to the Slint language reference use the `SlintRef` component
(`src/components/SlintRef.astro`, the C++ counterpart of the Python docs'): it
resolves a symbol through the shared `linkMap` and prepends `slintDocsBase()`
(from `cpp-site-config.mjs`), the sibling `…/docs/slint/` URL for the version
being built — so the links track snapshots/releases instead of hardcoding
`https://slint.dev`. Prose pages that use it are `.mdx`.

Validated locally (Doxygen 1.9.8 + `pnpm`): the pipeline builds 79 pages and
Starlight reports all internal links valid.

Remaining:

- **Doxygen strictness**: the `Doxyfile` sets `WARN_AS_ERROR = NO` because the
  public headers have two pre-existing, benign Doxygen quirks (a template
  constructor out-of-line match in `slint-interpreter.h` and a documented
  internal `cbindgen_private::LayoutInfo::merge`). Output-link integrity is
  enforced by Starlight's links validator instead. Clean those two up to
  restore `WARN_AS_ERROR = YES`.
- **Prose → API links**: API cross-references that were `{cpp:class}` /
  `{cpp:func}` are currently rendered as inline code rather than links into the
  generated API pages, because the generated slugs can't be verified without
  running Doxygen. Once verified, turn the common ones into links (e.g.
  `slint::Color` → `/api/slint/color/`). Namespace references
  (`/api/slint/`, `/api/slint/interpreter/`) are already linked.
