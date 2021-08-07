# SixtyFPS  <!-- omit in toc -->

[![Build Status](https://github.com/sixtyfpsui/sixtyfps/workflows/CI/badge.svg)](https://github.com/sixtyfpsui/sixtyfps/actions)
[![Discussions](https://img.shields.io/github/discussions/sixtyfpsui/sixtyfps)](https://github.com/sixtyfpsui/sixtyfps/discussions)

SixtyFPS is a toolkit to efficiently develop fluid graphical user interfaces for any display: embedded devices and desktop applications. We support multiple programming languages, such as
Rust, C++, and JavaScript.

Our design goals are:

 - **Lightweight**: Fit into a few hundred kilobytes of RAM and require little processing power.
 - **Straightforward**: Programmers and designers should feel productive and be able to enjoy the design and development process.
   The APIs should be consistent, easy to use, and intuitive, no matter the target language. High-quality documentation
   should describe the APIs, teach concepts and how to use them.
 - **Native**: We support many different target platforms, from embedded devices to desktops including mobile and web. Both the user and the developer should feel at
   home on each platform. The look and feel and experience should match the users´ expectations of a
   native application.

### Current Status

It´s possible to create user interfaces from C++, Rust, or NodeJS. These user interfaces can be compiled and
shown on Linux, macOS, Windows, and in Web Browsers (using WebAssembly). You can also try out SixtyFPS using
our [online editor](https://sixtyfps.io/editor).

We plan to support the development of this project through dual-licensing and services. We seek feedback
from potential customers or users.

## Table Of Contents  <!-- omit in toc -->

- [Documentation](#documentation)
- [Demos running in WebAssembly Simulation](#demos-running-in-webassembly-simulation)
- [Desktop Native Widgets](#desktop-native-widgets)
- [The .60 Markup Language](#the-60-markup-language)
- [Architecture](#architecture)
  - [Compiler](#compiler)
  - [Runtime](#runtime)
  - [Tooling](#tooling)
- [Contributions](#contributions)
- [License](#license)
- [Frequently Asked Questions](#frequently-asked-questions)
- [About us](#about-us)
- [Contact us](#contact-us)

## Documentation

Refer to the README of each language directory in the `api` sub-folder:

 * [SixtyFPS-cpp](api/sixtyfps-cpp) ([Documentation](https://sixtyfps.io/docs/cpp) | [Tutorial](https://sixtyfps.io/docs/tutorial/cpp) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo))
 * [SixtyFPS-rs](api/sixtyfps-rs) [![Crates.io](https://img.shields.io/crates/v/sixtyfps)](https://crates.io/crates/sixtyfps) ([Documentation](https://sixtyfps.io/docs/rust/sixtyfps/) | [Tutorial](https://sixtyfps.io/docs/tutorial/rust) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo))
 * [SixtyFPS-node](api/sixtyfps-node) [![npm](https://img.shields.io/npm/v/sixtyfps)](https://www.npmjs.com/package/sixtyfps) ([Documentation](https://sixtyfps.io/docs/node))

The [examples](/examples) folder contains examples and demos. The `docs` folder contains [build instructions](docs/building.md) and [internal developer docs](docs/development.md).


## Demos running in WebAssembly Simulation

Click on the screenshots to run the WebAssembly simulation

| Printer Demo | Slide Puzzle | Todo | Widget Gallery |
|--------------|----------------|----------------|----------------|
| [![Screenshot of the Printer Demo](https://sixtyfps.io/resources/printerdemo_screenshot.png "Printer Demo")](https://sixtyfps.io/demos/printerdemo/) | [![Screenshot of the Slide Puzzle](https://sixtyfps.io/resources/puzzle_screenshot.png "Slide Puzzle")](https://sixtyfps.io/demos/slide_puzzle/) | [![Screenshot of the Todo Demo](https://sixtyfps.io/resources/todo_screenshot.png "Todo Demo")](https://sixtyfps.io/demos/todo/) | [![Screenshot of the Gallery Demo](https://sixtyfps.io/resources/gallery_screenshot.png "Gallery Demo")](https://sixtyfps.io/demos/gallery/) |

## Desktop Native Widgets

| Windows | macOS | Linux |
|---------|-------|-------|
| ![Screenshot of the Gallery on Windows](https://sixtyfps.io/resources/gallery_win_screenshot.png "Gallery") | ![Screenshot of the Gallery on macOS](https://sixtyfps.io/resources/gallery_mac_screenshot.png "Gallery") | ![Screenshot of the Gallery on Linux](https://sixtyfps.io/resources/gallery_linux_screenshot.png "Gallery") |

## The .60 Markup Language

SixtyFPS comes with a markup language that is specifically designed for user interfaces. This language provides a
powerful way to describe graphical elements, their placement, and the flow of data through the different states. It is a familiar syntax to describe the hierarchy of elements and property bindings. Here´s the obligatory "Hello World":

```60
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

An application is composed of the business logic written in Rust, C++, or JavaScript and the `.60` user interface design markup, which
is compiled to native code.

![Architecture Overview](https://sixtyfps.io/resources/architecture.drawio.svg)

### Compiler

The `.60` files are compiled ahead of time. The expressions in the `.60` are pure functions that the
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

The runtime library consists of an engine that supports properties declared in the `.60` language.
Components with their elements, items, and properties are laid out in a single memory region, to reduce
memory allocations.

Rendering backends and styles are configurable at compile time. Current there are two back-ends:

 * The `gl` backend uses OpenGL ES 2.0 for rendering.
 * The `qt` backend uses Qt´s QStyle to achieve native looking widgets. In the future it could also use
   QPainter.

### Tooling

We have a few tools to help with the development of .60 files:
 - A [**LSP Server**](./tools/lsp) that adds things like auto-complete and live preview of the .60 files to many editors
 - It is bundled in a [**Visual Studio Code Extension**](./vscode_extension) accessible from the market place
 - A [**sixtyfps-viewer**](./tools/viewer) tool which display the .60 files. With the `--auto-reload` argument, makes it easy to preview
   your UI as you are working it (when using the LSP preview is not possible)
 - An [**online editor**](https://sixtyfps.io/editor) to try out .60 syntax without installing anything ([sources](./tools/online_editor))
 - An [**updater**](./tools/syntax_updater) to convert the .60 files from previous version to the newer version
 - An experimental [**Figma importer**](./tools/figma_import)

## Contributions

We welcome your contributions: in the form of code, bug reports or feedback.

 * If you see an [RFC tag](https://github.com/sixtyfpsui/sixtyfps/labels/rfc) on an issue, feel free to
   chime in.
 * For contribution guidelines see [CONTRIBUTING.md](CONTRIBUTING.md). The dual-licensing requires the
   contributor to accept a CLA.

## License

This software is provided under a dual licensing scheme:

 - **GNU GPLv3**: Open source license ideal for free software.
 - **Commercial SixtyFPS license**: <https://sixtyfps.io/#offering>

See also the [Licensing FAQ](FAQ.md#licensing)

## Frequently Asked Questions

Please see our separate [FAQ](FAQ.md).

## About us

Olivier and Simon started their open source journey in the KDE project, the popular Open Source Desktop Environment
for Linux. Later they met while working together in a small company in Norway called Trolltech, on the Qt C++ toolkit.
Here, they gained valuable experience in API design, cross-platform software development and user interface components.
Simon continued in the Qt Company as one lead developer and maintainer of the QtQml engine, while Olivier
co-founded [Woboq](https://woboq.com), a software consulting company. Years later, and now based in Berlin, they are starting a
new project. With SixtyFPS they aim to make developing user interfaces fun for everyone: from JavaScript, C++, or
Rust developers all the way to UI/UX designers.

## Contact us

Feel free to join [Github discussions](https://github.com/sixtyfpsui/sixtyfps/discussions) for general chat or questions. Use [Github issues](https://github.com/sixtyfpsui/sixtyfps/issues) to report public suggestions or bugs.

To contact us privately send an email to info@sixtyfps.io

For chat, we also have [our Mattermost instance](https://chat.sixtyfps.io)
