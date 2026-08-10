
# Viewer for Slint

This program is a viewer for `.slint` files from the [Slint project](https://slint.dev).

## Installation

Install the binary from crates.io:

```bash
cargo install slint-viewer
```

Pre-built binaries for Linux and Windows are attached to each [release](https://github.com/slint-ui/slint/releases).

## Usage

```bash
slint-viewer path/to/myfile.slint
```

See the [Slint Viewer documentation](https://docs.slint.dev/latest/docs/slint/guide/tooling/slint-viewer/)
for live reload, screenshots, command-line options, callback handlers, dialogs, exit codes,
and the remote viewer.

## Building for iOS

On iOS the viewer previews `.slint` files from your editor on an iPhone, iPad, or the simulator.

### Prerequisites

- A computer running macOS with an up-to-date installation of [Xcode](https://developer.apple.com/xcode/).
- [XcodeGen](https://github.com/yonaskolb/XcodeGen) (e.g. `brew install xcodegen`).
- [Rust](https://rustup.rs) with the iOS toolchains:
  `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.

### Build and run

Generate the Xcode project and open it:

```bash
cd tools/viewer
xcodegen generate --spec ios-project.yml
open "Slint Viewer.xcodeproj"
```

From Xcode, build and run on the simulator or a device.
