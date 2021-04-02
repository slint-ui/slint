# sixtyfps-vscode README

Extension for VSCode which include syntax coloration and a way to start the LSP server

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

## How to run the LSP

At the moment you need to load this directory in VS code and then start debugging (Run -> Start Debugging).
That will "debug" the vs code extension and create a new VS code window. The LSP server binary will be started if previously built
You can see the output in the output pane "SixtyFPS LSP" (that's the drop-down that usually shows "Tasks").

## How to build the extension package

To create a `.vsix` package:

  1. Install `vsce` (via npm for example).
  2. Change to the `vscode_extension` sub-directory.
  3. Install the dependencies: `npm install`.
  4. Build the lsp binaries: `npm compile-lsp`.
  5. Run `vsce package` to create the extension package.
