<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

This GitHub release is a nightly snapshot of Slint development. It serves to provide access to pre-release binaries.

The attached binaries are updated once a day by a GitHub action building from the  `master` branch.

## How To Try Out This Development Release

### Rust

For Rust users, include the following in your Cargo.toml (or .cargo/config.toml):

```toml
[patch.crates-io]
slint = { git = "https://github.com/slint-ui/slint" }
slint-build = { git = "https://github.com/slint-ui/slint" }
```

Please note: All Slint dependencies need to be on the same revision. To update and run, use `cargo update` and `cargo run`.
Make sure the log shows you are building the right version of Slint.


### C++

For C++ users with a binary package, download the binary from the "Assets" section below.

If you're building from source with CMake's `FetchContent`, change the `GIT_TAG` to `master`:

```cmake
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG master  # Change this to master
    SOURCE_SUBDIR api/cpp
)
```

Remember to remove your build directory and re-run cmake.

### JavaScript / Node.js

Run `npm install slint-ui@nightly` to install or upgrade. This works for new and existing projects.

### Python

Add the following section to your `pyproject.toml` to configure [uv](https://docs.astral.sh/uv/) to build Slint from sources:

```toml
[tool.uv.sources]
slint = { git = "https://github.com/slint-ui/slint", subdirectory = "api/python" }
```

### Editors / IDEs

For VSCode, you download the ["Slint (Nightly)" extension from the Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Slint.slint-nightly).
Note that you need to disable or uninstall an existing version of the Slint  VS Code extension.

For other editors, you compile the latest version of the Slint Language Server with:

```sh
cargo install --git https://github.com/slint-ui/slint slint-lsp
```

Alternatively, download the binary from "Assets" section below.

### Online Demos and Documentation

 - Documentation: https://slint.dev/snapshots/master/docs
 - SlintPad: https://slint.dev/snapshots/master/editor
 - Demos: links from https://github.com/slint-ui/slint/tree/master/examples
