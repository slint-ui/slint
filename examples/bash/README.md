# SixtyFPS Bash example

This shows how to use `sixtyfps-viewer` to display dialog from a bash script.

This assume that the `sixtyfps-viewer` tool is in patch. This can be achieved with cargo install.
(use the `--path tools/viewer` option to install it from the current repository.)

```bash
cargo install sixtyfps-viewer
```

The examples also assume that [`jq`](https://stedolan.github.io/jq/) is in the path

 * `simple_input.sh`: shows how to query a few parameter with bash


## Attributions

The `laptop.svg` icon is `emoji_u1f4bb.svg` from the Noto Emoji font from
    https://github.com/googlefonts/noto-emoji
and licensed under the terms of the Apache license, version 2.0; copyright Google Inc.
