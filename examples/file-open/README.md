# File Open (macOS)

This example demonstrates how a Slint application receives the files that the operating
system asks it to open, for example when the user double-clicks a file of an associated
type or chooses "Open With" in Finder.

The example is a small backup-archive inspector: opening a `.slintsave` file (a plain
text list of entries) shows the entries it contains in the window.

## How it works

The application registers a callback with [`slint::set_open_file_handler`]:

```rust
slint::set_open_file_handler(move |paths| {
    // `paths` is a `&[SharedString]` with the requested file paths.
    for path in paths {
        println!("Open request: {path}");
    }
})
```

On **macOS** the framework receives app-open events (which the OS does *not* deliver
through the command line) and forwards them to this callback. On **Windows and Linux**, this callback is not invoked by the framework, so applications typically read the path from `std::env::args()` instead, as in `main.rs`.

## The `.slintsave` extension

For a file type to open in the app, the app bundle has to declare it. On macOS the
build script generates the app bundle's `Info.plist`, which uses the modern Uniform
Type Identifier approach: it exports a `dev.slint.backup-archive` UTI (conforming to
`public.data`) that maps to the `.slintsave` extension and the
`application/x-slint-backup` MIME type, and references that UTI from
`CFBundleDocumentTypes`. On older macOS versions this is equivalent to placing the
extension directly in `CFBundleTypeExtensions`.

## Running on macOS

File-open events are only delivered to an `.app` app bundle, never to a bare binary.
Build and assemble a bundle with the generated `Info.plist`:

```sh
cargo build --manifest-path examples/Cargo.toml -p file-open
BIN=target/debug/file-open
OUT_DIR=$(find target/debug/build -path '*/file-open-*/out' | head -1)
rm -rf target/FileOpen.app
mkdir -p target/FileOpen.app/Contents/MacOS
cp "$BIN" target/FileOpen.app/Contents/MacOS/
cp "$OUT_DIR/Info.plist" target/FileOpen.app/Contents/
open target/FileOpen.app
```

To trigger a file-open request, create a `.slintsave` file with some lines and open it:

```sh
printf 'save-game-1\nsave-game-2\nsettings.json\n' > example.slintsave
open example.slintsave
```

The app window should list the contents of `example.slintsave`. The first time you open
a `.slintsave` file, macOS asks whether you want to associate that extension with the
Backup Inspector app; choose "Open With" to route it there.

## Running on Windows / Linux

These platforms pass the file on the command line, which the example also handles:

```sh
cargo run --manifest-path examples/Cargo.toml -p file-open -- example.slintsave
```
