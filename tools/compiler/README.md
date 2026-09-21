
# slint-compiler

A command-line tool that compiles a `.slint` user interface into C++, Rust or Python source code.

## Overview

`slint-compiler` is the ahead-of-time compiler of the [Slint framework](https://slint.dev).
It reads a `.slint` file and writes the code that builds and drives that user interface at run time,
so the application compiles the result with the rest of its source instead of parsing `.slint` on startup.
`--format` picks the language: a C++ header, Rust code, or a typed Python module.

C++ projects run it through the CMake integration, and Rust projects through `slint-build` or the `slint!` macro.
Reach for the binary when you drive the build yourself: another build system, a cross-compilation step that
generates code on the host, or a look at what Slint generates.

It also decides how resources reach the binary.
`--embed-resources` picks between referring to files by path and embedding them,
including a layout that suits the software renderer on a device with no filesystem.
Translations marked with `@tr()` can be bundled the same way with `--bundle-translations`.

See the [Slint documentation](https://docs.slint.dev) for the language itself.

## Installation

From crates.io, which builds it from source and needs
[Rust](https://www.rust-lang.org/learn/get-started):

```bash
cargo install --locked slint-compiler
```

Or from PyPI, which ships a pre-built binary and needs no Rust toolchain:

```bash
pip install slint-compiler
```

Both install the same `slint-compiler` program.

## Usage

Generate a C++ header from a `.slint` file:

```bash
slint-compiler -f cpp -o app-window.h app-window.slint
```

Generate Rust code instead, and embed the images and fonts it refers to:

```bash
slint-compiler -f rust -o app-window.rs --embed-resources embed-files app-window.slint
```

```
Usage: slint-compiler [OPTIONS] <file>

Arguments:
  <file>  Specify the path to the main .slint file to compile. Use '-' to read from stdin

Options:
  -f, --format <FORMAT>              Output format: 'cpp', 'rust', 'python', or 'llr' for the
                                     compiler's low-level representation
  -I <include path>                  Include path for imported .slint files and image resources
  -L <library path>                  Library path as `<library>=<path>`, a directory or an entry-point file
      --style <style name>           Set the style for the UI (e.g., 'native' or 'fluent')
      --scale-factor <scale factor>  Scale factor applied to embedded assets, for high-DPI displays
      --depfile <dependency file>    Write a dependency file for build systems like CMake or Ninja
      --embed-resources <value>      Declare which resources to embed into the final output
  -o <output file>                   Output file for the generated code, '-' for stdout [default: -]
      --translation-domain <DOMAIN>  Translation domain for translatable strings
      --bundle-translations <path>   Bundle the gettext `.po` files found under this path
      --no-default-translation-context
                                     Don't use the component name as the default `@tr` context
      --cpp-namespace <C++ namespace>  C++ namespace for the generated code
      --cpp-file <output .cpp file>  Put the function definitions in this .cpp file instead of the header
  -h, --help                         Print help (see more with '--help')
  -V, --version                      Print version
```
