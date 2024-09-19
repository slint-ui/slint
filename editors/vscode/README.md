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

## Building from source and debugging

The following step will build a local version of the vscode extension and the LSP

```sh
cargo install wasm-pack
cargo build -p slint-lsp
cd editors/vscode
npm clean-install
npm run build:wasm_lsp
npm run compile
```

Later, you only need to do the steps for the part you change like `cargo build -p slint-lsp` to rebuild the lsp binary
or `npm run compile` to rebuild the typescript.

You can run vscode with that extension by running, in the `editors/vscode` directory:

```sh
code --extensionDevelopmentPath=$PWD ../..
```

If you want to load the web extension, add `--extensionDevelopmentKind=web`

## How to build the extension package

To create a `.vsix` package for local installation:

1. Follow the setup steps above to build the lsp binary and install npm dependencies.

2. Create a `.vsix` package (needs `vsce` installed)

```sh
npm run local-package
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
To ensure your PR does not fail check everything with `npm run syntax_check && npm run format && npm run lint`.
`npm run lint:fix` and `npm run format:fix` can be used to auto fix lint and code formatting issues.

## Preview the Library, Preview, and Property Editor

The built-in live-preview can be used to preview itself. For this to work, VS Code needs to be restarted with an environment variable:

1. Close all VS Code Windows and terminate the application.
2. In a terminal Window, re-launch VS Code:
   ```bash
   SLINT_ENABLE_EXPERIMENTAL_FEATURES=1 code
   ```
3. Open `tools/lsp/ui/main.slint` and launch the preview, or preview individual components such as the
   library or properties view.
