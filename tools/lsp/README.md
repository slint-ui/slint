
# LSP (Language Server Protocol) Server for Slint

This directory contains the implementation of the LSP server for [Slint](https://slint.dev)
featuring diagnostics, code completion, goto definition, and more importantly, live-preview

## Generic usage

The LSP server consists of a binary, `slint-lsp` (or `slint-lsp.exe` on Windows). It provides all the functionality and allows any programming editor that also implements the standardized LSP protocol to communicate with it.

For details on configuration see [the Slint documentation](http://docs.slint.dev/docs/guide/tooling/manual-setup/#slint-lsp).

## Code formatting

The Slint code formatting tool is part of the lsp. To learn how to use it as a standalone tool, see [the Slint documentation](http://docs.slint.dev/docs/guide/tooling/manual-setup/#fmt)
