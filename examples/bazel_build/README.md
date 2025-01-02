<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint Bazel Build example

This shows how to build a trivial slint application in the [bazel build system](https://bazel.build/)

Requires you to have a bazel toolchain setup. Installation instructions can be found [here](https://bazel.build/install)

This example has been tested with:  
Slint 1.7.0  
Ubuntu 24.04.1  
Bazel 7.2.1 (specified in the file .bazelversion)

This example currently does NOT use the copy of slint in this repo; it pulls it in from crates.io.

It also pulls in a mostly hermetic rust toolchain, so expect the build to take considerably longer than the other examples the first time you build.

To run the example, run the following command from any subdirectory of this directory (examples/bazel_build):

```bash
bazel run //src:slint_bazel_example
```
