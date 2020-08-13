# SixtyFPS

This is the monolithic repository of the SixtyFPS project.

## What is SixtyFPS

It is a toolkit to efficiently develop fluid graphical user interfaces in different languages.
The aim of the project is to be

 - **Lightweight**: We aim to provide a runtime that fits into a few hundred kilobytes of RAM and requires little processing
   power.
 - **Straightforward**: We want programmers and designers to feel productive. It should be an enjoyable experience to design
   and developer the user interface. We aim to provide APIS that are idiomatic and easy to use for each target language,
   along with excellent documentation and modern tools.
 - **Native**: We support many platforms. From embedded devices, to desktop, including mobile and web. One should feel at 
   home on each platform. Both the user and the developer. The appearance and look and feel should match was is expected of 
   an application build for this platform.

| Printer Demo | Widget Gallery |
|--------------|----------------|
| [![Screenshot of the Printer Demo](resources/printerdemo_screenshot.png "Printer Demo")](https://www.sixtyfps.io/demos/printerdemo/) | [![Screenshot of the Gallery Demo](resources/gallery_screenshot.png "Gallery Demo")](https://www.sixtyfps.io/demos/gallery/) |

## Documentation

SixtyFPS can be used from different frontend languages. Refer to the README of these direcory for
actual user documentation.

The docs folder contains [build instructions](docs/building.md) and [internal developer docs](docs/development.md).
 
## [SixtyFps.cpp](api/sixtyfps-cpp)
 
Modern C++ Library for GUI

## [SixtyFps-rs](api/sixtyfps-rs)
 
Rust GUI library

## [SixtyFps-js](api/sixtyfps-js)

NodeJS bindings
 
## The .60 DSL

SixtyFPS UI comes with a mark-up language that is specifically designed for user interfaces: It provides a powerful way to
describe graphical elements, their placement and the flow of data through the different states. Here's the obligatory
"Hello World":

```60
HelloWorld := Window {
    Text {
       text: "Hello, world"
    }
}
```

Check out the [language reference](docs/langref.md) for more details.

## Examples / Demo

Todo

## Contributions

Contributions are welcome.
To contribute, please see [CONTRIBUTING.md](CONTRIBUTING.md).
Contributions are done under a CLA.

## License

The software is provided under a dual license scheme

 - **GNU GPLv3**: Open source license ideal for free software.
 - **Commercial SixtyFps license**: more details to come.
