# Installing Slint CLI tools

Slint releases ship two CLI binaries:

- `slint-lsp` — language server (diagnostics, hover, go-to-definition,
  formatting); used by any editor or AI assistant that speaks LSP.
- `slint-viewer` — render a `.slint` file standalone (live preview with
  hot-reload, or headless screenshot with `--screenshot`).

Both must be on `PATH`. If a tool reports it can't find `slint-lsp` or
`slint-viewer`, the install location is missing from `PATH`.

## Prebuilt binaries (preferred)

Download `https://github.com/slint-ui/slint/releases/latest/download/slint-<tool>-<suffix>`,
where `<tool>` is `lsp` or `viewer` (pin a version with `…/releases/download/v<version>/…`).
`<suffix>` and the archive layout:

- `x86_64-unknown-linux-gnu.tar.gz`, `universal-apple-darwin.tar.gz` (macOS): binary in `slint-<tool>/`
- `aarch64-unknown-linux-gnu.tar.gz`, `armv7-unknown-linux-gnueabihf.tar.gz`: binary in `slint-<tool>-<suffix without .tar.gz>/`
- `x86_64-pc-windows-msvc.zip`, `aarch64-pc-windows-msvc.zip`: `.exe` at the top level

```sh
curl -fsSL https://github.com/slint-ui/slint/releases/latest/download/slint-viewer-x86_64-unknown-linux-gnu.tar.gz | tar xz
install slint-viewer/slint-viewer ~/.local/bin/
```

The prebuilt `slint-viewer` includes the `mcp` feature.

On Debian/Ubuntu, install the runtime dependencies:

```sh
sudo apt install -y libx11-xcb1 libxkbcommon0 libinput10 libgbm1
```

## cargo install (requires Rust)

```sh
cargo install --locked slint-lsp
cargo install --locked slint-viewer --features mcp
```

Cargo places binaries in `$HOME/.cargo/bin`, which is on `PATH` when Rust was
installed via `rustup`. For builds against unreleased `master`, add
`--git https://github.com/slint-ui/slint`.
`--features mcp` `(1.18+)` gives MCP access to a running `.slint` file (see [debugging-and-mcp.md](reference/debugging-and-mcp.md));
drop it if you don't need that.
