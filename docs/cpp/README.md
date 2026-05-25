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

This is the infrastructure (Astro site + converter + wiring). Remaining work to
fully replace `api/cpp/docs/`:

- **Port the prose pages** from `api/cpp/docs/*.md` (`overview.md`, `types.md`,
  `getting_started.md`, `generated_code.md`, `cmake.md`, `cmake_reference.md`,
  `live_preview.md`, `mcu/*`) into `src/content/docs/` and re-add them to the
  sidebar. These are mechanical copies plus a few MyST→Starlight translations:
  - `:::{note} … :::` → `:::note … :::` (Starlight asides).
  - ```` ```{eval-rst} ```` RST grid tables → GitHub Markdown tables.
  - `{ref}`/`{cpp:class}`/`{cpp:func}` → root-relative links into the generated
    API pages (e.g. `slint::Color` → `/api/classes/slint-color/`).
  - `{toctree}` → sidebar entries in `astro.config.mjs`.
- **Versioned header**: the other doc sites use
  `@slint/common-files` `HeaderVersioned`, which currently only knows
  `docsUrlKind: "node"`. Add a `"cpp"` kind there to get the version selector;
  this site uses the default Starlight header until then.
- **CI**: swap the `rust-cpp-docs` job's `cargo xtask cppdocs` (Sphinx) step for
  `doxygen` + this site's `build`, mirroring the `node-python-docs` job, and
  drop the Sphinx/Breathe/Exhale `uv` environment in `api/cpp/docs/`.
