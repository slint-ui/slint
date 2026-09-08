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

In Dev Mode, choose Slint to generate a snippet for the selected node only.
Enable Use Variables to reference existing Slint variable globals; it is off by
default. Use preview and project export for the complete selected tree.

## Development

Use the repository's Node.js and pnpm versions, Rust with the
`wasm32-unknown-unknown` target, and the wasm-pack version in `runtime-pin.json`.

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
output for preview and export. Its Slint runtime is independently pinned in
`runtime-pin.json` and prepared in `.generated/slint-source`. Set `SLINT_REPO`
to use another clean checkout matching the pin.

See [fixture instructions](fixtures/README.md) and [publishing](PUBLISH.md).
