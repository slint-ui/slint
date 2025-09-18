<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Python Slint Compiler

This package is a wrapper around [Slint's](https://slint.dev) compiler for Python, to generate a typed `.py` file from a `.slint` file, for use with [Slint for Python](https://pypi.org/project/slint/).

When run, the slint compiler binary is downloaded, cached, and run.

By default, the Slint compiler matching the version of this package is downloaded. To select a specific version, set the `SLINT_COMPILER_VERSION` environment variable. Set it to `nightly` to select the latest nightly release.

## Example

```bash
uxv run slint-compiler -f python -o app_window.py app-window.slint
```
