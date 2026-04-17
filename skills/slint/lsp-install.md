# Installing `slint-lsp`

`slint-lsp` is the Slint Language Server. It speaks LSP and can be
used by any editor or AI coding assistant that supports the protocol
to get diagnostics, hover, go-to-definition, and formatting on `.slint`
files. The binary is distributed separately from this skill and must
be on `PATH` for tools to find it.

If your editor or assistant reports that `slint-lsp` cannot be found,
install it using one of the methods below and ensure the install
location is on your `PATH`.

## Install with cargo (recommended if you have Rust)

```sh
cargo install slint-lsp
```

This places `slint-lsp` in `$HOME/.cargo/bin`, which is on `PATH` when
Rust was installed via `rustup`.

## Install a prebuilt binary

Prebuilt binaries are published with each Slint release. From the
[latest release](https://github.com/slint-ui/slint/releases/latest)
download the asset for your platform:

| Platform          | Asset                                              |
|-------------------|----------------------------------------------------|
| Linux x86-64      | `slint-lsp-linux.tar.gz`                           |
| Linux aarch64     | `slint-lsp-aarch64-unknown-linux-gnu.tar.gz`       |
| Linux armv7       | `slint-lsp-armv7-unknown-linux-gnueabihf.tar.gz`   |
| macOS (universal) | `slint-lsp-macos.tar.gz`                           |
| Windows x86-64    | `slint-lsp-windows-x86_64.zip`                     |
| Windows aarch64   | `slint-lsp-windows-arm64.zip`                      |

Extract the archive and put the `slint-lsp` binary somewhere on `PATH`.

On Debian/Ubuntu, install the runtime dependencies:

```sh
sudo apt install -y libx11-xcb1 libxkbcommon0 libinput10 libgbm1
```
