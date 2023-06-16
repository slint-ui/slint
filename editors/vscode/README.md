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

## Building from Source

1. Build the LSP

```sh
cargo build --bin slint-lsp
```

2. run npm install in the vscode directory

```sh
cd editors/vscode
npm install
```

## How to debug the LSP

From another VS Code instance: Load this directory in VS code and then start debugging (Run -> Start Debugging).
That will "debug" the vs code extension and create a new VS code window. The LSP server binary will be started if previously built

You can see the output in the output pane "Slint LSP" (that's the drop-down that usually shows "Tasks").

Alternatively, you can start vscode with the following options:

```sh
cd editors/vscode
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
