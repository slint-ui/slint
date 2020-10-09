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

## Documentation

SixtyFPS can be used from different frontend languages. Refer to the README of these directories for reference documentation.

The docs folder contains [build instructions](docs/building.md) and [internal developer docs](docs/development.md).

## Examples / Demo

All examples and demos are located in the [examples](/examples) folder.

### Demos running in WebAssembly Simulation

Click on the screenshot to see the WebAssembly simulation

| Printer Demo | Widget Gallery |
|--------------|----------------|
| [![Screenshot of the Printer Demo](resources/printerdemo_screenshot.png "Printer Demo")](https://www.sixtyfps.io/demos/printerdemo/) | [![Screenshot of the Gallery Demo](resources/gallery_screenshot.png "Gallery Demo")](https://www.sixtyfps.io/demos/gallery/) |

### Desktop native controls

| Windows | macOS | Linux |
|---------|-------|-------|
| ![Screenshot of the Gallery on Windows](resources/gallery_win_screenshot.png "Gallery") | ![Screenshot of the Gallery on macOS](resources/gallery_mac_screenshot.png "Gallery") | ![Screenshot of the Gallery on Linux](resources/gallery_linux_screenshot.png "Gallery") |


## Supported integration languages

 * [SixtyFps-cpp](api/sixtyfps-cpp) ([Documentation](https://www.sixtyfps.io/docs/cpp))
 * [SixtyFps-rs](api/sixtyfps-rs) ([Documentation](https://www.sixtyfps.io/docs/rust/sixtyfps/))
 * [SixtyFps-node](api/sixtyfps-node)

## The .60 Language

SixtyFPS comes with a mark-up language that is specifically designed for user interfaces: It provides a powerful way to
describe graphical elements, their placement and the flow of data through the different states. At the heart of it are a familar syntax to describe the hierarchy of elements and property bindings. Here's the obligatory "Hello World":

```60
HelloWorld := Window {
    width: 400lx;
    height: 400lx;

    Text {
       y: parent.width / 2;
       x: parent.x + 200lx;
       text: "Hello, world";
       color: blue;
    }
}
```

Check out the [language reference](docs/langref.md) for more details.

### Try online

Try online using the [Experimental online editor](https://sixtyfps.io/editor)

## Architecture

An application is composed of the business logic written in Rust, C++, or JavaScript and the `.60` user interface design markup, which
is compiled to native code.

![Architecture Overview](resources/architecture.drawio.svg)

### Compiler

The idea is that the `.60` files gets compiled ahead of time. The expression in the `.60` are
meant to be pure function that the compiler can easily interpret at compile time in order to
optimize as much as possible. The compiler could decide to "inline" properties and remove
the ones that are always constant or not changed.
Ideally it will be possible to pre-process images and text as to improve rendering time of design
on low end devices. (For example, the compiler could find out that a Text or an Image is always
on top of an Image in the same location, and pre-render the text/image on top of the
backgrund image to imprive rendering time).

The compiler is using the typical compiler phase of lexing, parsing, optimisation, code generation.

There are different backend for the code generation in the target language. The C++ code generator
generates a C++ header file, the rust generator generates rust code, and so on.
In addition, there is also an interpreter for dynamic languages.

### Runtime

The runtime library consist in an engine that can support the properties written in the `.60` language.
It is meant to reduce memory allocations. Components with all their elements and items are usually
laid out in a single memory region.

Rendering backend and styles are pluggable. There are currently two backend: the `gl` backend uses OpenGL to draw everything. There is also a `qt` backend which would use Qt's QPainter,
this allow to use native looking widgets using Qt's QStyle.
The choice of rendering backend or style is a compile time decision.

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

Please feel free to join [Github discussions](https://github.com/sixtyfpsui/sixtyfps/discussions) for general talk or questions.
[Github issues](https://github.com/sixtyfpsui/sixtyfps/issues) can be used to report public suggestions or bugs. 

You can also contact us privately by sending email to info@sixtyfps.io
