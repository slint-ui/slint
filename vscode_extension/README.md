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

To create a `.vsix` package, install `vsce` (via npm for example) and run `vsce package`. This will build the extension and the LSP server
binaries into a bundle.
