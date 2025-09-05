<!-- Copyright © Luke. D Jones <luke@ljones.dev> ; SPDX-License-Identifier: GPL-3.0-or-later -->

# Zed Slint

This extension adds support for the [Slint UI](https://slint.dev) language to [Zed]([zed.dev](https://zed.dev)https://zed.dev).

## Settings

This extension allows customization of the binary path and its arguments.
Normally you won't need to configure anything. Unless you are using a development shell like `nix develop` or `devenv`.

```json
{
  "lsp": {
    "slint": {
      "binary": {
        "path": "/path/to/slint-lsp",
        "arguments": [],
        "env": {}
      },
    }
  }
}
```

## Development

The extension will download the version of the Slint LSP binary match the version of the extension.
To test out the development version of the extension, start `zed` with the `SLINT_DEV_MODE` environment variable set, and "Install Dev Extension" from the `editors/zed` folder, or "rebuild extension".
When the `SLINT_DEV_MODE` environment variable is set, the extension will download the
latest nightly version of the LSP binary.
