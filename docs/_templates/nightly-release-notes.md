This GitHub release is a nightly snapshot of Slint development. It serves to provide access to pre-release binaries.

The attached binaries are updated once a day by a GitHub action building from the  `master` branch.

## How To Try Out Nightly

### Rust

For Rust users, please include the following in your Cargo.toml (or .cargo/config.toml):

```toml
[patch.crates-io]
slint = { git = "https://github.com/slint-ui/slint" }
slint-build = { git = "https://github.com/slint-ui/slint" }
```

Please note: All Slint dependencies need to be on the same revision. To update and run, use `cargo update` and `cargo run`.
Make sure the log shows you are building the right version of Slint.


### C++

For C++ users with a binary package, download the binary from the "Assets" bellow

If you're building from source with FetchContent, change the `GIT_TAG` to `master`:

```cmake
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG master  # Change this to master
    SOURCE_SUBDIR api/cpp
)
```

Remember to remove your build directory and re-run cmake.

### Javascript

Use the latest nightly version of the [slint-ui npm package](https://www.npmjs.com/package/slint-ui?activeTab=versions)

### Editor extension

For VSCode, you can download the ["Slint (Nightly)" extension from the Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Slint.slint-nightly).
Note that you need to disable or uninstall the non-nightly one.

For other editors, you can compile the nightly version with:

```sh
cargo install --git https://github.com/slint-ui/slint slint-lsp
```

Or download the binary from "Assets" bellow

### Online demos and docs

 - Docs: https://slint.dev/snapshots/master/docs
 - Slintpad: https://slint.dev/snapshots/master/editor
 - Demos: links from https://github.com/slint-ui/slint/tree/master/examples
