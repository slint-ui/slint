<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# SlintPad

This directory contains the frontend code for SlintPad, the online code editor
which is hosted on https://slint.dev/editor (last stable) and
https://slint.dev/snapshots/master/editor (nightly).

You need to install the following components:
* **[Node.js](https://nodejs.org/download/release/)** (v20. or newer)
* **[pnpm](https://www.pnpm.io/)**

To try it out locally type this in this directory:

```sh
## only need to run this once
pnpm install
pnpm build:wasm_interpreter  # Build the wasm interpreter used in `preview.html`
pnpm build:wasm_lsp          # Build the wasm LSP used by the text editor

## Run this to refresh slintpad (dev mode!)
pnpm start                   # Run in development mode

## Run this to refresh slintpad (build mode!)
pnpm build                   # Build the web UI code
pnpm preview              # Start a server serving the slintpad UI
```

## Documentation

The `index.html` page contains a code editor and every key press reload the preview.
The `preview.html` page contains only the preview and the code must be given via query parameter.

-   `?load_url=` query argument make it possible to load the .slint code directly from an URL.
    If the slint code contains relative path for imports or images, they are loaded relative to
    that slint file. That way it is possible to load code from github (via raw.githubusercontent)
    or gists.

    Example: this loads the printerdemo.slint file from the github URL

    -   https://slint.dev/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/demos/printerdemo/ui/printerdemo.slint
    -   https://slint.dev/editor/preview.html?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/demos/printerdemo/ui/printerdemo.slint

-   `?snippet=` query argument, followed by the URL-encoded slint code, will simply load this code.

    Example: a simple code with "Hello Slint"

    -   https://slint.dev/editor/?snippet=_+%3A%3D+Text+%7B+text%3A+%22Hello+Slint%22%3B+%7D
    -   https://slint.dev/editor/preview.html?snippet=_+%3A%3D+Text+%7B+text%3A+%22Hello+Slint%22%3B+%7D

-   `?style=` query argument, followed by the name of the style to use.

    Example: The "Hello Slint" using different styles

    -   https://slint.dev/editor/?snippet=_+%3A%3D+Text+%7B+text%3A+%22Hello+Slint%22%3B+%7D?style=fluent-dark
    -   https://slint.dev/editor/preview.html?snippet=_+%3A%3D+Text+%7B+text%3A+%22Hello+Slint%22%3B+%7D?style=material

-   `?gz=` Like `?snippet=` but compressed with gzip and base64 encoded.