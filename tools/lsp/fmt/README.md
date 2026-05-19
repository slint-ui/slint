# LSP formatter glue

The formatter implementation lives in `internal/formatter/`.

This directory now contains:

- the `slint-lsp format` command-line glue in `tool.rs`
- temporary regression tests in `fmt.rs`

The long-term plan is to move the remaining formatter regression tests into the
shared formatter crate once that test surface stabilizes.
