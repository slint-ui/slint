<!-- cSpell:ignore Doxyfile -->

# Slint C++ API documentation

Astro [Starlight](https://starlight.astro.build/) site for the Slint C++ API.
The API reference is generated from the C++ headers by running Doxygen (XML
output) and converting that XML to Markdown with a small in-repo tool — the
C++ counterpart of `starlight-typedoc`, and a replacement for the previous
Sphinx + Breathe + Exhale pipeline (`api/cpp/docs/`).

Content lives in `src/content/docs/`. The API reference under
`src/content/docs/api/` and the third-party license page under
`src/content/docs/generated/` are generated (both gitignored).

## How the API reference is generated

```
C++ headers ──doxygen (XML)──▶ target/cppdocs/xml ──converter──▶ src/content/docs/api/*.md ──astro──▶ dist/
```

- `Doxyfile` configures Doxygen to emit XML only (no HTML/LaTeX). It mirrors the
  `INPUT`/`EXCLUDE`/`PREDEFINED`/`WARN_AS_ERROR` settings that used to live in
  `api/cpp/docs/conf.py` (`exhaleDoxygenStdin`).
- `scripts/generate-api.ts` runs Doxygen, then reads the Doxygen XML and writes
  one Markdown page per class/struct/namespace into `src/content/docs/api/`. The
  conversion logic is in `scripts/lib/`:
  - `xml.ts` — a tiny dependency-free XML parser (Doxygen XML is machine
    generated, so a full XML library isn't needed).
  - `slug.ts` — page slugs (`api/classes/slint-color`) and member anchors.
  - `doxygen.ts` — the model + Markdown renderer (signatures, descriptions,
    parameter/return docs, notes → Starlight asides, code listings, enum value
    tables, and cross-references resolved to root-relative links).

The converter is dependency-free and unit-tested against a fixture:

```sh
pnpm -C docs/cpp test     # node --test, no extra deps
```

## Prerequisites

- [Node.js](https://nodejs.org/) (v22+) and [pnpm](https://pnpm.io/)
- [Doxygen](https://www.doxygen.nl/) for `gen:api`
- For `thirdparty`: a Rust toolchain and
  [cargo-about](https://github.com/EmbarkStudios/cargo-about)

The C++ API reference also needs the cbindgen-generated headers. Produce them
with `cargo xtask cppdocs` and point `SLINT_CPP_GENERATED_INCLUDE` at the
generated directory before running `gen:api`.

## Commands

From `docs/cpp` (or prefix with `pnpm -C docs/cpp`):

```sh
pnpm install     # install dependencies (usually done once from repo root)
pnpm dev         # start dev server (run gen:api first; the API sidebar autogenerates from it)
pnpm gen:api     # run Doxygen + converter into src/content/docs/api/
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
`rust-cpp-docs` job now generates the cbindgen headers with
`cargo xtask cppdocs`, then runs `pnpm -C docs/cpp run build`
(Doxygen XML + converter + Astro) and publishes `docs/cpp/dist`. The legacy
Sphinx/Breathe/Exhale setup in `api/cpp/docs/` and the Sphinx branch of
`xtask cppdocs` have been removed.

When porting the prose, these MyST constructs were translated to Starlight:
`:::{note}`/```` ```{caution} ```` → `:::note`/`:::caution` asides;
```` ```{eval-rst} ```` RST grid tables → GitHub Markdown tables; `{toctree}` →
sidebar entries in `astro.config.mjs`; intra-doc `*.md` links → page slugs; and
`slint-reference:`/`../slint/…` links → absolute `https://slint.dev/docs/slint/`
URLs.

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
  `slint::Color` → `/api/classes/slint-color/`). Namespace references
  (`/api/namespaces/slint/`, `/api/namespaces/slint-interpreter/`) are already
  linked.
