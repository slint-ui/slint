# sixtyfps-vscode README

Extension for VSCode which include syntax coloration and a way to start the LSP server

## Features

 - Syntax highlighting
 - Diagnostics from .60 files
 - Live Preview of a .60 file
 - Completion of properties
 - Jump to definition (currently, only definition of Component)

## Setup

1. Build the LSP

```sh
cargo build --bin sixtyfps-lsp
```

2. run npm install in the vscode directory

```sh
cd vscode_extension
npm install
```

## How to debug the LSP

At the moment you need to load this directory in VS code and then start debugging (Run -> Start Debugging).
That will "debug" the vs code extension and create a new VS code window. The LSP server binary will be started if previously built
You can see the output in the output pane "SixtyFPS LSP" (that's the drop-down that usually shows "Tasks").

## How to build the extension package

To create a `.vsix` package for local installation:

1. Follow the setup steps above to build the lsp binary and install npm dependencies.

2. Create a `.vsix` package (needs `vsce` installed)

```sh
npm run local-package
```
3. Install the `.vsix` file with

```sh
code --install-extension sixtyfps-vscode-*.vsix
```

4. Reload your VS code windows

Note that the resulting `.vsix` package contains your locally built debug LSP server. It is not suitable for distribution.
