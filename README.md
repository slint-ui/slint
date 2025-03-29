<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

![Slint](./logo/slint-logo-full-light.svg#gh-light-mode-only) ![Slint](./logo/slint-logo-full-dark.svg#gh-dark-mode-only)

[![Build Status](https://github.com/slint-ui/slint/workflows/CI/badge.svg)](https://github.com/slint-ui/slint/actions)
[![REUSE status](https://api.reuse.software/badge/github.com/slint-ui/slint)](https://api.reuse.software/info/github.com/slint-ui/slint)
[![Discussions](https://img.shields.io/github/discussions/slint-ui/slint)](https://github.com/slint-ui/slint/discussions)

Slint is a declarative GUI toolkit to build native user interfaces for embedded,
desktop, and mobile applications written in Rust, C++, JavaScript, or Python.

The name *Slint* is derived from our design goals:

- **Scalable**: Slint should support responsive UI design, allow cross-platform
    usage across operating systems and processor architectures and support
    multiple programming languages.
- **Lightweight**: Slint should require minimal resources, in terms of memory
    and processing power, and yet deliver a smooth, smartphone-like user
    experience on any device.
- **Intuitive**: Designers and developers should feel productive while enjoying
    the GUI design and development process. The design creation tools should be
    intuitive to use for the designers. Similarly for the developers, the APIs
    should be consistent and easy to use, no matter which programming language
    they choose.
- **Native**: GUI built with Slint should match the end users' expectations of a
    native application irrespective of the platform - desktop, mobile, web or
    embedded system. The UI design should be compiled to machine code and provide
    flexibility that only a native application can offer: Access full operating
    system APIs, utilize all CPU and GPU cores, connect to any peripheral.

Visit [#MadeWithSlint](https://madewithslint.com) to view some of the projects
using Slint. We invite you to use Slint and be part of its community.

## Current Status

Slint is in active development. The state of support for each platform is as
follows:

- **Embedded**: *Ready*. Slint is being used by customers in production on embedded
    devices running embedded Linux and Windows. The Slint run-time requires less than
    300KiB of RAM and can run on different processor architectures such as ARM Cortex M,
    ESP32, STM32 from the MCU category to ARM Cortex A, Intel x86 from the MPU category.
- **Desktop**: *In Progress*. While Slint is a good fit on Windows, Linux and Mac,
    we are working on improving the platform support in subsequent releases.
- **Web**: *In Progress*. Slint apps can be compiled to WebAssembly and can run
    in a web browser. As there are many other web frameworks, the web platform
    is not one of our primary target platforms. The web support is currently
    limited to demo purposes.
- **Mobile**
  - Android: *In Progress*. Track the progress of work here https://github.com/slint-ui/slint/issues/46.
  - iOS: *Todo*. Support for iOS will commence after the initial support for Android is completed.

### Accessibility

Slint supports keyboard based navigation of many widgets, and user interfaces
are scalable. The basic infrastructure for assistive technology like screen
readers is in place. We're aware that more work is needed to get best-of-class
support for users with special needs.

## Demos

### Embedded

| RaspberryPi                          | STM32                         | RP2040                         |
| ------------------------------------ | ----------------------------- | ------------------------------ |
| [Video of Slint on Raspberry Pi][#1] | [Video of Slint on STM32][#2] | [Video of Slint on RP2040][#3] |

### Desktop

| Windows                                     | macOS                                     | Linux                                     |
| ------------------------------------------- | ----------------------------------------- | ----------------------------------------- |
| ![Screenshot of the Gallery on Windows][#4] | ![Screenshot of the Gallery on macOS][#5] | ![Screenshot of the Gallery on Linux][#6] |

### Web using WebAssembly

| Printer Demo                                | Slide Puzzle                                 | Energy Monitor                                       | Widget Gallery                                | Weather demo                                  |
| ------------------------------------------- | -------------------------------------------- | ---------------------------------------------------- | --------------------------------------------- | --------------------------------------------- |
| [![Screenshot of the Printer Demo][#7]][#8] | [![Screenshot of the Slide Puzzle][#9]][#10] | [![Screenshot of the Energy Monitor Demo][#11]][#12] | [![Screenshot of the Gallery Demo][#13]][#14] | [![Screenshot of the weather Demo][#29]][#30] |

More examples and demos in the [examples folder](examples#examples)

## Get Started

### Hello World

The UI is defined in a Domain Specific Language that is declarative, easy to use,
intuitive, and provides a powerful way to describe graphical elements, their
placement, their hierarchy, property bindings, and the flow of data through the
different states.

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

### Documentation

For more details, check out the [Slint Language Documentation](https://slint.dev/docs/slint).

The [examples](examples) folder contains examples and demos, showing how to
use the Slint markup language and how to interact with a Slint user interface
from supported programming languages.

The `docs` folder contains a lot more information, including
[build instructions](docs/building.md), and
[internal developer docs](docs/development.md).

Refer to the README of each language directory in the `api` folder:

- [C++](api/cpp) ([Documentation][#15] | [Getting Started Template][#17])
- [Rust](api/rs/slint) [![Crates.io][#18]][#19] ([Documentation][#20] | [Tutorial Video][#22] | [Getting Started Template][#23])
- [JavaScript/NodeJS (Beta)](api/node) [![npm][#24]][#25] ([Documentation][#26] | [Getting Started Template][#28])
- [Python (Beta)](api/python) [![pypi][#31]][#32] ([Documentation][#33] | [Getting Started Template][#34])

## Architecture

An application is composed of the business logic written in Rust, C++, or
JavaScript and the `.slint` user interface design markup, which is compiled to
native code.

![Architecture Overview](https://slint.dev/resources/architecture.drawio.svg)

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

NOTE: When Qt is installed on the system, the `qt` style becomes available,
using Qt's QStyle to achieve native looking widgets.

### Tooling

We have a few tools to help with the development of .slint files:

- A [**LSP Server**](./tools/lsp) that adds features like auto-complete and live
  preview of the .slint files to many editors.
- It is bundled in a [**Visual Studio Code Extension**](./editors/vscode)
  available from the market place.
- A [**slint-viewer**](./tools/viewer) tool which displays the .slint files. The
  `--auto-reload` argument makes it easy to preview your UI while you are
  working on it (when using the LSP preview is not possible).
- [**SlintPad**](https://slintpad.com/), an online editor to try out .slint syntax
  without installing anything ([sources](./tools/slintpad)).
- An [**updater**](./tools/updater) to convert the .slint files from
  previous versions to newer versions.
- A [**Figma to Slint**](https://www.figma.com/community/plugin/1474418299182276871/figma-to-slint) plugin.

Please check our [Editors README](./editors/README.md) for tips on how to
configure your favorite editor to work well with Slint.

## License

You can use Slint under ***any*** of the following licenses, at your choice:

1. Build proprietary desktop, mobile, or web applications for free with the [Royalty-free License](LICENSES/LicenseRef-Slint-Royalty-free-2.0.md),
2. Build open source embedded, desktop, mobile, or web applications for free with the [GNU GPLv3](LICENSES/GPL-3.0-only.txt),
3. Build proprietary embedded, desktop, mobile, or web applications with the [Paid license](LICENSES/LicenseRef-Slint-Software-3.0.md).

See the [Slint licensing options on the website](https://slint.dev/pricing.html) and the [Licensing FAQ](FAQ.md#licensing).

## Contributions

We welcome your contributions: in the form of code, bug reports or feedback.

- If you see an [RFC tag](https://github.com/slint-ui/slint/labels/rfc) on an
  issue, feel free to chime in.
- For contribution guidelines see [CONTRIBUTING.md](CONTRIBUTING.md).

## Frequently Asked Questions

Please see our separate [FAQ](FAQ.md).

## About us (SixtyFPS GmbH)

We are passionate about software - API design, cross-platform software
development and user interface components. Our aim is to make developing user
interfaces fun for everyone: from Python, JavaScript, C++, or Rust developers all the
way to UI/UX designers. We believe that software grows organically and keeping
it open source is the best way to sustain that growth. Our team members are
located remotely in Germany.

### Stay up to date

- Follow [@slint_ui](https://twitter.com/slint_ui) on X/Twitter.
- Follow [@slint@fosstodon.org](https://mastodon.social/@slint@fosstodon.org) on Mastodon.
- Follow [@slint-ui](https://www.linkedin.com/company/slint-ui/) on LinkedIn.
- Follow [@slint.dev](https://bsky.app/profile/slint.dev) on Bluesky
- Subscribe to our [YouTube channel](https://www.youtube.com/@Slint-UI)

### Contact us

Feel free to join [Github discussions](https://github.com/slint-ui/slint/discussions)
for general chat or questions. Use [Github issues](https://github.com/slint-ui/slint/issues)
to report public suggestions or bugs.

We chat in [our Mattermost instance](https://chat.slint.dev) where you are
welcome to listen in or ask your questions.

You can of course also contact us privately via email to [info@slint.dev](mailto://info@slint.dev).

[#1]: https://www.youtube.com/watch?v=_BDbNHrjK7g
[#2]: https://www.youtube.com/watch?v=NNNOJJsOAis
[#3]: https://www.youtube.com/watch?v=dkBwNocItGs
[#4]: https://slint.dev/resources/gallery_win_screenshot.png "Gallery"
[#5]: https://slint.dev/resources/gallery_mac_screenshot.png "Gallery"
[#6]: https://slint.dev/resources/gallery_linux_screenshot.png "Gallery"
[#7]: https://slint.dev/resources/printerdemo_screenshot.png "Printer Demo"
[#8]: https://slint.dev/demos/printerdemo/
[#9]: https://slint.dev/resources/puzzle_screenshot.png "Slide Puzzle"
[#10]: https://slint.dev/demos/slide_puzzle/
[#11]: https://slint.dev/resources/energy-monitor-screenshot.png "Energy Monitor Demo"
[#12]: https://slint.dev/demos/energy-monitor/
[#13]: https://slint.dev/resources/gallery_screenshot.png "Gallery Demo"
[#14]: https://slint.dev/demos/gallery/
[#15]: https://slint.dev/latest/docs/cpp
[#17]: https://github.com/slint-ui/slint-cpp-template
[#18]: https://img.shields.io/crates/v/slint
[#19]: https://crates.io/crates/slint
[#20]: https://slint.dev/latest/docs/rust/slint/
[#22]: https://youtu.be/WBcv4V-whHk
[#23]: https://github.com/slint-ui/slint-rust-template
[#24]: https://img.shields.io/npm/v/slint-ui
[#25]: https://www.npmjs.com/package/slint-ui
[#26]: https://slint.dev/latest/docs/node
[#28]: https://github.com/slint-ui/slint-nodejs-template
[#29]: ./demos/weather-demo/docs/img/desktop-preview.png "Weather Demo"
[#30]: https://slint.dev/demos/weather-demo/
[#31]: https://img.shields.io/pypi/v/slint
[#32]: https://pypi.org/project/slint/
[#33]: http://snapshots.slint.dev/master/docs/python/
[#34]: https://github.com/slint-ui/slint-python-template
