# ![Slint](./logo/slint-logo-full-light.svg#gh-light-mode-only)![Slint](./logo/slint-logo-full-white.svg#gh-dark-mode-only)

<!-- cSpell: ignore ChipTrack Moiré Trolltech valign Woboq -->

[![Build Status](https://github.com/slint-ui/slint/workflows/CI/badge.svg)](https://github.com/slint-ui/slint/actions)
[![REUSE status](https://api.reuse.software/badge/github.com/slint-ui/slint)](https://api.reuse.software/info/github.com/slint-ui/slint)
[![Discussions](https://img.shields.io/github/discussions/slint-ui/slint)](https://github.com/slint-ui/slint/discussions)

Slint is a toolkit to efficiently develop fluid graphical user interfaces for
any display: embedded devices and desktop applications. We support multiple
programming languages, such as Rust, C++, and JavaScript. We invite you
to use Slint and be part of its community.

## Design Goals

We use the following design goals as a guide when developing Slint:

- **Scalable**: Run on any screen connected to a device, from desktop computers
    to low end embedded systems.
- **Lightweight**: Fit into a few hundred kilobytes of RAM and require little
    processing power.
- **Intuitive**: Both developers and designers should feel productive and enjoy
    the design and development process.
    The APIs should be consistent and easy to use, no matter the target language.
- **Native**: Slint apps should match the users' expectations of a native
    application. Various target platforms such as embedded devices, desktops,
    mobile and web should be supported so that both the user and the developer
    feel comfortable on their platform of choice.

## The .slint Markup Language

Slint comes with a markup language that is specifically designed for user
interfaces. This language is easy to learn, to read and write, and provides
a powerful way to describe graphical elements, their placement, and the flow of
data through the different states. It's a familiar syntax to describe the
hierarchy of elements and property bindings.

Here's the obligatory "Hello World":

```slint
export component HelloWorld inherits Window {
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

## Current Status

Slint is in active development. The state of the toolkit for each platform is as
follows:

- **Embedded**: *Ready* Slint is already being used in production projects by
    customers (running on an Arm processor with Linux).
- **Microcontrollers** (MCU): *Ready*. Slint apps can run on MCUs with less than
    300K of RAM.
- **Desktop**: *In Progress*. While Slint is a good fit on Windows, Linux and Mac,
    we are working on improving the platform support in subsequent releases.
- **Mobile** (Android/iOS): *Todo*. We haven't started supporting mobile
    platforms yet, but it is our intention to do so in the near future.
- **Web**: *In Progress*. Slint apps can be compiled to WebAssembly and can run
    in a web browser. As there are many other web frameworks, the web platform
    is not one of our primary target platforms. The web support is currently
    limited to demo purposes.

### Accessibility

Slint supports keyboard based navigation of many widgets, and user interfaces
are scalable. The basic infrastructure for assistive technology like screen
readers is in place, but currently requires the Qt backend.
We're aware that more work is needed to get best-of-class support for users
with special needs.

### Stay up to date

Follow [@slint-ui](https://twitter.com/slint_ui) on Twitter or keep an eye out
for our [🥠 Weekly Status Updates](https://slint-ui.com/thisweek/).

## Documentation

For more details, check out the [Slint Language Documentation](https://slint-ui.com/docs/slint).

The [examples](examples) folder contains examples and demos, showing how to
use the Slint markup language and how to interact with a Slint user interface
from supported programming languages.

The `docs` folder contains a lot more information, including
[build instructions](docs/building.md), and
[internal developer docs](docs/development.md).

Refer to the README of each language directory in the `api` folder:

- [C++](api/cpp) ([Documentation](https://slint-ui.com/docs/cpp) | [Tutorial](https://slint-ui.com/docs/tutorial/cpp) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo) | [Getting Started Template](https://github.com/slint-ui/slint-cpp-template))
- [Rust](api/rs/slint) [![Crates.io](https://img.shields.io/crates/v/slint)](https://crates.io/crates/slint) ([Documentation](https://slint-ui.com/docs/rust/slint/) | [Tutorial](https://slint-ui.com/docs/tutorial/rust) | [Tutorial Video](https://youtu.be/_-Hxr6ZrHyo) | [Getting Started Template](https://github.com/slint-ui/slint-rust-template))
- [JavaScript/NodeJS (Beta)](api/node) [![npm](https://img.shields.io/npm/v/slint-ui)](https://www.npmjs.com/package/slint-ui) ([Documentation](https://slint-ui.com/docs/node) | [Tutorial](https://slint-ui.com/docs/tutorial/node) | [Getting Started Template](https://github.com/slint-ui/slint-nodejs-template))

## Demos

### Embedded

[Video of Slint on RaspberryPi](https://www.youtube.com/watch?v=_BDbNHrjK7g)

### MCU

| STM32                                                                  | RP2040                                                                  |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| [Video of Slint on STM32](https://www.youtube.com/watch?v=NNNOJJsOAis) | [Video of Slint on RP2040](https://www.youtube.com/watch?v=dkBwNocItGs) |

### WebAssembly

| Printer Demo                                                                                                                                           | Slide Puzzle                                                                                                                                       | Energy Monitor                                                                                                                     | Widget Gallery                                                                                                                                 |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| [![Screenshot of the Printer Demo](https://slint-ui.com/resources/printerdemo_screenshot.png "Printer Demo")](https://slint-ui.com/demos/printerdemo/) | [![Screenshot of the Slide Puzzle](https://slint-ui.com/resources/puzzle_screenshot.png "Slide Puzzle")](https://slint-ui.com/demos/slide_puzzle/) | [![Screenshot of the Energy Monitor Demo](https://slint-ui.com/resources/energy-monitor-screenshot.png "Energy Monitor Demo")](https://slint-ui.com/demos/energy-monitor/) | [![Screenshot of the Gallery Demo](https://slint-ui.com/resources/gallery_screenshot.png "Gallery Demo")](https://slint-ui.com/demos/gallery/) |

### Desktop Native Widgets

| Windows                                                                                                      | macOS                                                                                                      | Linux                                                                                                        |
| ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| ![Screenshot of the Gallery on Windows](https://slint-ui.com/resources/gallery_win_screenshot.png "Gallery") | ![Screenshot of the Gallery on macOS](https://slint-ui.com/resources/gallery_mac_screenshot.png "Gallery") | ![Screenshot of the Gallery on Linux](https://slint-ui.com/resources/gallery_linux_screenshot.png "Gallery") |

## Architecture

An application is composed of the business logic written in Rust, C++, or
JavaScript and the `.slint` user interface design markup, which is compiled to
native code.

![Architecture Overview](https://slint-ui.com/resources/architecture.drawio.svg)

### Compiler

The `.slint` files are compiled ahead of time. The expressions in the `.slint`
are pure functions that the compiler can optimize. For example, the compiler
could choose to "inline" properties and remove those that are constant or
unchanged. In the future we hope to improve rendering time on low end devices by
pre-processing images and text. The compiler could determine that a `Text` or an
`Image` element is always on top of another `Image` in the same location.
Consequently both elements could be rendered ahead of time into a single
element, thus cutting down on rendering time.

The compiler uses the typical compiler phases of lexing, parsing, optimization,
and finally code generation. It provides different back-ends for code generation
in the target language. The C++ code generator produces a C++ header file, the
Rust generator produces Rust code, and so on. An interpreter for dynamic
languages is also included.

### Runtime

The runtime library consists of an engine that supports properties declared in
the `.slint` language. Components with their elements, items, and properties are
laid out in a single memory region, to reduce memory allocations.

Rendering backends and styles are configurable at compile time:

- The `femtovg` renderer uses OpenGL ES 2.0 for rendering.
- The `skia` renderer uses [Skia](https://skia.org) for rendering.
- The `software` renderer uses the CPU with no additional dependencies.
- When Qt is installed on the system, the `native` style uses Qt's QStyle to achieve native looking widgets.

### Tooling

We have a few tools to help with the development of .slint files:

- A [**LSP Server**](./tools/lsp) that adds features like auto-complete and live
  preview of the .slint files to many editors.
- It is bundled in a [**Visual Studio Code Extension**](./editors/vscode)
  available from the market place.
- A [**slint-viewer**](./tools/viewer) tool which displays the .slint files. The
  `--auto-reload` argument makes it easy to preview your UI while you are
  working on it (when using the LSP preview is not possible).
- [**SlintPad**](https://slint-ui.com/editor), an online editor to try out .slint syntax
  without installing anything ([sources](./tools/slintpad)).
- An [**updater**](./tools/updater) to convert the .slint files from
  previous versions to newer versions.
- An experimental [**Figma importer**](./tools/figma_import).

Please check our [Editors README](./editors/README.md) for tips on how to
configure your favorite editor to work well with Slint.

## Made with Slint

List of some open source projects using Slint: (Contact us or open a pull
request to add yours)

- **[Cargo UI](https://github.com/slint-ui/cargo-ui)**: A graphical frontend for
  Cargo.
- **[ImageSieve](https://github.com/Futsch1/image-sieve)** : GUI based tool to
  sort and categorize images.
- **[Moiré](https://codeberg.org/moire/moire)** : Musical live performance
  application with a DAW-like timeline interface.
- **[Chiptrack](https://github.com/jturcotte/chiptrack)**: A cross-platform
  sequencer that internally uses a Game Boy emulator to synthesize the sound.
- **[Project Trains Launcher](https://github.com/Project-Trains/launcher)**:
  Cross-platform game launcher made for Project Trains simulator.
- **[Mastermind](https://github.com/ElevenJune/mastermind_Rust)**: Mastermind
  game coded in Rust.
- **[coop_widgets](https://codeberg.org/flovansl/co_sl)**: Custom widget
  library for Slint
  [online demo](https://flovansl.codeberg.page/coop_sl/snapshots/examples/widgets/).

## License

Slint is available under either a [proprietary license](LICENSES/LicenseRef-Slint-commercial.md)
or [GNU GPLv3](LICENSES/GPL-3.0-only.txt)

The proprietary license is provided free-of-charge in exchange for (1) consent that we can showcase the application, (2) attribution of Slint in the application and (3) feedback to improve Slint.
Check out our [ambassador program](https://slint-ui.com/ambassador-program.html).

See also the [Licensing FAQ](FAQ.md#licensing)

## Contributions

We welcome your contributions: in the form of code, bug reports or feedback.

- If you see an [RFC tag](https://github.com/slint-ui/slint/labels/rfc) on an
  issue, feel free to chime in.
- For contribution guidelines see [CONTRIBUTING.md](CONTRIBUTING.md). The
  dual-licensing of Slint requires the contributor to accept our CLA.

## Frequently Asked Questions

Please see our separate [FAQ](FAQ.md).

## About us (SixtyFPS GmbH)

We are passionate about software - API design, cross-platform software
development and user interface components. Our aim is to make developing user
interfaces fun for everyone: from JavaScript, C++, or Rust developers all the
way to UI/UX designers. We believe that software grows organically and keeping
it open source is the best way to sustain that growth. Our team members are
located remotely in Germany.

## Contact us

Feel free to join [Github discussions](https://github.com/slint-ui/slint/discussions)
for general chat or questions. Use [Github issues](https://github.com/slint-ui/slint/issues)
to report public suggestions or bugs.

We chat in [our Mattermost instance](https://chat.slint-ui.com) where you are
welcome to listen in or ask your questions.

You can of course also tweet at [@slint-ui](https://twitter.com/slint_ui) or
contact us privately via email to [info@slint-ui.com](mailto://info@slint-ui.com).
