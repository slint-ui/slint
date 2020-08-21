# SixtyFPS

This is the monolithic repository of the SixtyFPS project.

## What is SixtyFPS

SixtyFPS is a toolkit to efficiently develop fluid graphical user interfaces for devices with touch screens and desktop applications. Multiple
programming language environments are supported, such as Rust or C++. Our design goals are

 - **Lightweight**: Fit into a few hundred kilobytes of RAM and require little processing power.
 - **Straightforward**: Programmers and designers should feel feel productive and enjoy the design and development process.
 - **Intuitive APIs** The APIs should be idiomatic, easy to use and intuitive, for each supported target language. Excellent documentation
   should describe the APIs, teach concepts and how to use them most efficiently.
 - **Native**: We support many platforms. From embedded devices, to desktop, including mobile and web. One should feel at 
   home on each platform. Both the user and the developer. The appearance and look and feel should match was is expected of 
   an application build for this platform.

| Printer Demo | Widget Gallery |
|--------------|----------------|
| [![Screenshot of the Printer Demo](resources/printerdemo_screenshot.png "Printer Demo")](https://www.sixtyfps.io/demos/printerdemo/) | [![Screenshot of the Gallery Demo](resources/gallery_screenshot.png "Gallery Demo")](https://www.sixtyfps.io/demos/gallery/) |

## Status

SixtyFPS is still in the early stages of development. APIs can and will change, important features are still missing.

At the moment, it's possible to create user interfaces from C++, Rust and NodeJS. Those can be compiled/shown on Linux, macOS, Windows and in Web Browsers (using WebAssembly).

## Architecture

An application is composed of the business logic written in Rust, C++ or JavaScript and the `.60` user interface markup design, which
is compiled to native code.

![Architecture Overview](resources/architecture.drawio.svg)

## Documentation

SixtyFPS can be used from different frontend languages. Refer to the README of these directories for reference documentation.

The docs folder contains [build instructions](docs/building.md) and [internal developer docs](docs/development.md).

## [SixtyFps.cpp](api/sixtyfps-cpp)
 
C++ Library

## [SixtyFps-rs](api/sixtyfps-rs)
 
Rust Crate

## [SixtyFps-js](api/sixtyfps-js)

NodeJS bindings
 
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

Contributions are welcome.
To contribute, please see [CONTRIBUTING.md](CONTRIBUTING.md).
Due to the dual-licensing nature, contributions require agreeing to a CLA.

## License

The software is provided under a dual licensing scheme

 - **GNU GPLv3**: Open source license ideal for free software.
 - **Commercial SixtyFps license**: more details to come.
