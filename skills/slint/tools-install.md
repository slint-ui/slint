# Installing Slint CLI tools

Slint releases ship two CLI binaries:

- `slint-lsp` — language server (diagnostics, hover, go-to-definition,
  formatting); used by any editor or AI assistant that speaks LSP.
- `slint-viewer` — render a `.slint` file standalone (live preview with
  hot-reload, or headless screenshot with `--screenshot`).

Both must be on `PATH`. If a tool reports it can't find `slint-lsp` or
`slint-viewer`, the install location is missing from `PATH`.

## Prebuilt binaries (preferred)

Released alongside every Slint release on
<https://github.com/slint-ui/slint/releases/latest>.
Download the archive for your platform, extract, and drop the binaries on
`PATH`.

On Debian/Ubuntu, install the runtime dependencies:

```sh
sudo apt install -y libx11-xcb1 libxkbcommon0 libinput10 libgbm1
```

## cargo install (requires Rust)

```sh
cargo install slint-lsp
cargo install slint-viewer
```

Cargo places binaries in `$HOME/.cargo/bin`, which is on `PATH` when Rust was
installed via `rustup`. For builds against unreleased `master`, add
`--git https://github.com/slint-ui/slint`.
