# sixtyfps-vscode README

Extension for VSCode which include syntax coloration and a way to start the LSP server

## How to build the extension package

Until the building of the LSP binaries is automated via npm, the following commands need to be run before packaging with `vsce package`:

```shell
cd ..
cross build --target x86_64-unknown-linux-gnu --release -p sixtyfps-lsp
cp target/x86_64-unknown-linux-gnu/release/sixtyfps-lsp vscode_extension/bin/sixtyfps-lsp-x86_64-unknown-linux-gnu
cross build --target x86_64-pc-windows-gnu --release -p sixtyfps-lsp
cp target/x86_64-pc-windows-gnu/release/sixtyfps-lsp.exe vscode_extension/bin/sixtyfps-lsp-x86_64-pc-windows-gnu.exe
cargo build --release -p sixtyfps-lsp
cp target/release/sixtyfps-lsp vscode_extension/bin/sixtyfps-lsp-x86_64-apple-darwin
```

## How to run the LSP

At the moment you need to load this directory in VS code and then start debugging (Run -> Start Debugging). That will "debug" the vs code extension and create a new VS code window. The LSP server binary will be started if previously built. You can see the output in the output pane "SixtyFPS LSP" (that's the drop-down that usually shows "Tasks").