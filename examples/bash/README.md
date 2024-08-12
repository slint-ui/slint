<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint Bash example

This shows how to use [`slint-viewer`](../../tools/viewer) to display dialog from a bash script.

This assume that the `slint-viewer` tool is in path. This can be achieved with cargo install.
(use the `--path tools/viewer` option to install it from the current repository.)

```bash
cargo install slint-viewer
```

The examples also assume that [`jq`](https://stedolan.github.io/jq/) is in the path

 * `simple_input.sh`: shows how to query a few parameter with bash
 * `sysinfo_linux.sh`/`sysinfo_macos.sh`: show how to display the result of bash commands.


## Attributions

The `laptop.svg` icon is `emoji_u1f4bb.svg` from the Noto Emoji font from
    https://github.com/googlefonts/noto-emoji
and licensed under the terms of the SIL Open Font License, version 1.1; copyright Google Inc.

![Screenshot of the bash example](https://github.com/user-attachments/assets/dcac4189-fc7b-4a9f-b7cb-9d075b338628 "Bash")
