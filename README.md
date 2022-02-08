# ![Slint](./logo/slint-logo-full-light.svg)

[![Build Status](https://github.com/slint-ui/slint/workflows/CI/badge.svg)](https://github.com/slint-ui/slint/actions)
[![Discussions](https://img.shields.io/github/discussions/slint-ui/slint)](https://github.com/slint-ui/slint/discussions)

Slint is a toolkit to efficiently develop fluid graphical user interfaces for any display: embedded devices and desktop applications. We support multiple programming languages, such as
Rust, C++, and JavaScript.

Our design goals are:

 - **Lightweight**: Fit into a few hundred kilobytes of RAM and require little processing power.
 - **Straightforward**: Programmers and designers should feel productive and be able to enjoy the design and development process.
   The APIs should be consistent, easy to use, and intuitive, no matter the target language. High-quality documentation
   should describe the APIs, teach concepts and how to use them.
 - **Native**: We support many different target platforms, from embedded devices to desktops including mobile and web. Both the user and the developer should feel at
   home on each platform. The look and feel and experience should match the users' expectations of a
   native application.

## Current Status

Slint is in active development. Its state of development depends on the kind of application.

 - **Embedded**: *Ready!* Slint is already happily used by customers on embedded devices
   (running on an Arm processor with Linux). We are also looking into supporting microcontrollers.
 - **Desktop**: *In Progress*. Slint is usable on Windows, Linux and Mac. We plan on building
    up the desktop support in subsequent releases to reach the level of integration we're aiming for.
 - **Mobile** (Android/iOS): *Todo*. We haven't started supporting mobile platforms yet, but it
   is our intention to do so in the future.
 - **Web**: Our focus is on Embedded and Desktop applications. We do not intend to compete in
   the web space as a primary platform. That said, Slint can compile to WebAssembly and the
   demos are running in a web browser. But the web support is currently limited to demo purposes.

## Documentation

Refer to the README of each language directory in the `api` sub-folder:

 * [SixtyFPS-cpp](api/cpp) ([Documentation](https://slint-ui.com/docs/cpp) | [Tutorial](https://slint-ui.com/docs/tutorial/cpp) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo) | [Getting Started Template](https://github.com/sixtyfpsui/sixtyfps-cpp-template))
 * [SixtyFPS-rs](api/sixtyfps-rs) [![Crates.io](https://img.shields.io/crates/v/sixtyfps)](https://crates.io/crates/sixtyfps) ([Documentation](https://slint-ui.com/docs/rust/sixtyfps/) | [Tutorial](https://slint-ui.com/docs/tutorial/rust) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo) | [Getting Started Template](https://github.com/sixtyfpsui/sixtyfps-rust-template))
 * [SixtyFPS-node](api/node) [![npm](https://img.shields.io/npm/v/sixtyfps)](https://www.npmjs.com/package/sixtyfps) ([Documentation](https://slint-ui.com/docs/node))

The [examples](/examples) folder contains examples and demos. The `docs` folder contains [build instructions](docs/building.md) and [internal developer docs](docs/development.md).


## Demos running in WebAssembly Simulation

Click on the screenshots to run the WebAssembly simulation

| Printer Demo | Slide Puzzle | Todo | Widget Gallery |
|--------------|----------------|----------------|----------------|
| [![Screenshot of the Printer Demo](https://slint-ui.com/resources/printerdemo_screenshot.png "Printer Demo")](https://slint-ui.com/demos/printerdemo/) | [![Screenshot of the Slide Puzzle](https://slint-ui.com/resources/puzzle_screenshot.png "Slide Puzzle")](https://slint-ui.com/demos/slide_puzzle/) | [![Screenshot of the Todo Demo](https://slint-ui.com/resources/todo_screenshot.png "Todo Demo")](https://slint-ui.com/demos/todo/) | [![Screenshot of the Gallery Demo](https://slint-ui.com/resources/gallery_screenshot.png "Gallery Demo")](https://slint-ui.com/demos/gallery/) |

## Desktop Native Widgets

| Windows | macOS | Linux |
|---------|-------|-------|
| ![Screenshot of the Gallery on Windows](https://slint-ui.com/resources/gallery_win_screenshot.png "Gallery") | ![Screenshot of the Gallery on macOS](https://slint-ui.com/resources/gallery_mac_screenshot.png "Gallery") | ![Screenshot of the Gallery on Linux](https://slint-ui.com/resources/gallery_linux_screenshot.png "Gallery") |

## The .slint Markup Language

Slint comes with a markup language that is specifically designed for user interfaces. This language provides a
powerful way to describe graphical elements, their placement, and the flow of data through the different states. It is a familiar syntax to describe the hierarchy of elements and property bindings. Here's the obligatory "Hello World":

```slint
HelloWorld := Window {
    width: 400px;
    height: 400px;

    Text {
       y: parent.width / 2;
       x: parent.x + 200px;
       text: "Hello, world";
       color: blue;
    }
}
```

Check out the [language reference](docs/langref.md) for more details.

## Architecture

An application is composed of the business logic written in Rust, C++, or JavaScript and the `.slint` user interface design markup, which
is compiled to native code.

![Architecture Overview](https://slint-ui.com/resources/architecture.drawio.svg)

### Compiler

The `.slint` files are compiled ahead of time. The expressions in the `.slint` are pure functions that the
compiler can optimize. For example, the compiler could choose to "inline" properties and remove those
that are constant or unchanged. In the future we hope to improve rendering time on low end devices by
pre-processing images and text. The compiler could determine that a `Text` or an `Image` element is
always on top of another `Image` in the same location. Consequently both elements could be rendered ahead
of time into a single element, thus cutting down on rendering time.

The compiler uses the typical compiler phases of lexing, parsing, optimization, and finally code
generation. It provides different back-ends for code generation in the target language. The C++ code
generator produces a C++ header file, the Rust generator produces Rust code, and so on. An interpreter
for dynamic languages is also included.

### Runtime

The runtime library consists of an engine that supports properties declared in the `.slint` language.
Components with their elements, items, and properties are laid out in a single memory region, to reduce
memory allocations.

Rendering backends and styles are configurable at compile time. Current there are two back-ends:

 * The `gl` backend uses OpenGL ES 2.0 for rendering.
 * The `qt` backend uses Qt's QStyle to achieve native looking widgets. In the future it could also use
   QPainter.

### Tooling

We have a few tools to help with the development of .slint files:
 - A [**LSP Server**](./tools/lsp) that adds things like auto-complete and live preview of the .slint files to many editors
 - It is bundled in a [**Visual Studio Code Extension**](./vscode_extension) accessible from the market place
 - A [**slint-viewer**](./tools/viewer) tool which display the .slint files. With the `--auto-reload` argument, makes it easy to preview
   your UI as you are working it (when using the LSP preview is not possible)
 - An [**online editor**](https://slint-ui.com/editor) to try out .slint syntax without installing anything ([sources](./tools/online_editor))
 - An [**updater**](./tools/syntax_updater) to convert the .slint files from previous version to the newer version
 - An experimental [**Figma importer**](./tools/figma_import)

## Contributions

We welcome your contributions: in the form of code, bug reports or feedback.

 * If you see an [RFC tag](https://github.com/slint-ui/slint/labels/rfc) on an issue, feel free to
   chime in.
 * For contribution guidelines see [CONTRIBUTING.md](CONTRIBUTING.md). The dual-licensing requires the
   contributor to accept a CLA.

## License

This software is provided under a dual licensing scheme:

 - **GNU GPLv3**: Open source license ideal for free software.
 - **Commercial Slint license**: <https://slint-ui.com/#offering>

See also the [Licensing FAQ](FAQ.md#licensing)

## Frequently Asked Questions

Please see our separate [FAQ](FAQ.md).

## About us

Olivier and Simon started their open source journey in the KDE project, the popular Open Source Desktop Environment
for Linux. Later they met while working together in a small company in Norway called Trolltech, on the Qt C++ toolkit.
Here, they gained valuable experience in API design, cross-platform software development and user interface components.
Simon continued in the Qt Company as one lead developer and maintainer of the QtQml engine, while Olivier
co-founded [Woboq](https://woboq.com), a software consulting company. Years later, and now based in Berlin, they are starting a
new project. With Slint they aim to make developing user interfaces fun for everyone: from JavaScript, C++, or
Rust developers all the way to UI/UX designers.

## Contact us

Feel free to join [Github discussions](https://github.com/slint-ui/slint/discussions) for general chat or questions. Use [Github issues](https://github.com/slint-ui/slint/issues) to report public suggestions or bugs.

To contact us privately send an email to info@sixtyfps.io

For chat, we also have [our Mattermost instance](https://chat.sixtyfps.io)
