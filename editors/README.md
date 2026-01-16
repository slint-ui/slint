
# Editor Configuration for Slint

This folder contains extensions or configuration files for different editor to better support .slint files.
See [the Slint documentation](https://snapshots.slint.dev/master/docs/slint/guide/tooling/manual-setup/) for details on how to configure your editor of choice.

## Using Slint with Rust Projects

When developing Slint applications in Rust, you'll typically use **two language servers** running simultaneously:

| Language Server | Handles | Features |
|----------------|---------|----------|
| **rust-analyzer** | `.rs` files | Rust completions, diagnostics, go-to-definition, refactoring |
| **slint-lsp** | `.slint` files and `slint!` macros | Slint completions, diagnostics, live preview, formatting |

### How They Work Together

Both language servers run independently and don't communicate with each other. This means:

- **In `.slint` files**: Only slint-lsp is active, providing full Slint language support
- **In `.rs` files**: Both are active
  - rust-analyzer handles Rust code outside the `slint!` macro
  - slint-lsp detects `slint!` macro content and provides Slint features within it

### Editor Setup for Rust + Slint

Most editors handle multiple language servers automatically. Install both:

```sh
# Install rust-analyzer (if not already installed via rustup)
rustup component add rust-analyzer

# Install slint-lsp
cargo install slint-lsp
```

Then configure your editor for both (see editor-specific sections below).

### Features and Limitations

**What works well:**
- Full Slint language support in `.slint` files
- Slint completions and diagnostics inside `slint!` macros in Rust files
- Independent Rust analysis by rust-analyzer
- Live preview of components

**Current limitations:**
- No cross-language go-to-definition (can't jump from Rust `component.get_property()` to Slint property definition)
- No unified diagnostics (errors shown separately by each LSP)
- The `slint!` macro uses identifier syntax (`foo-bar`) that differs from Rust (`foo - bar`). In rust-analyzer, adjacent identifiers with dashes are treated as single identifiers, which is usually correct for Slint code.

### Troubleshooting

**rust-analyzer shows errors in `slint!` macro:**
This can happen because rust-analyzer expands the macro differently than rustc. These errors are usually false positives - check that `cargo build` succeeds.

**Completions not working in `slint!` macro:**
Ensure slint-lsp is running. In VS Code, check the Output panel for "Slint Language Server". In other editors, verify the LSP process is started.

**Performance issues:**
If your editor feels slow, rust-analyzer's macro expansion may be the cause. You can disable proc-macro expansion in rust-analyzer settings, though this will reduce Rust-side analysis quality:
```json
{
  "rust-analyzer.procMacro.enable": false
}
```

## Editors

- [Visual Studio Code](https://snapshots.slint.dev/master/docs/slint/guide/tooling/vscode/)
- [Kate](https://snapshots.slint.dev/master/docs/slint/guide/tooling/kate/)
- [Qt Creator](https://snapshots.slint.dev/master/docs/slint/guide/tooling/qt-creator/)
- [Helix](https://snapshots.slint.dev/master/docs/slint/guide/tooling/helix/)
- [(Neo-)Vim](https://snapshots.slint.dev/master/docs/slint/guide/tooling/neo-vim/)
- [Sublime Text](https://snapshots.slint.dev/master/docs/slint/guide/tooling/sublime-text/)
- [JetBrains IDE](https://snapshots.slint.dev/master/docs/slint/guide/tooling/jetbrains-ide/)
- [Zed](https://snapshots.slint.dev/master/docs/slint/guide/tooling/zed/)
- [Manual Setup](https://snapshots.slint.dev/master/docs/slint/guide/tooling/manual-setup/)
