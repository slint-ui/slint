# SixtyFPS

[![Build Status](https://github.com/sixtyfpsui/sixtyfps/workflows/CI/badge.svg)](https://github.com/sixtyfpsui/sixtyfps/actions)

This is the repository of the SixtyFPS project.

## Warning: Pre-Alpha

SixtyFPS is still in the early stages of development: APIs will change and important features are still being developed. **Do not use this yet in production**.

At the moment, it's possible to create user interfaces from C++, Rust, and NodeJS. These can be compiled and shown on Linux, macOS, Windows and in Web Browsers (using WebAssembly).

We aim to support the development of this project through commercial licensing and services.
We would love to get feedback from potential future customers or users.

## What is SixtyFPS

SixtyFPS is a toolkit to efficiently develop fluid graphical user interfaces for any display: embedded devices and desktop applications. We
support multiple programming languages, such as Rust, C++ or JavaScript.

Our design goals are:

 - **Lightweight**: Fit into a few hundred kilobytes of RAM and require little processing power.
 - **Straightforward**: Programmers and designers should feel productive and be able to enjoy the design and development process.
   The APIs should be consistent, easy to use, and intuitive, no matter the target language. High-quality documentation
   should describe the APIs, teach concepts and how to use them.
 - **Native**: We support many different target platforms, from embedded devices, to desktops, including mobile and web. You should feel at
   home on each platform; both the user and the developer. The look and feel and experience should match what users expect of an
   application built for a specific platform.

### Demos running in WebAssembly Simulation

| Printer Demo | Widget Gallery |
|--------------|----------------|
| [![Screenshot of the Printer Demo](resources/printerdemo_screenshot.png "Printer Demo")](https://www.sixtyfps.io/demos/printerdemo/) | [![Screenshot of the Gallery Demo](resources/gallery_screenshot.png "Gallery Demo")](https://www.sixtyfps.io/demos/gallery/) |

### Desktop native controls

| Windows | macOS | Linux |
|---------|-------|-------|
| ![Screenshot of the Gallery on Windows](resources/gallery_win_screenshot.png "Gallery") | ![Screenshot of the Gallery on macOS](resources/gallery_mac_screenshot.png "Gallery") | ![Screenshot of the Gallery on Linux](resources/gallery_linux_screenshot.png "Gallery") |

## Architecture

An application is composed of the business logic written in Rust, C++, or JavaScript and the `.60` user interface design markup, which
is compiled to native code.

![Architecture Overview](resources/architecture.drawio.svg)

## Documentation

SixtyFPS can be used from different frontend languages. Refer to the README of these directories for reference documentation.

The docs folder contains [build instructions](docs/building.md) and [internal developer docs](docs/development.md).

## Supported integration languages

 * [SixtyFps-cpp](api/sixtyfps-cpp) ([Documentation](https://www.sixtyfps.io/docs/cpp))
 * [SixtyFps-rs](api/sixtyfps-rs) ([Documentation](https://www.sixtyfps.io/docs/rust/sixtyfps/))
 * [SixtyFps-node](api/sixtyfps-node)

## The .60 Language

SixtyFPS comes with a mark-up language that is specifically designed for user interfaces: It provides a powerful way to
describe graphical elements, their placement and the flow of data through the different states. At the heart of it are a familar syntax to describe the hierarchy of elements and property bindings. Here's the obligatory "Hello World":

```60
HelloWorld := Window {
    Text {
       text: "Hello, world"
       color: blue;
    }
}
```

Check out the [language reference](docs/langref.md) for more details.

## Examples / Demo

All examples and demos are located in the [examples](/examples) folder.

## Contributions

Contributions are welcome, in the form of code, bug reports or feedback. As an example
we track issues where we specifically seek feedback with an [RFC tag](https://github.com/sixtyfpsui/sixtyfps/labels/rfc).
To contribute, please see [CONTRIBUTING.md](CONTRIBUTING.md).
Due to the dual-licensing nature, contributions require agreeing to a CLA.

## License

The software is provided under a dual licensing scheme

 - **GNU GPLv3**: Open source license ideal for free software.
 - **Commercial SixtyFps license**: more details to come.

## Contact us

[Github issues](https://github.com/sixtyfpsui/sixtyfps/issues) can be used to report public suggestions or bugs.

You can also contact us privately by sending email to info@sixtyfps.io
