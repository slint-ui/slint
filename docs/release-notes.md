<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

Slint {version} is a patch release that fixes several bugs and crashes. For a complete list of changes, check out the [ChangeLog](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md).

Upgrading to Slint {version}:

   * Rust: Run `cargo update`.
   * C++: If you're using `FetchContent` with CMake, update the GIT_TAG entry in CMakeLists.txt to `v{version}` or `release/{major_version}.{minor_version}` for automatic tracking of {major_version}.{minor_version}.x releases.
   * Node.js: Run `npm update slint-ui` to bring your `package.json` up-to-date.
   * ESP-IDF developers: Run `idf.py update-dependencies` to bring your idf_component.yml up-to-date.
   * Python: Run `uv lock --upgrade-package slint` to update your [uv](https://docs.astral.sh/uv/) lock file to the latest version.
