# Figma to Slint

An offline Figma plugin that previews a selection with Slint and exports editable
Slint source. Targets Slint 1.18+; the exact development runtime is pinned in
`runtime-pin.json`.

## Development

Use the monorepo Node.js and pnpm versions, Rust with the `wasm32-unknown-unknown` target,
and wasm-pack 0.13.1. Then:

```sh
pnpm install --frozen-lockfile # from the repository root
cd tools/figma-inspector
pnpm build:slint
pnpm build
pnpm build:dev
```

`prepare:slint` prepares a separate checkout in `.generated/slint-source`.
Set `SLINT_REPO` to use an existing clean checkout matching the pin. That source
is read-only. `SLINT_CARGO` and `SLINT_WASM_PACK` can select explicit tool binaries
when shell tool managers cannot run inside the prepared checkout.

Import `dist/manifest.json` in Figma Desktop through Plugins > Development >
Import plugin from manifest. Use `dist-dev/manifest.json` for development controls.
`pnpm dev` watches plugin files and rebuilds the development bundle.

## Behavior

Select one node to capture and preview it. Empty selection clears the preview.
Changing selection clears old output immediately; errors open Diagnostics.
Pin keeps the selected root active until unpinned, deleted, or the page changes.
Pin state is not persisted.

The converter supports fixed geometry, supported auto-layout through
`FlexboxLayout`, text, images, masks, strokes, shadows, component families,
variants and variables. Unsupported properties produce diagnostics and explicit
approximations where possible. Some content is rasterized for preview fidelity;
export preserves editable native text and supported component APIs. Exported
fonts must be supplied by the user when requested by the package instructions.

Copy returns raw generated Slint. Export downloads a self-contained project ZIP.
The plugin bundle embeds its runtime and assets and denies network access.

In Figma Dev Mode, choose Slint in the native code panel to get an element
snippet for the selected node. This captures only the root, excluding its Figma
descendants and component families. Generated helpers for the root's fills,
strokes, shadows and layout remain included. Text stays native without font-file
imports; supported images are inline. Warnings appear in a separate Diagnostics
section. This is available in both builds and does not open the preview window.
Use the regular preview/export workflow for the complete selected tree.

## Checks and packaging

- `pnpm verify`: runtime/artifact checks, formatting, lint, types, unused code,
  production/development builds, bundle/package contracts, unit and browser tests.
- `pnpm test:unit`: deterministic conversion and lifecycle regressions.
- `pnpm test:browser`: built UI and interpreter integration tests.
- `pnpm zip`: release packaging, requiring an official release pin and assigned
  plugin ID; development pins cannot be published.
- `pnpm zip:nightly`: builds `zip/figma-plugin.zip` from the clean pinned development runtime.

See [architecture](docs/architecture.md), [authored fixtures](fixtures/README.md),
and [release instructions](docs/RELEASING.md). Builds and package tests do not publish.

## Published plugin compatibility

The plugin retains the [Figma Community listing](https://www.figma.com/community/plugin/1474418299182276871/figma-to-slint) and supports native codegen in Figma Desktop and the Figma VS Code extension.
Choose Slint in Dev Mode and enable Use Variables to reference existing Slint variable globals.
The setting defaults to No and affects native snippets only.
Unavailable or unsupported variable bindings retain resolved values with diagnostics.
The preview and project ZIP replace the old inspector UI and its separate variable-export workflow.

[Nightly builds](https://github.com/slint-ui/slint/releases/download/nightly/figma-plugin.zip) use the exact runtime in `runtime-pin.json`, independently of the surrounding monorepo revision.
Nightly packaging is separate from Community release packaging and records its channel in the ZIP provenance.
