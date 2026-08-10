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
`slint-lsp-<target>` or `slint-viewer-<target>` with `<target>` one of:

- `linux.tar.gz` (x86_64), `aarch64-unknown-linux-gnu.tar.gz`,
  `armv7-unknown-linux-gnueabihf.tar.gz`
- `macos.tar.gz`
- `windows-x86_64.zip`, `windows-arm64.zip`

Each archive extracts to a directory named after the tool with the binary
inside; put that on `PATH`:

```sh
curl -fsSL https://github.com/slint-ui/slint/releases/latest/download/slint-viewer-linux.tar.gz | tar xz
install slint-viewer/slint-viewer ~/.local/bin/
```

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
