# SixtyFPS

This is the monolithique repository of the SixtyFPS project.

## What is SixtyFPS

It is a GUI engine, with libraries for different languages.
The aim of the project is to be

 - **Lightweight**: We aim to provide a runtime that fits in a few MB. Minimize the RAM usage, and also be blazing fast.
 - **Straightforward**: We want to optimize as well for programmer and designer productivity.
    Making it an agreable experience to work with. We claim to use well thought idiomatic API for each target language.
    Provide well written documentation. And use of modern tools.
 - **Native**: We support many platform. From embedded devices, to desktop, including mobile and web. One should feel at 
    home on each platfom. Both the user and the developer. The appearence and look and feel should match was is expected of 
    an application build for this platform.
    
 `<insert a few screenshot here>`
 
## [SixtyFps.cpp](api/sixtyfps-cpp)
 
Modern C++ Library for GUI

## [SixtyFps-rs](api/sixtyfps-rs)
 
Rust GUI library

## [SixtyFps-js](api/sixtyfps-js)

NodeJS bindings
 
## The .60 DSL

```60
HelloWorld = Window {
 TextLabel { text: "Hello, world" }
}
```

### [The compiler](...)

### [The runtime library](...)

## Examples / Demo

Todo

## Contributions

Contributions are welcome.
To contribute to Rust, please see [CONTRIBUTING.md](CONTRIBUTING.md).
Contributions are done under a CLA.

## License

The software is provided under a double license scheme

 - **GNU GPLv3**: Open source license ideal for free software
 - **Comertial SixtyFps license**: more details to come.
