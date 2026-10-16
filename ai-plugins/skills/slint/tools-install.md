# Installing Slint CLI tools

Slint releases ship two CLI binaries:

- `slint-lsp` — language server (diagnostics, hover, go-to-definition,
  formatting); used by any editor or AI assistant that speaks LSP.
- `slint-viewer` — render a `.slint` file standalone (live preview with
  hot-reload, or headless screenshot with `--screenshot`).

Both must be on `PATH`. If a tool reports it can't find `slint-lsp` or
`slint-viewer`, the install location is missing from `PATH`.

## Prebuilt binaries (preferred)

Released alongside every Slint release, downloadable without an API call from
`https://github.com/slint-ui/slint/releases/latest/download/<file>` (pin a
version with `…/releases/download/v<version>/<file>`). `<file>` is
`slint-lsp-<target>.<ext>` or `slint-viewer-<target>.<ext>`,
named after the Rust target triple:

| Platform          | `<target>.<ext>`                       | Extracts to                              |
| ----------------- | -------------------------------------- | ---------------------------------------- |
| Linux x86_64      | `x86_64-unknown-linux-gnu.tar.gz`      | `slint-<tool>/`                          |
| Linux aarch64     | `aarch64-unknown-linux-gnu.tar.gz`     | `slint-<tool>-<target>/`                 |
| Linux armv7       | `armv7-unknown-linux-gnueabihf.tar.gz` | `slint-<tool>-<target>/`                 |
| macOS (universal) | `universal-apple-darwin.tar.gz`        | `slint-<tool>/`                          |
| Windows x86_64    | `x86_64-pc-windows-msvc.zip`           | no folder, `slint-<tool>.exe` at the top |
| Windows arm64     | `aarch64-pc-windows-msvc.zip`          | no folder, `slint-<tool>.exe` at the top |

Don't use the older names (`slint-<tool>-linux.tar.gz`, `-macos.tar.gz`, `-windows-x86_64.zip`, `-windows-arm64.zip`).
They're only kept as copies for backwards compatibility and may be dropped.

Put the directory holding the binary on `PATH`, or copy the binary somewhere that is:

```sh
curl -fsSL https://github.com/slint-ui/slint/releases/latest/download/slint-viewer-x86_64-unknown-linux-gnu.tar.gz | tar xz
install slint-viewer/slint-viewer ~/.local/bin/
```

The prebuilt `slint-viewer` is built with the `mcp` feature.

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
