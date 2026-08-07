# Slint SC Safety Manual

Astro Starlight site for the Slint SC Safety Manual and Qualification Plan.

Hand-written content lives in `src/content/docs/`. Everything under
`src/content/docs/generated/` is produced by `slint-doc-generator` and
gitignored, with one subdirectory per section of the site: `reference/` holds
the API reference of the items annotated with `\sc` in
`internal/compiler/builtins.slint` (and later in
`internal/common/{enums,builtin_structs}.rs`), `qualification-plan/` the
traceability matrix. The pages set their own `slug`, so their location under
`generated/` doesn't affect their URL.

The language specification under `src/content/docs/language/` is also
gitignored: its canonical source is the main Slint docs
(`docs/astro/src/content/docs/reference/language/`), from which
`scripts/sync-language-spec.mjs` copies it on every `pnpm dev`/`pnpm build`.
Edit the chapters there, not here.

## Prerequisites

- [Node.js](https://nodejs.org/) (v22+)
- [pnpm](https://pnpm.io/)
- A Rust toolchain (for `slint-doc-generator`)

Run `pnpm install` from the repository root first.

## Commands

```sh
pnpm install   # install dependencies

# Regenerate the SC-filtered API reference (run from the repo root).
# Required before `pnpm build` if you've changed builtins or generator code.
cargo run -p slint-doc-generator -- --slint-sc generate-mdx

pnpm dev       # start dev server
pnpm build     # type-check and build for production
pnpm preview   # preview the production build
```
