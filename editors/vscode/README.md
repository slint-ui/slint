<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Slint for Visual Studio Code

This extension for VS Code adds support for the [Slint](https://slint.dev) design markup language.

## Features

-   Syntax highlighting
-   Diagnostics from .slint files
-   Live Preview of a .slint file
-   Completion of properties
-   Jump to definition (currently, only definition of Component)

## Installation

You can install the extension directly from the [Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Slint.slint). Afterwards it is
automatically activated when editing files with the `.slint` extension.

## Live-Preview

In addition to the usual code editing features such as completion and syntax highlighting, this extension
also offers the ability to view a rendering of the file you're editing and update it on-the-fly when making
changes.

You can issue the "Slint: Show Preview" command from the command palette when editing a `.slint` file. This
will create a new top-level window that renders the file you're editing. Any changes you make are immediately
visible, it is not necessary to save the file.

## Reporting Issues

Issues should be reported in the [Slint issue tracker](https://github.com/slint-ui/slint/labels/vscode-extension).

<!-- lines below this marker are stripped from the release -->

## Building from Source and Debugging

You need to install the following components:
* **[Node.js](https://nodejs.org/download/release/)** (v20. or newer)
* **[pnpm](https://www.pnpm.io/)**

The following step will build a local version of the vscode extension and the LSP

```sh
cargo install wasm-pack
cargo build --features renderer-skia -p slint-lsp
cd editors/vscode
pnpm install --frozen-lockfile
pnpm build:wasm_lsp
pnpm compile
```

Later, you only need to do the steps for the part you change like `cargo build -p slint-lsp` to rebuild the lsp binary
or `pnpm compile` to rebuild the typescript.

You can run vscode with that extension by running, in the `editors/vscode` directory:

```sh
code --extensionDevelopmentPath=$PWD <path to a slint project that is different to the one already open in vscode>
```

If you want to load the web extension, add `--extensionDevelopmentKind=web`

## How to build the extension package

To create a `.vsix` package for local installation:

1. Follow the setup steps above to build the lsp binary and install npm dependencies.

2. Create a `.vsix` package (needs `vsce` installed)

```sh
pnpm local-package
```

3. Install the `.vsix` file with

```sh
code --install-extension slint-*.vsix
```

4. Reload your VS code windows

Note that the resulting `.vsix` package contains your locally built debug LSP server. It is not suitable for distribution.

## Rules for PRs
The code is typechecked with `tsc` and linted/formatted with Biome.
If using VS Code then install the [biome extension](https://marketplace.visualstudio.com/items?itemName=biomejs.biome).
To ensure your PR does not fail check everything with `pnpm type-check && pnpm format && pnpm lint`.
`pnpm lint:fix` and `pnpm format:fix` can be used to auto fix lint and code formatting issues.

## Preview the Library, Preview, and Property Editor

The built-in live-preview can be used to preview itself. For this to work, VS Code needs to be restarted with an environment variable:

1. Close all VS Code Windows and terminate the application.
2. In a terminal Window, re-launch VS Code:
   ```bash
   SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 code
   ```
3. Open `tools/lsp/ui/main.slint` and launch the preview, or preview individual components such as the
   library or properties view.

## Quality Assurance

This extensions comes with some tools to help with QA:

 * `pnpm lint` and `pnpm lint:fix` run the biome linter on the source code
 * `pnpm type-check` run the typescript compiler
 * `pnpm test_grammar` run the tests on the TextMate grammar build into the extension
