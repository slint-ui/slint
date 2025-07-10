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
      "binary": "slint-lsp",
      "args": []
    }
  }
}
```
