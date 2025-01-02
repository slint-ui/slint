<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint Bazel Build example

This shows how to build a trivial slint application in the bazel build system.

Requires you to have a bazel toolchain setup. 

This example currently does NOT use the copy of slint in this repo; it pulls it in from crates.io.

It also pulls in a mostly hermetic rust toolchain, so expect the build to take considerably longer than the other examples the first time you build.

To run the example, run the following command from any subdirectory of this directory (examples/bazel_build):

```bash
bazel run //src:slint_bazel_example
```
