# Figma to Slint

A Figma plugin that previews a selection with Slint and exports editable Slint
projects. It also provides Slint snippets in Figma Dev Mode, including the
Figma VS Code extension. All conversion and preview processing runs offline.

## Installation and usage

Install from [Figma Community](https://www.figma.com/community/plugin/1474418299182276871/figma-to-slint),
or download the [nightly ZIP](https://github.com/slint-ui/slint/releases/download/nightly/figma-plugin.zip).
For a nightly build, extract the archive and import its `manifest.json` through
Figma Desktop's Plugins > Development > Import plugin from manifest.

Select a node and run the plugin to preview it. Use Copy Slint for the generated
source or Export ZIP for a project. Pin keeps the current selection in view.
Diagnostics explains unsupported features and approximations.
Export generation errors and compiler diagnostics keep a successful preview and its generated source available.
Export ZIP stays disabled when the reusable project cannot be generated or compiled.
Unexpected interpreter failures require restarting the plugin.

Large selections retain the full selected tree and its component definitions.
The preview appears before project export generation and validation finish.
Large previews scale to fit; snapshots and project exports keep their original dimensions.
Repeat selections reuse completed work when the document and referenced components are unchanged.
Variable definitions and the document color profile are checked before reuse.
Failed captures and text rendered as images are read again.
Wasm and fonts initialize during the first capture.

In Dev Mode, choose Slint to generate a snippet for the selected node only.
Enable Use Variables to reference existing Slint variable globals; it is off by
default. Use preview and project export for the complete selected tree.

## Development

Use the repository's Node.js and pnpm versions, Rust with the
`wasm32-unknown-unknown` target, and wasm-pack.

```sh
pnpm install --frozen-lockfile # from the repository root
cd tools/figma-inspector
pnpm build:slint
pnpm build
```

Import `dist/manifest.json` in Figma Desktop. For development, run `pnpm dev`
and import `dist-dev/manifest.json`.
Run `pnpm verify` for the full checks and tests.

The plugin captures Figma nodes as JSON, converts them to Slint, and uses that
output for preview and export.
`pnpm build:slint` builds its Slint runtime from `api/wasm-interpreter` in this repository.
Use `pnpm build:slint:dev` for a development interpreter build.

`pnpm check` verifies the checked-in font capability manifest against the runtime's embedded fonts and the preview's bundled Roboto font.
The manifest records font families, glyph coverage, weight axes, and italic availability.
When the runtime's embedded fonts change, run `node scripts/build-font-capabilities.mjs --update` and review the manifest.
Static checks do not need a runtime build.
Preview uses native text only when those capabilities cover the captured text and supported font settings.
Otherwise, it uses Figma's rendered image.
Family matching does not establish that Figma uses the same font version.
Native exports remain editable text and require the matching fonts installed or imported.

See [fixture instructions](fixtures/README.md) and [publishing](PUBLISH.md).

## Capture Profiling

Development diagnostics and Copy Trace report capture reads, encoding, component discovery, bulk JSON, asset exports, and browser presentation.
Capture categories report operation counts, summed duration, active time, and overlap with synchronous work.
Their intervals overlap; add neither category durations nor nested work to estimate total capture time.
Export validation has separate source-preparation, compiler-queue, compilation, and completion timings.

Start each cold measurement in a fresh plugin session with the file already open.
For a repeat measurement, select another node, wait for completion, and return to the original selection in the same page.
Keep the developer console closed until conversion and export validation finish.
Record the source revision, selection ID, capture counts, preview time, and export-validation completion time.
Report errors and timeouts separately from successful conversion timings.
