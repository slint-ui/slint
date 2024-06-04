<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Tutorials

The source code for the Rust, C++, and Node.js versions of the Memory Game tutorial are located in
the respect rust, cpp, and node sub-directories. They are built using `mdbook`.

# Requirements

Building the tutorial requires `mdbook`, which you can install with `cargo`:

```sh
cargo install mdbook
```

# Building

To build the tutorial, enter the `rust`, `cpp`, or `node` sub-directory and run:

```sh
mdbook build
```

The output will be in the `book/html` subdirectory. To check it out, open it in your web browser.

# Code Samples

The code in the tutorial is available in separate steps in .rs, .cpp, and .js files.

The .rs files are mapped to different binaries, so you if you change into the `rust/src`
sub-directory, then `cargo run` will present you with binaries for the different steps.

The .cpp files are built using `cpp/src/CMakeLists.txt`, which is included from the top-level
`CMakeLists.txt`.
