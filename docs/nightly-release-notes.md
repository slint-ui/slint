
This GitHub release is a nightly snapshot of Slint development. It serves to provide access to pre-release binaries.

The attached binaries are updated once a day by a GitHub action building from the  `{branch}` branch.

## How To Try Out This Development Release

### Rust

For Rust users, include the following in your Cargo.toml (or .cargo/config.toml):

```toml
[patch.crates-io]
slint = { git = "https://github.com/slint-ui/slint", branch = "{branch}" }
slint-build = { git = "https://github.com/slint-ui/slint", branch = "{branch}" }
```

Please note: All Slint dependencies need to be on the same revision. To update and run, use `cargo update` and `cargo run`.
Make sure the log shows you are building the right version of Slint.


### C++

For C++ users with a binary package, download the binary from the "Assets" section below.

If you're building from source with CMake's `FetchContent`, change the `GIT_TAG` to `{branch}`:

```cmake
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG {branch}  # Change this to {branch}
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
slint = { git = "https://github.com/slint-ui/slint", subdirectory = "api/python/slint" }
```

### Editors / IDEs

For VSCode, you download the ["Slint (Nightly)" extension from the Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Slint.slint-nightly).
Note that you need to disable or uninstall an existing version of the Slint  VS Code extension.

For other editors, you compile the latest version of the Slint Language Server with:

```sh
cargo install --locked --git https://github.com/slint-ui/slint slint-lsp --branch {branch}
```

Alternatively, download the binary from "Assets" section below.

### AI Coding Assistants

Install the Slint plugin from the `nightly` branch of [slint-ui/ai-plugins](https://github.com/slint-ui/ai-plugins/tree/nightly).
It follows `master`, so the skill covers unreleased features and the Slint preview runs the latest development build.

 - Claude Code: `/plugin marketplace add slint-ui/ai-plugins#nightly`, then `/plugin install slint@slint`
 - Codex: `codex plugin marketplace add slint-ui/ai-plugins@nightly`, then `codex plugin add slint@slint`
 - Antigravity: `git clone --branch nightly https://github.com/slint-ui/ai-plugins`, then `agy plugin install ./ai-plugins`
 - GitHub CLI: `gh skill install slint-ui/ai-plugins slint --pin nightly`; reinstall with `--force` to update
 - skills CLI: `npx skills add https://github.com/slint-ui/ai-plugins/tree/nightly --skill slint`

Claude Code and Codex accept only one `slint` marketplace.
If you added the release one, remove it first with `/plugin marketplace remove slint` or `codex plugin marketplace remove slint`.

### Online Demos and Documentation

 - Documentation: https://slint.dev/snapshots/{feature}/docs
 - SlintPad: https://slint.dev/snapshots/{feature}/editor
 - Demos: links from https://github.com/slint-ui/slint/tree/master/examples
 - Android demo APKs: https://snapshots.slint.dev/{feature}/demos/android/
 - Slint Viewer for Android: download `slint-viewer-arm64-v8a.apk` (most phones), `slint-viewer-armeabi-v7a.apk`
   or `slint-viewer-x86_64.apk` from the "Assets" section below
