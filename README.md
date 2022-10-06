# ![Slint](./logo/slint-logo-full-light.svg#gh-light-mode-only)![Slint](./logo/slint-logo-full-dark.svg#gh-dark-mode-only)

<!-- cSpell: ignore ChipTrack Moiré Trolltech valign Woboq -->

<table border="0" style="border:none"><tr><td valign="center">Formerly known as </td><td><img height="40" src="https://slint-ui.com/resources/logo.drawio.svg" style="vertical-alignment: middle"></td></tr></table>

[![Build Status](https://github.com/slint-ui/slint/workflows/CI/badge.svg)](https://github.com/slint-ui/slint/actions)
[![REUSE status](https://api.reuse.software/badge/github.com/slint-ui/slint)](https://api.reuse.software/info/github.com/slint-ui/slint)
[![Discussions](https://img.shields.io/github/discussions/slint-ui/slint)](https://github.com/slint-ui/slint/discussions)

Slint is a toolkit to efficiently design, develop and deploy responsive apps on any screen. Frontends built with Slint can be easily integrated with backends written in different programming languages such as C/C++, Rust, and JavaScript. Slint is developed by SixtyFPS GmbH together with the community. We invite you to use Slint and be part of its community.

## Design Goals

We use the following design goals as a guide when developing Slint

 - **Scalable**: Run on any screen connected to a device, from desktop computers to low end embedded systems.
 - **Lightweight**: Fit into a few hundred kilobytes of RAM and require little processing power.
 - **Intuitive**: Both developers and designers should feel productive and enjoy the design and development process.
   The APIs should be consistent and easy to use, no matter the target language.
 - **Native**: Slint apps should match the users' expectations of a native application. Various target platforms such as 
   embedded devices, desktops, mobile and web should be supported so that both the user and the developer feel comfortable on
   their platform of choice.

## Get Started

Check the [online demos](#demos-running-in-webassembly-simulation) or
[build on your own system](docs/building.md). Also check out the [documentation](#documentation).

## Current Status

Slint is in active development. The state of the toolkit for each platform is as follows:

 - **Embedded**: *Ready!* Slint is already being used in production projects by customers 
   (running on an Arm processor with Linux).
 - **Microcontrollers** (MCU): *Ready*. Slint apps can run on MCUs with less than 300K of RAM.
   [Video of Slint on STM32](https://www.youtube.com/watch?v=NNNOJJsOAis) –
   [Video of Slint on RP2040](https://www.youtube.com/watch?v=dkBwNocItGs) – 
   [docs](https://slint-ui.com/docs/rust/slint/docs/mcu/index.html)
 - **Desktop**: *In Progress*. While Slint is usable on Windows, Linux and Mac, we are working on
    improving the platform support in subsequent releases to reach the level of integration that
    have aimed for.
 - **Mobile** (Android/iOS): *Todo*. We haven't started supporting mobile platforms yet, but it
   is our intention to do so in the near future.
 - **Web**: *In Progress*. Slint apps can be compiled to WebAssembly and can run in a web browser.
   As there are many other web frameworks, the web platform is not one of our primary target platforms.
   The web support is currently limited to demo purposes.

### Accessibility
 Slint supports keyboard based navigation of many widgets, and user interfaces are scalable. 
 The basic infrastructure for assistive technology like screen readers is in place, but currently requires 
 the Qt backend.
 We are aware that more work is needed to get best-of-class support for users with special needs.

### Stay up to date

Follow [@slint-ui](https://twitter.com/slint_ui) on twitter or keep an eye out for
our [🥠 Weekly Status Updates](https://slint-ui.com/thisweek/).

## Documentation

The [examples](examples) folder contains examples and demos.

The `docs` folder contains a lot more information, including
[build instructions](docs/building.md), [recipes for common tasks](docs/recipes/recipes.md) and
[internal developer docs](docs/development.md).

Refer to the README of each language directory in the `api` folder:

 * [C++](api/cpp) ([Documentation](https://slint-ui.com/docs/cpp) | [Tutorial](https://slint-ui.com/docs/tutorial/cpp) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo) | [Getting Started Template](https://github.com/slint-ui/slint-cpp-template))
 * [Rust](api/rs/slint) [![Crates.io](https://img.shields.io/crates/v/slint)](https://crates.io/crates/slint) ([Documentation](https://slint-ui.com/docs/rust/slint/) | [Tutorial](https://slint-ui.com/docs/tutorial/rust) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo) | [Getting Started Template](https://github.com/slint-ui/slint-rust-template))
 * [JavaScript/NodeJS](api/node) [![npm](https://img.shields.io/npm/v/slint-ui)](https://www.npmjs.com/package/sixtyfps) ([Documentation](https://slint-ui.com/docs/node))

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
 - A [**LSP Server**](./tools/lsp) that adds features like auto-complete and live preview of the .slint files to many editors.
 - It is bundled in a [**Visual Studio Code Extension**](./editors/vscode) available from the market place.
 - A [**slint-viewer**](./tools/viewer) tool which displays the .slint files. The `--auto-reload` argument makes it easy to preview
   your UI while you are working on it (when using the LSP preview is not possible).
 - An [**online editor**](https://slint-ui.com/editor) to try out .slint syntax without installing anything ([sources](./tools/online_editor)).
 - An [**updater**](./tools/syntax_updater) to convert the .slint files from previous versions to newer versions.
 - An experimental [**Figma importer**](./tools/figma_import).

Please check our [Editors README](./editors/README.md) for tips on how to configure your favorite editor to work well with Slint.

## Made with Slint

List of some open source projects using Slint: (Contact us or open a pull request to add yours)

 - **[Cargo UI](https://github.com/slint-ui/cargo-ui)**: A graphical frontend for Cargo.
 - **[ImageSieve](https://github.com/Futsch1/image-sieve)** : GUI based tool to sort and categorize images.
 - **[Moiré](https://codeberg.org/Be.ing/moire)** : Musical live performance application with a DAW-like timeline interface.
 - **[Chiptrack](https://github.com/jturcotte/chiptrack)**: A cross-platform sequencer that internally uses a Game Boy emulator to synthesize the sound.
 - **[Project Trains Launcher](https://github.com/Project-Trains/launcher)**: Cross-platform game launcher made for Project Trains simulator.
 - **[Mastermind](https://github.com/ElevenJune/mastermind_Rust)**: Mastermind game coded in Rust.

## License

Slint can be used under either a [commercial license](./LICENSES/LicenseRef-Slint-commercial.md) or [GNU GPLv3](./LICENSES/GPL-3.0-only.txt), at your choice.

The commercial license is provided for free if you help us promote Slint: Check out our
[ambassador license](https://slint-ui.com/ambassador-program.html).

See also the [Licensing FAQ](FAQ.md#licensing)

## Contributions

We welcome your contributions: in the form of code, bug reports or feedback.

 * If you see an [RFC tag](https://github.com/slint-ui/slint/labels/rfc) on an issue, feel free to
   chime in.
 * For contribution guidelines see [CONTRIBUTING.md](CONTRIBUTING.md). The dual-licensing of Slint requires the
   contributor to accept a CLA.

## Frequently Asked Questions

Please see our separate [FAQ](FAQ.md).

## About us

We are passionate about software - API design, cross-platform software development and user interface components.
Our aim to make developing user interfaces fun for everyone: from JavaScript, C++, or Rust developers all the 
way to UI/UX designers. We believe that software grows organically and keeping it open source is the best way 
to sustain that growth. Our team members are located remotely in Germany.

## Contact us

Feel free to join [Github discussions](https://github.com/slint-ui/slint/discussions) for general chat or questions. Use [Github issues](https://github.com/slint-ui/slint/issues) to report public suggestions or bugs.

We chat in [our Mattermost instance](https://chat.slint-ui.com) where you are welcome to listen in or ask your questions.

You can of course also tweet at [@slint-ui](https://twitter.com/slint_ui) or contact us privately via email to [info@slint-ui.com](mailto://info@slint-ui.com).
