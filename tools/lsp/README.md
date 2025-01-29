<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# LSP (Language Server Protocol) Server for Slint

This directory contains the implementation of the LSP server for [Slint](https://slint.dev)
featuring diagnostics, code completion, goto definition, and more importantly, live-preview

## Generic usage

The LSP server consists of a binary, `slint-lsp` (or `slint-lsp.exe` on Windows). It provides all the functionality and allows any programming editor that also implements the standardized LSP protocol to communicate with it.



If you have Rust installed, you can install the binary by running the following command:

```sh
cargo install slint-lsp
```

This makes the latest released version available in `$HOME/.cargo/bin`. If you would like to try a development version, you can also point `cargo install` to the git repository:
for the released version. Or, to install the development version:

```sh
cargo install slint-lsp --git https://github.com/slint-ui/slint --force
```


Alternatively, you can download one of our pre-built binaries for Linux or Windows:

1. Open <https://github.com/slint-ui/slint/releases>
2. Click on the latest release
3. From "Assets" download either `slint-lsp-linux.tar.gz` for a Linux x86-64 binary
   or `slint-lsp-windows.zip` for a Windows x86-64 binary.
4. Uncompress the downloaded archive into a location of your choice.

As the next step, configure your editor to use the binary, no arguments are required

Make sure the required dependencies are found. On Debian-like systems install them with the following command:

```shell
sudo apt install -y build-essential libx11-xcb1 libx11-dev libxcb1-dev libxkbcommon0 libinput10 libinput-dev libgbm1 libgbm-dev
```

## Code formatting

The slint code formatting tool is part of the lsp. To learn how to use it as a standalone tool, see [fmt README](./fmt/README.md)

# Editor configuration

Please check the [editors folder](../../editors/README.md) in the Slint repository for instructions on how to set up different editors to work with Slint.
