<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
<!-- cSpell: ignore ccmake dslint femtovg -->

# Live-Preview

`.slint` files are compiled to C++ code when using the [`slint_target_sources()`](cmake_reference.md#slint_target_sources) function.
This is the default and recommended for release builds.

During debugging and development, changes to `.slint` files requires re-compiling and re-starting the application. To speed up
modifications to the UI while connected to the applications' business logic, you can opt into enabling Live-Preview for C++:

1. Compile Slint [from sources](cmake.md#build-from-sources). At the configure step, enable the `SLINT_FEATURE_LIVE_PREVIEW` cmake option.
2. When compiling your application, set the `SLINT_LIVE_PREVIEW=1` environment variable.
3. Start you application. The Slint run-time library will load and reload `.slint` files after you've modified them on disk.
